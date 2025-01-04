use crate::maintainers::{Maintainer, MaintainersByPackage};
use crate::outpathdiff::PackageArch;

pub struct PkgsAddedRemovedTagger {
    possible: Vec<String>,
    selected: Vec<String>,
}

impl Default for PkgsAddedRemovedTagger {
    fn default() -> PkgsAddedRemovedTagger {
        let mut t = PkgsAddedRemovedTagger {
            possible: vec![
                String::from("8.has: package (new)"),
                String::from("8.has: clean-up"),
            ],
            selected: vec![],
        };
        t.possible.sort();

        t
    }
}

impl PkgsAddedRemovedTagger {
    pub fn new() -> PkgsAddedRemovedTagger {
        Default::default()
    }

    pub fn changed(&mut self, removed: &[PackageArch], added: &[PackageArch]) {
        if !removed.is_empty() {
            self.selected.push(String::from("8.has: clean-up"));
        }

        if !added.is_empty() {
            self.selected.push(String::from("8.has: package (new)"));
        }
    }

    pub fn tags_to_add(&self) -> Vec<String> {
        self.selected.clone()
    }

    pub fn tags_to_remove(&self) -> Vec<String> {
        // The cleanup tag is too vague to automatically remove.
        vec![]
    }
}

pub struct MaintainerPrTagger {
    possible: Vec<String>,
    selected: Vec<String>,
}

impl Default for MaintainerPrTagger {
    fn default() -> MaintainerPrTagger {
        let mut t = MaintainerPrTagger {
            possible: vec![String::from("11.by: package-maintainer")],
            selected: vec![],
        };
        t.possible.sort();

        t
    }
}

impl MaintainerPrTagger {
    pub fn new() -> MaintainerPrTagger {
        Default::default()
    }

    pub fn record_maintainer(
        &mut self,
        pr_submitter: &str,
        identified_maintainers: &MaintainersByPackage,
    ) {
        let submitter = Maintainer::from(pr_submitter);

        if identified_maintainers.0.is_empty() {
            // No packages -> not from the maintainer
            return;
        }

        for (_package, maintainers) in identified_maintainers.0.iter() {
            if !maintainers.contains(&submitter) {
                // One of the packages is not maintained by this submitter
                return;
            }
        }

        self.selected
            .push(String::from("11.by: package-maintainer"));
    }

    pub fn tags_to_add(&self) -> Vec<String> {
        self.selected.clone()
    }

    pub fn tags_to_remove(&self) -> Vec<String> {
        // The cleanup tag is too vague to automatically remove.
        vec![]
    }
}
