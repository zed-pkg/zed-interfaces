use std::collections::BTreeSet;

use zed_interfaces::artifact::ArtifactFormat;
use zed_interfaces::lockfile::{LockedPackage, Lockfile};

const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn package(revision: Option<&str>) -> LockedPackage {
    LockedPackage {
        org: "zed-pkg".to_string(),
        name: "fixture".to_string(),
        version: "1.2.3".to_string(),
        sha256: DIGEST.to_string(),
        size: 42,
        format: ArtifactFormat::TarGz,
        vcs_tag: "v1.2.3".to_string(),
        vcs_commit: revision.map(str::to_string),
        source: "file:///tmp/registry".to_string(),
    }
}

#[test]
fn canonical_writer_upgrades_legacy_none_without_mutating_the_builder() {
    let lock = Lockfile {
        version: Lockfile::CURRENT_VERSION,
        packages: vec![package(None)],
        native_dependencies: Vec::new(),
        nix_adapters: Vec::new(),
    };

    let rendered = lock.to_toml_string().unwrap();
    let expected = format!("artifact-sha256:{DIGEST}");
    assert!(rendered.contains(&format!("vcs_commit = \"{expected}\"")));
    assert_eq!(lock.packages[0].vcs_commit, None);

    let parsed = Lockfile::parse(&rendered).unwrap();
    assert_eq!(
        parsed.packages[0].vcs_commit.as_deref(),
        Some(expected.as_str())
    );
}

#[test]
fn canonical_writer_preserves_a_stronger_verified_revision() {
    let revision = "fedcba9876543210fedcba9876543210fedcba98";
    let lock = Lockfile {
        version: Lockfile::CURRENT_VERSION,
        packages: vec![package(Some(revision))],
        native_dependencies: Vec::new(),
        nix_adapters: Vec::new(),
    };

    let rendered = lock.to_toml_string().unwrap();
    let parsed = Lockfile::parse(&rendered).unwrap();
    assert_eq!(parsed.packages[0].vcs_commit.as_deref(), Some(revision));
    assert!(!rendered.contains("artifact-sha256:"));
}

#[test]
fn parser_still_rejects_an_omitted_committed_revision() {
    let input = format!(
        r#"version = 1

[[package]]
org = "zed-pkg"
name = "fixture"
version = "1.2.3"
sha256 = "{DIGEST}"
size = 42
format = "tar.gz"
vcs_tag = "v1.2.3"
source = "file:///tmp/registry"
"#
    );
    let error = Lockfile::parse(&input).unwrap_err().to_string();
    assert!(error.contains("vcs_commit"), "unexpected error: {error}");
}

#[test]
fn public_schema_keeps_revision_required_despite_the_builder_fallback() {
    let schema = schemars::schema_for!(Lockfile);
    let value = serde_json::to_value(schema).unwrap();
    let required = value["$defs"]["LockedPackage"]["required"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry.as_str())
        .collect::<BTreeSet<_>>();
    assert!(required.contains("format"));
    assert!(required.contains("vcs_commit"));
}

#[test]
fn fallback_derivation_rejects_malformed_or_zero_hashes() {
    let digests = ["bad".to_string(), "0".repeat(64)];
    for digest in digests {
        let mut bad = package(None);
        bad.sha256 = digest;
        let lock = Lockfile {
            version: Lockfile::CURRENT_VERSION,
            packages: vec![bad],
            native_dependencies: Vec::new(),
            nix_adapters: Vec::new(),
        };
        let error = lock.to_toml_string().unwrap_err().to_string();
        assert!(error.contains("sha256"), "unexpected error: {error}");
    }
}
