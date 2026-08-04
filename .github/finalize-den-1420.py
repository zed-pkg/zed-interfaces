from pathlib import Path


def replace_once(path: Path, old: str, new: str, label: str) -> None:
    source = path.read_text(encoding="utf-8")
    count = source.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    path.write_text(source.replace(old, new, 1), encoding="utf-8")


source = Path("src/native_registry.rs")
replace_once(
    source,
    """        let mut platform_publications = BTreeMap::new();
        let mut meta_count = 0usize;
""",
    """        let mut platform_publications = BTreeMap::new();
        let mut meta_count = 0usize;
        let mut portable_count = 0usize;
""",
    "publication counters",
)
replace_once(
    source,
    """                NativePublicationKind::Portable => {}
""",
    """                NativePublicationKind::Portable => {
                    portable_count += 1;
                    if portable_count > 1 {
                        return Err(NativeRegistryError::MultiplePortablePackages);
                    }
                }
""",
    "portable cardinality",
)
replace_once(
    source,
    """        for publication in self
            .publications
            .iter()
            .filter(|publication| publication.kind == NativePublicationKind::Meta)
        {
            for selection in &publication.platform_packages {
                match platform_publications.get(&selection.platform) {
                    Some(package) if package == &selection.package => {}
                    Some(package) => {
                        return Err(NativeRegistryError::PlatformPackageMismatch {
                            meta_package: publication.package.name.clone(),
                            platform: selection.platform.selector(),
                            expected: package.clone(),
                            selected: selection.package.clone(),
                        });
                    }
                    None => {
                        return Err(NativeRegistryError::MissingPlatformPublication {
                            meta_package: publication.package.name.clone(),
                            platform: selection.platform.selector(),
                            selected: selection.package.clone(),
                        });
                    }
                }
            }
        }
""",
    """        for publication in self
            .publications
            .iter()
            .filter(|publication| publication.kind == NativePublicationKind::Meta)
        {
            if publication.platform_packages.is_empty() {
                return Err(NativeRegistryError::MetaPackageRequiresPlatformSelections {
                    package: publication.package.name.clone(),
                });
            }

            let mut selected_platforms = BTreeSet::new();
            for selection in &publication.platform_packages {
                selected_platforms.insert(selection.platform.clone());
                match platform_publications.get(&selection.platform) {
                    Some(package) if package == &selection.package => {}
                    Some(package) => {
                        return Err(NativeRegistryError::PlatformPackageMismatch {
                            meta_package: publication.package.name.clone(),
                            platform: selection.platform.selector(),
                            expected: package.clone(),
                            selected: selection.package.clone(),
                        });
                    }
                    None => {
                        return Err(NativeRegistryError::MissingPlatformPublication {
                            meta_package: publication.package.name.clone(),
                            platform: selection.platform.selector(),
                            selected: selection.package.clone(),
                        });
                    }
                }
            }

            for (platform, package) in &platform_publications {
                if !selected_platforms.contains(platform) {
                    return Err(NativeRegistryError::UnselectedPlatformPublication {
                        meta_package: publication.package.name.clone(),
                        platform: platform.selector(),
                        package: package.clone(),
                    });
                }
            }
        }
""",
    "meta coverage",
)
replace_once(
    source,
    """    #[error("one adapter record may contain at most one meta package")]
    MultipleMetaPackages,
""",
    """    #[error("one adapter record may contain at most one portable package")]
    MultiplePortablePackages,
    #[error("one adapter record may contain at most one meta package")]
    MultipleMetaPackages,
    #[error("meta publication `{package}` must select at least one platform package")]
    MetaPackageRequiresPlatformSelections { package: String },
""",
    "cardinality errors",
)
replace_once(
    source,
    """    #[error(
        "meta package `{meta_package}` selects missing platform package `{selected}` for `{platform}`"
    )]
    MissingPlatformPublication {
        meta_package: String,
        platform: String,
        selected: String,
    },
""",
    """    #[error(
        "meta package `{meta_package}` selects missing platform package `{selected}` for `{platform}`"
    )]
    MissingPlatformPublication {
        meta_package: String,
        platform: String,
        selected: String,
    },
    #[error(
        "meta package `{meta_package}` does not select published platform package `{package}` for `{platform}`"
    )]
    UnselectedPlatformPublication {
        meta_package: String,
        platform: String,
        package: String,
    },
""",
    "coverage error",
)
replace_once(
    source,
    """    #[test]
    fn cargo_and_npm_names_use_conservative_portable_subsets() {
""",
    """    #[test]
    fn publication_family_cardinality_and_meta_coverage_fail_closed() {
        let mut multiple_portable = record();
        multiple_portable.publications.extend([
            publication(
                "@fiducia/core-portable",
                NativePublicationKind::Portable,
                None,
                'd',
            ),
            publication(
                "@fiducia/core-portable-extra",
                NativePublicationKind::Portable,
                None,
                'e',
            ),
        ]);
        assert!(matches!(
            multiple_portable.validate(),
            Err(NativeRegistryError::MultiplePortablePackages)
        ));

        let mut empty_meta = record();
        empty_meta
            .publications
            .iter_mut()
            .find(|publication| publication.kind == NativePublicationKind::Meta)
            .unwrap()
            .platform_packages
            .clear();
        assert!(matches!(
            empty_meta.validate(),
            Err(NativeRegistryError::MetaPackageRequiresPlatformSelections { .. })
        ));

        let mut incomplete_meta = record();
        incomplete_meta
            .publications
            .iter_mut()
            .find(|publication| publication.kind == NativePublicationKind::Meta)
            .unwrap()
            .platform_packages
            .pop();
        assert!(matches!(
            incomplete_meta.validate(),
            Err(NativeRegistryError::UnselectedPlatformPublication { .. })
        ));

        let mut platform_only = record();
        platform_only
            .publications
            .retain(|publication| publication.kind == NativePublicationKind::Platform);
        assert!(platform_only.validate().is_ok());
    }

    #[test]
    fn cargo_and_npm_names_use_conservative_portable_subsets() {
""",
    "topology tests",
)

docs = Path("docs/native-registry-contract.md")
replace_once(
    docs,
    """A publication family may contain:

- one portable package;
- one generic meta package whose platform edges reference packages in the same
  record; and
- one package for each unique platform selector.
""",
    """A publication family may contain:

- at most one portable package;
- at most one generic meta package, with at least one platform edge; and
- one package for each unique platform selector.

When a meta package is present it must select every platform publication in the
record exactly once. Platform-only families remain valid when consumers select
native packages directly without a generic wrapper.
""",
    "contract topology prose",
)
