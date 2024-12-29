//! Evaluates the expression like Hydra would, with regards to
//! architecture support and recursed packages.
use crate::nix;
use crate::outpathdiff;

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::PathBuf;

pub struct HydraNixEnv {
    path: PathBuf,
    nix: nix::Nix,
    check_meta: bool,
}

impl HydraNixEnv {
    pub fn new(nix: nix::Nix, path: PathBuf, check_meta: bool) -> HydraNixEnv {
        HydraNixEnv {
            path,
            nix,
            check_meta,
        }
    }

    pub fn execute(&self) -> Result<outpathdiff::PackageOutPaths, Error> {
        let (status, stdout, stderr) = self.run_nix_env();

        if status {
            let outpaths = outpathdiff::parse_json(
                File::open(self.outpaths_json()).map_err(|e| Error::Io(e))?,
            )
            .map_err(|e| Error::Internal(e))?;

            let evaluation_errors = BufReader::new(stderr)
                .lines()
                .collect::<Result<Vec<String>, _>>()?
                .into_iter()
                .filter(|msg| !msg.trim().is_empty())
                .filter(|line| !nix::is_user_setting_warning(line))
                .collect::<Vec<String>>();

            if !evaluation_errors.is_empty() {
                return Err(Error::UncleanEvaluation(evaluation_errors));
            }

            Ok(outpaths)
        } else {
            Err(Error::CommandFailed(stderr))
        }
    }

    fn outpaths_json(&self) -> PathBuf {
        self.path.join("result/outpaths.json")
    }

    fn run_nix_env(&self) -> (bool, File, File) {
        let check_meta = if self.check_meta { "true" } else { "false" };

        let cmd = self.nix.safe_command(
            &nix::Operation::Build,
            &self.path,
            &[
                "ci",
                "-A",
                "eval.full",
                "--max-jobs",
                "1",
                "--cores",
                "4",
                "--arg",
                "nixpkgs",
                self.path.to_str().unwrap(),
                "--arg",
                "chunkSize",
                "10000",
                "--arg",
                "evalSystems",
                "[\"x86_64-linux\"]",
                "--arg",
                "checkMeta",
                check_meta,
                "--out-link",
                &format!("{}/result", self.path.to_str().unwrap()),
            ],
            &[],
        );

        let (status, stdout, stderr) = self.nix.run_stderr_stdout(cmd);

        (status, stdout, stderr)
    }
}

pub enum Error {
    Io(io::Error),
    Internal(Box<dyn std::error::Error>),
    CommandFailed(File),
    StatsParse(File, Result<u64, io::Error>, serde_json::Error),
    UncleanEvaluation(Vec<String>),
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Error {
        Error::Io(e)
    }
}

impl Error {
    pub fn display(self) -> String {
        match self {
            Error::Io(err) => format!("Failed during the setup of executing nix-env: {err:?}"),
            Error::Internal(err) => format!("Internal error: {err:?}"),
            Error::CommandFailed(mut fd) => {
                let mut buffer = Vec::new();
                let read_result = fd.read_to_end(&mut buffer);
                let bufstr = String::from_utf8_lossy(&buffer);

                match read_result {
                    Ok(_) => format!("nix-env failed:\n{bufstr}"),
                    Err(err) => format!(
                        "nix-env failed and loading the error result caused a new error {err:?}\n\n{bufstr}"
                    ),
                }
            }
            Error::UncleanEvaluation(warnings) => {
                format!("nix-env did not evaluate cleanly:\n {warnings:?}")
            }
            Error::StatsParse(mut fd, seek, parse_err) => {
                let mut buffer = Vec::new();
                let read_result = fd.read_to_end(&mut buffer);
                let bufstr = String::from_utf8_lossy(&buffer);

                let mut lines =
                    String::from("Parsing nix-env's performance statistics failed.\n\n");

                if let Err(seek_err) = seek {
                    lines.push_str(&format!(
                        "Additionally, resetting to the beginning of the output failed with:\n{seek_err:?}\n\n"
                    ));
                }

                if let Err(read_err) = read_result {
                    lines.push_str(&format!(
                        "Additionally, loading the output failed with:\n{read_err:?}\n\n"
                    ));
                }

                lines.push_str(&format!("Parse error:\n{parse_err:?}\n\n"));

                lines.push_str(&format!("Evaluation output:\n{bufstr}"));

                lines
            }
        }
    }
}
