use zed_interfaces::ArtifactFormat;
use zed_interfaces::excludes::{ALWAYS_INCLUDE, DEFAULT_EXCLUDES, effective_excludes};
use zed_interfaces::lockfile::{LockedPackage, Lockfile};
use zed_interfaces::manifest::{Manifest, ManifestError};
use zed_interfaces::paths::store_entry_rel;
use zed_interfaces::vcs::Vcs;

const SAMPLE: &str = r#"
[package]
org = "acme"
name = "http-kit"
version = "1.2.0"
description = "Tiny HTTP helpers"
license = "MIT"

[package.repository]
vcs = "git"
url = "https://github.com/acme/http-kit"

[dependencies]
"acme/logkit" = "^0.3"

[publish]
exclude = ["benches/**"]
smoke_test = "sh scripts/smoke.sh"

[scripts]
test = "make test"
"#;

#[test]
fn manifest_roundtrip() {
    let m = Manifest::parse(SAMPLE).unwrap();
    assert_eq!(m.full_name(), "acme/http-kit");
    assert_eq!(m.package.repository.vcs, Vcs::Git);
    assert_eq!(m.vcs_tag(), "v1.2.0");
    assert_eq!(m.publish.exclude, vec!["benches/**".to_string()]);
    assert!(!m.publish.include_readme);
    assert_eq!(m.scripts.test.as_deref(), Some("make test"));

    let serialized = m.to_toml_string().unwrap();
    let reparsed = Manifest::parse(&serialized).unwrap();
    assert_eq!(m, reparsed);
}

#[test]
fn install_dir_defaults_and_overrides() {
    // No [install] section -> the default dep dir.
    assert_eq!(
        Manifest::parse(SAMPLE).unwrap().modules_dir(),
        "zed_modules"
    );

    // A configured dir relocates the tree and round-trips.
    let with_dir = format!("{SAMPLE}\n[install]\ndir = \".vendor/.zed\"\n");
    let m = Manifest::parse(&with_dir).unwrap();
    assert_eq!(m.modules_dir(), ".vendor/.zed");
    assert_eq!(Manifest::parse(&m.to_toml_string().unwrap()).unwrap(), m);

    // Unsafe dirs are rejected.
    for bad in ["/abs/path", "../escape", "a/../../b"] {
        let src = format!("{SAMPLE}\n[install]\ndir = \"{bad}\"\n");
        assert!(
            matches!(
                Manifest::parse(&src),
                Err(ManifestError::InvalidInstallDir(_, _))
            ),
            "expected {bad} rejected"
        );
    }
}

#[test]
fn manifest_rejects_bad_input() {
    let bad_org = SAMPLE.replace("org = \"acme\"", "org = \"Acme!\"");
    assert!(matches!(
        Manifest::parse(&bad_org),
        Err(ManifestError::InvalidOrg(_))
    ));

    let bad_dep = SAMPLE.replace("\"acme/logkit\"", "\"logkit\"");
    assert!(matches!(
        Manifest::parse(&bad_dep),
        Err(ManifestError::InvalidDependencyKey(_))
    ));

    let bad_version = SAMPLE.replace("version = \"1.2.0\"", "version = \"not-semver\"");
    assert!(matches!(
        Manifest::parse(&bad_version),
        Err(ManifestError::InvalidVersion(_, _))
    ));
}

#[test]
fn lockfile_roundtrip() {
    let mut lock = Lockfile::default();
    lock.upsert(LockedPackage {
        org: "acme".into(),
        name: "http-kit".into(),
        version: "1.2.0".into(),
        sha256: "ab".repeat(32),
        size: 4096,
        format: ArtifactFormat::TarGz,
        vcs_tag: "v1.2.0".into(),
        vcs_commit: Some("deadbeef".into()),
        source: "https://registry.zed-pkg.dev".into(),
    });

    let text = lock.to_toml_string().unwrap();
    assert!(text.contains("[[package]]"));
    let reparsed = Lockfile::parse(&text).unwrap();
    assert_eq!(lock, reparsed);
    assert!(reparsed.find("acme", "http-kit").is_some());
}

#[test]
fn excludes_respect_include_readme() {
    let with_readme_stripped = effective_excludes(&[], false);
    assert!(with_readme_stripped.iter().any(|p| p == "README*"));
    assert!(with_readme_stripped.iter().any(|p| p == "tests/**"));

    let readme_kept = effective_excludes(&["extra/**".to_string()], true);
    assert!(!readme_kept.iter().any(|p| p == "README*"));
    assert!(readme_kept.iter().any(|p| p == "extra/**"));

    assert!(DEFAULT_EXCLUDES.contains(&".github/**"));
    assert!(ALWAYS_INCLUDE.contains(&"LICENSE*"));
    assert!(ALWAYS_INCLUDE.contains(&".zpkg.toml"));
}

#[test]
fn store_paths_are_sharded() {
    let sha = "abcdef0123".to_string() + &"0".repeat(54);
    assert_eq!(store_entry_rel(&sha), format!("store/v1/ab/{sha}"));
}
