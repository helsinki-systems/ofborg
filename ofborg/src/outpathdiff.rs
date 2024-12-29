use crate::nix;
use crate::nixenv::{Error as NixEnvError, HydraNixEnv};

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::path::PathBuf;

use tracing::{debug, trace, warn};

pub struct OutPathDiff {
    calculator: HydraNixEnv,
    pub original: Option<PackageOutPaths>,
    pub current: Option<PackageOutPaths>,
}

impl OutPathDiff {
    pub fn new(nix: nix::Nix, path: PathBuf) -> OutPathDiff {
        OutPathDiff {
            calculator: HydraNixEnv::new(nix, path, false),
            original: None,
            current: None,
        }
    }

    pub fn find_before(&mut self) -> Result<(), NixEnvError> {
        self.original = Some(self.run()?);
        Ok(())
    }

    pub fn find_after(&mut self) -> Result<(), NixEnvError> {
        if self.original.is_none() {
            debug!("Before is None, not bothering with After");
            return Ok(());
        }

        self.current = Some(self.run()?);
        Ok(())
    }

    pub fn package_diff(&self) -> Option<(Vec<PackageArch>, Vec<PackageArch>)> {
        if let Some(ref cur) = self.current {
            if let Some(ref orig) = self.original {
                let orig_set: HashSet<&PackageArch> = orig.keys().collect();
                let cur_set: HashSet<&PackageArch> = cur.keys().collect();

                let removed: Vec<PackageArch> = orig_set
                    .difference(&cur_set)
                    .map(|p| (*p).clone())
                    .collect();
                let added: Vec<PackageArch> = cur_set
                    .difference(&orig_set)
                    .map(|p| (*p).clone())
                    .collect();
                Some((removed, added))
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn calculate_rebuild(&self) -> Option<Vec<PackageArch>> {
        let mut rebuild: Vec<PackageArch> = vec![];

        if let Some(ref cur) = self.current {
            if let Some(ref orig) = self.original {
                for key in cur.keys() {
                    trace!("Checking out {:?}", key);
                    if cur.get(key) != orig.get(key) {
                        trace!("    {:?} != {:?}", cur.get(key), orig.get(key));
                        rebuild.push(key.clone())
                    } else {
                        trace!("    {:?} == {:?}", cur.get(key), orig.get(key));
                    }
                }

                return Some(rebuild);
            }
        }

        None
    }

    fn run(&mut self) -> Result<PackageOutPaths, NixEnvError> {
        self.calculator.execute()
    }
}

pub type PackageOutPaths = HashMap<PackageArch, OutPath>;

#[derive(Debug, PartialEq, Hash, Eq, Clone)]
pub struct PackageArch {
    pub package: Package,
    pub architecture: Architecture,
}
type Package = String;
type Architecture = String;
type OutPath = String;

pub fn parse_json(data_file: File) -> Result<PackageOutPaths, Box<dyn std::error::Error>> {
    let json: HashMap<String, HashMap<String, String>> = serde_json::from_reader(data_file)?;
    Ok(json
        .iter()
        .filter_map(|(name, outs)| {
            let path: Vec<&str> = name.rsplitn(2, '.').collect();
            if path.len() == 2 {
                Some((
                    PackageArch {
                        package: String::from(path[1]),
                        architecture: String::from(path[0]),
                    },
                    outs.clone()
                        .into_values()
                        .collect::<Vec<String>>()
                        .join(" "),
                ))
            } else {
                warn!("Didn't detect an architecture for {:?}", path);
                None
            }
        })
        .collect::<PackageOutPaths>())
}
