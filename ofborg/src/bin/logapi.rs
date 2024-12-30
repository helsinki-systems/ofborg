use std::{collections::HashMap, error::Error, path::PathBuf};

use hyper::{
    header::ContentType,
    mime,
    server::{Request, Response, Server},
    status::StatusCode,
};
use ofborg::config;
use tracing::{error, info, warn};

#[derive(serde::Serialize, Default)]
struct Attempt {
    metadata: Option<serde_json::Value>,
    result: Option<serde_json::Value>,
    log_url: Option<String>,
}

#[derive(serde::Serialize)]
struct LogResponse {
    attempts: HashMap<String, Attempt>,
}

fn main() -> Result<(), Box<dyn Error>> {
    ofborg::setup_log();

    let arg = std::env::args()
        .nth(1)
        .unwrap_or_else(|| panic!("usage: {} <config>", std::env::args().next().unwrap()));
    let Some(cfg) = config::load(arg.as_ref()).log_api_config else {
        error!("No LogApi configuration found!");
        panic!();
    };

    let threads = std::thread::available_parallelism()
        .map(|x| x.get())
        .unwrap_or(1);
    info!("Will listen on {} with {threads} threads", cfg.listen);
    Server::http(cfg.listen)?.handle_threads(
        move |req: Request, mut res: Response| {
            if req.method != hyper::Get {
                *res.status_mut() = StatusCode::MethodNotAllowed;
                return;
            }

            let uri = req.uri.to_string();
            let Some(reqd) = uri.strip_prefix("/logs/").map(ToOwned::to_owned) else {
                *res.status_mut() = StatusCode::NotFound;
                let _ = res.send(b"invalid uri");
                return;
            };
            let path: PathBuf = [&cfg.logs_path, &reqd].iter().collect();
            let Ok(path) = std::fs::canonicalize(&path) else {
                *res.status_mut() = StatusCode::NotFound;
                let _ = res.send(b"absent");
                return;
            };
            let Ok(iter) = std::fs::read_dir(path) else {
                *res.status_mut() = StatusCode::NotFound;
                let _ = res.send(b"non dir");
                return;
            };

            let mut attempts = HashMap::<String, Attempt>::new();
            for e in iter {
                let Ok(e) = e else { continue };
                let e_metadata = e.metadata();
                if e_metadata.as_ref().map(|v| v.is_dir()).unwrap_or(true) {
                    *res.status_mut() = StatusCode::InternalServerError;
                    let _ = res.send(b"dir found");
                    return;
                }

                if e_metadata.as_ref().map(|v| v.is_file()).unwrap_or_default() {
                    let Ok(file_name) = e.file_name().into_string() else {
                        warn!("entry filename is not a utf-8 string: {:?}", e.file_name());
                        continue;
                    };

                    if file_name.ends_with(".metadata.json") || file_name.ends_with(".result.json")
                    {
                        let Ok(file) = std::fs::File::open(e.path()) else {
                            warn!("could not open file: {file_name}");
                            continue;
                        };
                        let Ok(json) = serde_json::from_reader::<_, serde_json::Value>(file) else {
                            warn!("file is not a valid json file: {file_name}");
                            continue;
                        };
                        let Some(attempt_id) = json
                            .get("attempt_id")
                            .and_then(|v| v.as_str())
                            .map(ToOwned::to_owned)
                        else {
                            warn!("attempt_id not found in file: {file_name}");
                            continue;
                        };
                        let attempt_obj = attempts
                            .entry(attempt_id)
                            .or_insert_with(Attempt::default);
                        if file_name.ends_with(".metadata.json") {
                            attempt_obj.metadata = Some(json);
                        } else {
                            attempt_obj.result = Some(json);
                        }
                    } else {
                        let attempt_obj = attempts
                            .entry(file_name.clone())
                            .or_insert_with(Attempt::default);
                        attempt_obj.log_url =
                            Some(format!("{}/{reqd}/{file_name}", &cfg.serve_root));
                    }
                }
            }

            *res.status_mut() = StatusCode::Ok;
            res.headers_mut()
                .set::<ContentType>(hyper::header::ContentType(mime::Mime(
                    mime::TopLevel::Application,
                    mime::SubLevel::Json,
                    Vec::new(),
                )));
            let _ = res.send(
                serde_json::to_string(&LogResponse { attempts })
                    .unwrap_or_default()
                    .as_bytes(),
            );
        },
        threads,
    )?;
    Ok(())
}
