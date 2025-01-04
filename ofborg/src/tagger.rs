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

#[cfg(test)]
mod tests {
    use super::*;

    struct PackageArchSrc {
        linux: usize,
        darwin: usize,
    }

    impl PackageArchSrc {
        pub fn linux(linux: usize) -> PackageArchSrc {
            PackageArchSrc { linux, darwin: 0 }
        }

        pub fn and_darwin(mut self, darwin: usize) -> PackageArchSrc {
            self.darwin = darwin;
            self
        }
    }

    impl From<PackageArchSrc> for Vec<PackageArch> {
        fn from(src: PackageArchSrc) -> Vec<PackageArch> {
            let darwin: Vec<PackageArch> = (0..src.darwin)
                .map(|_| PackageArch {
                    package: String::from("bogus :)"),
                    architecture: String::from("x86_64-darwin"),
                })
                .collect();
            let linux: Vec<PackageArch> = (0..src.linux)
                .map(|_| PackageArch {
                    package: String::from("bogus :)"),
                    architecture: String::from("x86_64-linux"),
                })
                .collect();

            let mut combined = darwin;
            combined.extend(linux);
            combined
        }
    }

    #[test]
    pub fn test_packages_changed() {
        let mut tagger = RebuildTagger::new();
        tagger.parse_attrs(PackageArchSrc::linux(0).and_darwin(0).into());
        assert_eq!(
            tagger.tags_to_add(),
            vec!["10.rebuild-darwin: 0", "10.rebuild-linux: 0",]
        );
        assert_eq!(
            tagger.tags_to_remove(),
            vec![
                "10.rebuild-darwin: 1",
                "10.rebuild-darwin: 1-10",
                "10.rebuild-darwin: 11-100",
                "10.rebuild-darwin: 101-500",
                "10.rebuild-darwin: 501+",
                "10.rebuild-darwin: 501-1000",
                "10.rebuild-darwin: 1001-2500",
                "10.rebuild-darwin: 2501-5000",
                "10.rebuild-darwin: 5001+",
                "10.rebuild-linux: 1",
                "10.rebuild-linux: 1-10",
                "10.rebuild-linux: 11-100",
                "10.rebuild-linux: 101-500",
                "10.rebuild-linux: 501+",
                "10.rebuild-linux: 501-1000",
                "10.rebuild-linux: 1001-2500",
                "10.rebuild-linux: 2501-5000",
                "10.rebuild-linux: 5001+",
            ]
        );

        let mut tagger = RebuildTagger::new();
        tagger.parse_attrs(PackageArchSrc::linux(1).into());

        assert_eq!(
            tagger.tags_to_add(),
            vec![
                "10.rebuild-darwin: 0",
                "10.rebuild-linux: 1",
                "10.rebuild-linux: 1-10",
            ]
        );
        assert_eq!(
            tagger.tags_to_remove(),
            vec![
                "10.rebuild-darwin: 1",
                "10.rebuild-darwin: 1-10",
                "10.rebuild-darwin: 11-100",
                "10.rebuild-darwin: 101-500",
                "10.rebuild-darwin: 501+",
                "10.rebuild-darwin: 501-1000",
                "10.rebuild-darwin: 1001-2500",
                "10.rebuild-darwin: 2501-5000",
                "10.rebuild-darwin: 5001+",
                "10.rebuild-linux: 0",
                "10.rebuild-linux: 11-100",
                "10.rebuild-linux: 101-500",
                "10.rebuild-linux: 501+",
                "10.rebuild-linux: 501-1000",
                "10.rebuild-linux: 1001-2500",
                "10.rebuild-linux: 2501-5000",
                "10.rebuild-linux: 5001+"
            ]
        );

        let mut tagger = RebuildTagger::new();
        tagger.parse_attrs(PackageArchSrc::linux(1).and_darwin(1).into());
        assert_eq!(
            tagger.tags_to_add(),
            vec![
                "10.rebuild-darwin: 1",
                "10.rebuild-darwin: 1-10",
                "10.rebuild-linux: 1",
                "10.rebuild-linux: 1-10",
            ]
        );
        assert_eq!(
            tagger.tags_to_remove(),
            vec![
                "10.rebuild-darwin: 0",
                "10.rebuild-darwin: 11-100",
                "10.rebuild-darwin: 101-500",
                "10.rebuild-darwin: 501+",
                "10.rebuild-darwin: 501-1000",
                "10.rebuild-darwin: 1001-2500",
                "10.rebuild-darwin: 2501-5000",
                "10.rebuild-darwin: 5001+",
                "10.rebuild-linux: 0",
                "10.rebuild-linux: 11-100",
                "10.rebuild-linux: 101-500",
                "10.rebuild-linux: 501+",
                "10.rebuild-linux: 501-1000",
                "10.rebuild-linux: 1001-2500",
                "10.rebuild-linux: 2501-5000",
                "10.rebuild-linux: 5001+"
            ]
        );

        let mut tagger = RebuildTagger::new();
        tagger.parse_attrs(PackageArchSrc::linux(10).and_darwin(10).into());
        assert_eq!(
            tagger.tags_to_add(),
            vec!["10.rebuild-darwin: 1-10", "10.rebuild-linux: 1-10",]
        );
        assert_eq!(
            tagger.tags_to_remove(),
            vec![
                "10.rebuild-darwin: 0",
                "10.rebuild-darwin: 1",
                "10.rebuild-darwin: 11-100",
                "10.rebuild-darwin: 101-500",
                "10.rebuild-darwin: 501+",
                "10.rebuild-darwin: 501-1000",
                "10.rebuild-darwin: 1001-2500",
                "10.rebuild-darwin: 2501-5000",
                "10.rebuild-darwin: 5001+",
                "10.rebuild-linux: 0",
                "10.rebuild-linux: 1",
                "10.rebuild-linux: 11-100",
                "10.rebuild-linux: 101-500",
                "10.rebuild-linux: 501+",
                "10.rebuild-linux: 501-1000",
                "10.rebuild-linux: 1001-2500",
                "10.rebuild-linux: 2501-5000",
                "10.rebuild-linux: 5001+"
            ]
        );

        let mut tagger = RebuildTagger::new();
        tagger.parse_attrs(PackageArchSrc::linux(11).and_darwin(11).into());
        assert_eq!(
            tagger.tags_to_add(),
            vec!["10.rebuild-darwin: 11-100", "10.rebuild-linux: 11-100",]
        );
        assert_eq!(
            tagger.tags_to_remove(),
            vec![
                "10.rebuild-darwin: 0",
                "10.rebuild-darwin: 1",
                "10.rebuild-darwin: 1-10",
                "10.rebuild-darwin: 101-500",
                "10.rebuild-darwin: 501+",
                "10.rebuild-darwin: 501-1000",
                "10.rebuild-darwin: 1001-2500",
                "10.rebuild-darwin: 2501-5000",
                "10.rebuild-darwin: 5001+",
                "10.rebuild-linux: 0",
                "10.rebuild-linux: 1",
                "10.rebuild-linux: 1-10",
                "10.rebuild-linux: 101-500",
                "10.rebuild-linux: 501+",
                "10.rebuild-linux: 501-1000",
                "10.rebuild-linux: 1001-2500",
                "10.rebuild-linux: 2501-5000",
                "10.rebuild-linux: 5001+"
            ]
        );

        let mut tagger = RebuildTagger::new();
        tagger.parse_attrs(PackageArchSrc::linux(100).and_darwin(100).into());
        assert_eq!(
            tagger.tags_to_add(),
            vec!["10.rebuild-darwin: 11-100", "10.rebuild-linux: 11-100",]
        );
        assert_eq!(
            tagger.tags_to_remove(),
            vec![
                "10.rebuild-darwin: 0",
                "10.rebuild-darwin: 1",
                "10.rebuild-darwin: 1-10",
                "10.rebuild-darwin: 101-500",
                "10.rebuild-darwin: 501+",
                "10.rebuild-darwin: 501-1000",
                "10.rebuild-darwin: 1001-2500",
                "10.rebuild-darwin: 2501-5000",
                "10.rebuild-darwin: 5001+",
                "10.rebuild-linux: 0",
                "10.rebuild-linux: 1",
                "10.rebuild-linux: 1-10",
                "10.rebuild-linux: 101-500",
                "10.rebuild-linux: 501+",
                "10.rebuild-linux: 501-1000",
                "10.rebuild-linux: 1001-2500",
                "10.rebuild-linux: 2501-5000",
                "10.rebuild-linux: 5001+"
            ]
        );

        let mut tagger = RebuildTagger::new();
        tagger.parse_attrs(PackageArchSrc::linux(101).and_darwin(101).into());
        assert_eq!(
            tagger.tags_to_add(),
            vec!["10.rebuild-darwin: 101-500", "10.rebuild-linux: 101-500",]
        );
        assert_eq!(
            tagger.tags_to_remove(),
            vec![
                "10.rebuild-darwin: 0",
                "10.rebuild-darwin: 1",
                "10.rebuild-darwin: 1-10",
                "10.rebuild-darwin: 11-100",
                "10.rebuild-darwin: 501+",
                "10.rebuild-darwin: 501-1000",
                "10.rebuild-darwin: 1001-2500",
                "10.rebuild-darwin: 2501-5000",
                "10.rebuild-darwin: 5001+",
                "10.rebuild-linux: 0",
                "10.rebuild-linux: 1",
                "10.rebuild-linux: 1-10",
                "10.rebuild-linux: 11-100",
                "10.rebuild-linux: 501+",
                "10.rebuild-linux: 501-1000",
                "10.rebuild-linux: 1001-2500",
                "10.rebuild-linux: 2501-5000",
                "10.rebuild-linux: 5001+"
            ]
        );

        let mut tagger = RebuildTagger::new();
        tagger.parse_attrs(PackageArchSrc::linux(500).and_darwin(500).into());
        assert_eq!(
            tagger.tags_to_add(),
            vec!["10.rebuild-darwin: 101-500", "10.rebuild-linux: 101-500",]
        );
        assert_eq!(
            tagger.tags_to_remove(),
            vec![
                "10.rebuild-darwin: 0",
                "10.rebuild-darwin: 1",
                "10.rebuild-darwin: 1-10",
                "10.rebuild-darwin: 11-100",
                "10.rebuild-darwin: 501+",
                "10.rebuild-darwin: 501-1000",
                "10.rebuild-darwin: 1001-2500",
                "10.rebuild-darwin: 2501-5000",
                "10.rebuild-darwin: 5001+",
                "10.rebuild-linux: 0",
                "10.rebuild-linux: 1",
                "10.rebuild-linux: 1-10",
                "10.rebuild-linux: 11-100",
                "10.rebuild-linux: 501+",
                "10.rebuild-linux: 501-1000",
                "10.rebuild-linux: 1001-2500",
                "10.rebuild-linux: 2501-5000",
                "10.rebuild-linux: 5001+"
            ]
        );

        let mut tagger = RebuildTagger::new();
        tagger.parse_attrs(PackageArchSrc::linux(501).and_darwin(501).into());
        assert_eq!(
            tagger.tags_to_add(),
            vec![
                "10.rebuild-darwin: 501+",
                "10.rebuild-darwin: 501-1000",
                "10.rebuild-linux: 501+",
                "10.rebuild-linux: 501-1000",
            ]
        );
        assert_eq!(
            tagger.tags_to_remove(),
            vec![
                "10.rebuild-darwin: 0",
                "10.rebuild-darwin: 1",
                "10.rebuild-darwin: 1-10",
                "10.rebuild-darwin: 11-100",
                "10.rebuild-darwin: 101-500",
                "10.rebuild-darwin: 1001-2500",
                "10.rebuild-darwin: 2501-5000",
                "10.rebuild-darwin: 5001+",
                "10.rebuild-linux: 0",
                "10.rebuild-linux: 1",
                "10.rebuild-linux: 1-10",
                "10.rebuild-linux: 11-100",
                "10.rebuild-linux: 101-500",
                "10.rebuild-linux: 1001-2500",
                "10.rebuild-linux: 2501-5000",
                "10.rebuild-linux: 5001+"
            ]
        );

        let mut tagger = RebuildTagger::new();
        tagger.parse_attrs(PackageArchSrc::linux(1000).and_darwin(1000).into());
        assert_eq!(
            tagger.tags_to_add(),
            vec![
                "10.rebuild-darwin: 501+",
                "10.rebuild-darwin: 501-1000",
                "10.rebuild-linux: 501+",
                "10.rebuild-linux: 501-1000",
            ]
        );
        assert_eq!(
            tagger.tags_to_remove(),
            vec![
                "10.rebuild-darwin: 0",
                "10.rebuild-darwin: 1",
                "10.rebuild-darwin: 1-10",
                "10.rebuild-darwin: 11-100",
                "10.rebuild-darwin: 101-500",
                "10.rebuild-darwin: 1001-2500",
                "10.rebuild-darwin: 2501-5000",
                "10.rebuild-darwin: 5001+",
                "10.rebuild-linux: 0",
                "10.rebuild-linux: 1",
                "10.rebuild-linux: 1-10",
                "10.rebuild-linux: 11-100",
                "10.rebuild-linux: 101-500",
                "10.rebuild-linux: 1001-2500",
                "10.rebuild-linux: 2501-5000",
                "10.rebuild-linux: 5001+"
            ]
        );

        let mut tagger = RebuildTagger::new();
        tagger.parse_attrs(PackageArchSrc::linux(1001).and_darwin(1001).into());
        assert_eq!(
            tagger.tags_to_add(),
            vec![
                "10.rebuild-darwin: 501+",
                "10.rebuild-darwin: 1001-2500",
                "10.rebuild-linux: 501+",
                "10.rebuild-linux: 1001-2500"
            ]
        );
        assert_eq!(
            tagger.tags_to_remove(),
            vec![
                "10.rebuild-darwin: 0",
                "10.rebuild-darwin: 1",
                "10.rebuild-darwin: 1-10",
                "10.rebuild-darwin: 11-100",
                "10.rebuild-darwin: 101-500",
                "10.rebuild-darwin: 501-1000",
                "10.rebuild-darwin: 2501-5000",
                "10.rebuild-darwin: 5001+",
                "10.rebuild-linux: 0",
                "10.rebuild-linux: 1",
                "10.rebuild-linux: 1-10",
                "10.rebuild-linux: 11-100",
                "10.rebuild-linux: 101-500",
                "10.rebuild-linux: 501-1000",
                "10.rebuild-linux: 2501-5000",
                "10.rebuild-linux: 5001+"
            ]
        );

        let mut tagger = RebuildTagger::new();
        tagger.parse_attrs(PackageArchSrc::linux(2500).and_darwin(2500).into());
        assert_eq!(
            tagger.tags_to_add(),
            vec![
                "10.rebuild-darwin: 501+",
                "10.rebuild-darwin: 1001-2500",
                "10.rebuild-linux: 501+",
                "10.rebuild-linux: 1001-2500"
            ]
        );
        assert_eq!(
            tagger.tags_to_remove(),
            vec![
                "10.rebuild-darwin: 0",
                "10.rebuild-darwin: 1",
                "10.rebuild-darwin: 1-10",
                "10.rebuild-darwin: 11-100",
                "10.rebuild-darwin: 101-500",
                "10.rebuild-darwin: 501-1000",
                "10.rebuild-darwin: 2501-5000",
                "10.rebuild-darwin: 5001+",
                "10.rebuild-linux: 0",
                "10.rebuild-linux: 1",
                "10.rebuild-linux: 1-10",
                "10.rebuild-linux: 11-100",
                "10.rebuild-linux: 101-500",
                "10.rebuild-linux: 501-1000",
                "10.rebuild-linux: 2501-5000",
                "10.rebuild-linux: 5001+"
            ]
        );

        let mut tagger = RebuildTagger::new();
        tagger.parse_attrs(PackageArchSrc::linux(2501).and_darwin(2501).into());
        assert_eq!(
            tagger.tags_to_add(),
            vec![
                "10.rebuild-darwin: 501+",
                "10.rebuild-darwin: 2501-5000",
                "10.rebuild-linux: 501+",
                "10.rebuild-linux: 2501-5000"
            ]
        );
        assert_eq!(
            tagger.tags_to_remove(),
            vec![
                "10.rebuild-darwin: 0",
                "10.rebuild-darwin: 1",
                "10.rebuild-darwin: 1-10",
                "10.rebuild-darwin: 11-100",
                "10.rebuild-darwin: 101-500",
                "10.rebuild-darwin: 501-1000",
                "10.rebuild-darwin: 1001-2500",
                "10.rebuild-darwin: 5001+",
                "10.rebuild-linux: 0",
                "10.rebuild-linux: 1",
                "10.rebuild-linux: 1-10",
                "10.rebuild-linux: 11-100",
                "10.rebuild-linux: 101-500",
                "10.rebuild-linux: 501-1000",
                "10.rebuild-linux: 1001-2500",
                "10.rebuild-linux: 5001+"
            ]
        );

        let mut tagger = RebuildTagger::new();
        tagger.parse_attrs(PackageArchSrc::linux(5000).and_darwin(5000).into());
        assert_eq!(
            tagger.tags_to_add(),
            vec![
                "10.rebuild-darwin: 501+",
                "10.rebuild-darwin: 2501-5000",
                "10.rebuild-linux: 501+",
                "10.rebuild-linux: 2501-5000"
            ]
        );
        assert_eq!(
            tagger.tags_to_remove(),
            vec![
                "10.rebuild-darwin: 0",
                "10.rebuild-darwin: 1",
                "10.rebuild-darwin: 1-10",
                "10.rebuild-darwin: 11-100",
                "10.rebuild-darwin: 101-500",
                "10.rebuild-darwin: 501-1000",
                "10.rebuild-darwin: 1001-2500",
                "10.rebuild-darwin: 5001+",
                "10.rebuild-linux: 0",
                "10.rebuild-linux: 1",
                "10.rebuild-linux: 1-10",
                "10.rebuild-linux: 11-100",
                "10.rebuild-linux: 101-500",
                "10.rebuild-linux: 501-1000",
                "10.rebuild-linux: 1001-2500",
                "10.rebuild-linux: 5001+"
            ]
        );

        let mut tagger = RebuildTagger::new();
        tagger.parse_attrs(PackageArchSrc::linux(5001).and_darwin(5001).into());
        assert_eq!(
            tagger.tags_to_add(),
            vec![
                "10.rebuild-darwin: 501+",
                "10.rebuild-darwin: 5001+",
                "10.rebuild-linux: 501+",
                "10.rebuild-linux: 5001+"
            ]
        );
        assert_eq!(
            tagger.tags_to_remove(),
            vec![
                "10.rebuild-darwin: 0",
                "10.rebuild-darwin: 1",
                "10.rebuild-darwin: 1-10",
                "10.rebuild-darwin: 11-100",
                "10.rebuild-darwin: 101-500",
                "10.rebuild-darwin: 501-1000",
                "10.rebuild-darwin: 1001-2500",
                "10.rebuild-darwin: 2501-5000",
                "10.rebuild-linux: 0",
                "10.rebuild-linux: 1",
                "10.rebuild-linux: 1-10",
                "10.rebuild-linux: 11-100",
                "10.rebuild-linux: 101-500",
                "10.rebuild-linux: 501-1000",
                "10.rebuild-linux: 1001-2500",
                "10.rebuild-linux: 2501-5000",
            ]
        );
    }
}
