#![forbid(unsafe_code)]
#![deny(
    clippy::all,
    clippy::pedantic,
    clippy::expect_used,
    clippy::unwrap_used,
    future_incompatible,
    missing_debug_implementations,
    nonstandard_style,
    missing_copy_implementations,
    unused_qualifications
)]
#![allow(clippy::missing_errors_doc)]

mod config;
mod grpc;

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Context as _;
use harmonia_store_core::store_path::{FromStoreDirStr, StorePath};
use lapin::options::{BasicAckOptions, BasicConsumeOptions, QueueDeclareOptions};
use lapin::types::FieldTable;
use nix_utils::BaseStore as _;
use std::io::Write;
use tokio::sync::mpsc;
use tokio_stream::StreamExt as _;
use tonic::Request;

use grpc::OfborgClient;
use grpc::runner_v1::{CreateBuildRequest, NarData};

type CompressionEncoder<R> = async_compression::tokio::bufread::ZstdEncoder<R>;

const DUPLEX_BUFFER_SIZE: usize = 256 * 1024;

fn compression_encode_channel() -> (
    tokio::io::DuplexStream,
    mpsc::UnboundedReceiver<Result<NarData, tonic::Status>>,
) {
    let (raw_writer, raw_reader) = tokio::io::duplex(DUPLEX_BUFFER_SIZE);
    let (tx, rx) = mpsc::unbounded_channel();

    tokio::task::spawn(async move {
        let encoder = CompressionEncoder::new(tokio::io::BufReader::new(raw_reader));
        let mut stream = tokio_util::io::ReaderStream::new(encoder);
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(bytes) => {
                    if tx
                        .send(Ok(NarData {
                            chunk: bytes.into(),
                        }))
                        .is_err()
                    {
                        break;
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to compress chunk: {e}");
                    let _ = tx.send(Err(tonic::Status::internal("Compression error")));
                    break;
                }
            }
        }
    });

    (raw_writer, rx)
}

#[tracing::instrument(skip(client, drv_paths), err)]
async fn import_drvs(client: &mut OfborgClient, drv_paths: &[StorePath]) -> anyhow::Result<()> {
    let (raw_writer, rx) = compression_encode_channel();

    let paths = drv_paths.to_owned();
    tokio::task::spawn_blocking(move || {
        let store = nix_utils::LocalStore::init();
        let mut sync_writer = tokio_util::io::SyncIoBridge::new(raw_writer);
        for path in &paths {
            let _ = store.nar_from_path(path, |data: &[u8]| sync_writer.write_all(data).is_ok());
        }
    });

    let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx).filter_map(Result::ok);
    client
        .build_result(Request::new(stream))
        .await
        .context("Failed to import drv paths via BuildResult")?;

    Ok(())
}

#[tracing::instrument(skip(client), err)]
async fn create_builds(
    client: &mut OfborgClient,
    jobset_id: i32,
    drv_paths: &[StorePath],
) -> anyhow::Result<HashMap<String, i32>> {
    let drv_strs: Vec<String> = drv_paths
        .iter()
        .map(|p| nix_utils::LocalStore::init().print_store_path(p))
        .collect();

    let response = client
        .create_build(CreateBuildRequest {
            jobset_id,
            drv_paths: drv_strs,
        })
        .await
        .context("Failed to call CreateBuild")?;

    Ok(response.into_inner().build_ids)
}

#[tokio::main]
#[allow(clippy::too_many_lines)]
async fn main() -> anyhow::Result<()> {
    hydra_tracing::init()?;
    nix_utils::init_nix();

    let cli = Arc::new(config::Cli::new());
    let Some(cfg) = ofborg::config::load(&cli.config_path).hydra_evaluator else {
        tracing::error!("No ofborg/hydra evaluator configuration found!");
        panic!();
    };

    tracing::info!(
        "ofborg-evaluator starting endpoint={}",
        cli.gateway_endpoint
    );

    tracing::info!("running in AMQP consumer mode");
    let conn = ofborg::easylapin::from_config(&cfg.rabbitmq).await?;
    let chan = conn.create_channel().await?;

    // Declare the hydra-eval-jobs queue (must match what mass-rebuilder publishes to)
    chan.queue_declare(
        "hydra-eval-jobs".into(),
        QueueDeclareOptions {
            passive: false,
            durable: true,
            exclusive: false,
            auto_delete: false,
            nowait: false,
        },
        FieldTable::default(),
    )
    .await?;

    tracing::info!("connecting to queue-runner gRPC");
    let mut client = grpc::init_client(&cli).await?;

    tracing::info!("consuming from hydra-eval-jobs");
    let mut consumer = chan
        .basic_consume(
            "hydra-eval-jobs".into(),
            "ofborg-hydra-evaluator".into(),
            BasicConsumeOptions::default(),
            FieldTable::default(),
        )
        .await?;

    while let Some(Ok(delivery)) = consumer.next().await {
        let body = &delivery.data;
        let job: ofborg::message::hydra_eval_job::HydraEvalJob = match serde_json::from_slice(body)
        {
            Ok(job) => job,
            Err(e) => {
                tracing::error!(
                    "Failed to deserialize HydraEvalJob: {e}, body: {:?}",
                    std::str::from_utf8(body)
                );
                let _ = chan
                    .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                    .await;
                continue;
            }
        };

        tracing::info!(
            "Processing HydraEvalJob for {}/{} PR #{} ({} drv paths, jobset_id={})",
            job.repo.owner,
            job.repo.name,
            job.pr.number,
            job.drv_paths.len(),
            job.jobset_id,
        );

        if job.drv_paths.is_empty() {
            tracing::warn!("Received HydraEvalJob with no drv paths, acking");
            let _ = chan
                .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
                .await;
            continue;
        }

        let store_dir = nix_utils::LocalStore::init().store_dir().clone();
        let drv_paths: Vec<StorePath> = job
            .drv_paths
            .iter()
            .map(|s| {
                StorePath::from_store_dir_str(&store_dir, s)
                    .unwrap_or_else(|e| panic!("Invalid store path '{s}': {e}"))
            })
            .collect();

        match import_drvs(&mut client, &drv_paths).await {
            Ok(()) => {
                tracing::info!("Successfully imported {} drv(s)", drv_paths.len());
            }
            Err(e) => {
                tracing::error!("Failed to import drvs: {e:?}");
                // Nack and requeue so another consumer can retry
                let _ = chan
                    .basic_nack(
                        delivery.delivery_tag,
                        lapin::options::BasicNackOptions {
                            requeue: true,
                            ..Default::default()
                        },
                    )
                    .await;
                continue;
            }
        }

        match create_builds(&mut client, job.jobset_id, &drv_paths).await {
            Ok(build_ids) => {
                tracing::info!("Created {} build(s)", build_ids.len());
                for (drv_path, build_id) in &build_ids {
                    tracing::info!("  {build_id} <- {drv_path}");
                }
            }
            Err(e) => {
                tracing::error!("Failed to create builds: {e:?}");
                let _ = chan
                    .basic_nack(
                        delivery.delivery_tag,
                        lapin::options::BasicNackOptions {
                            requeue: true,
                            ..Default::default()
                        },
                    )
                    .await;
                continue;
            }
        }

        let _ = chan
            .basic_ack(delivery.delivery_tag, BasicAckOptions::default())
            .await;
        tracing::info!("Finished processing job for PR #{}", job.pr.number);
    }

    drop(conn); // Close connection.
    tracing::info!("Closed the session... EOF");
    Ok(())
}
