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
use nix_utils::BaseStore as _;
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
async fn import_drvs(
    client: &mut OfborgClient,
    drv_paths: &[nix_utils::StorePath],
) -> anyhow::Result<()> {
    let (raw_writer, rx) = compression_encode_channel();

    let paths = drv_paths.to_owned();
    tokio::task::spawn_blocking(move || {
        let store = nix_utils::LocalStore::init();
        let mut sync_writer = tokio_util::io::SyncIoBridge::new(raw_writer);
        let closure = move |data: &[u8]| {
            use std::io::Write;
            sync_writer.write_all(data).is_ok()
        };
        let _ = store.export_paths(&paths, closure);
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
    drv_paths: &[nix_utils::StorePath],
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
async fn main() -> anyhow::Result<()> {
    hydra_tracing::init()?;
    nix_utils::init_nix();

    let cli = Arc::new(config::Cli::new());
    let hostname = cli.get_hostname();

    tracing::info!(
        "ofborg-evaluator starting, hostname={hostname}, endpoint={}",
        cli.gateway_endpoint
    );

    let drv_paths: Vec<nix_utils::StorePath> = cli
        .drv
        .iter()
        .map(|s| nix_utils::StorePath::new(s))
        .collect();

    tracing::info!("connecting to queue-runner");
    let mut client = grpc::init_client(&cli).await?;

    // Import drv files into the queue-runner's store via BuildResult
    tracing::info!("importing {} drv(s) via BuildResult", drv_paths.len());
    import_drvs(&mut client, &drv_paths).await?;

    // Create build records for the imported drvs
    tracing::info!(
        "creating builds via CreateBuild (jobset_id={})",
        cli.jobset_id
    );
    let build_ids = create_builds(&mut client, cli.jobset_id, &drv_paths).await?;

    println!("Created {} build(s):", build_ids.len());
    for (drv_path, build_id) in &build_ids {
        println!("  {build_id} <- {drv_path}");
    }

    tracing::info!("ofborg-evaluator done, tunnel continues in background");

    // Keep the process alive so the tunnel stays connected
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
    }
}
