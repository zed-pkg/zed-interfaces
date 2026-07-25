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

/// A polyglot package declares one subtree per ecosystem; consumers pick one.
const POLYGLOT: &str = r#"
[package]
org = "zedtest"
name = "polyglot-lib"
version = "1.0.0"

[package.repository]
vcs = "git"
url = "https://github.com/zed-pkg-test/polyglot-lib"

[targets.node]
dir = "node"

[targets.python]
dir = "python"

[targets.go]
dir = "go"
"#;

#[test]
fn polyglot_targets_roundtrip_and_resolve() {
    let m = Manifest::parse(POLYGLOT).unwrap();
    assert!(m.is_polyglot());
    assert_eq!(m.targets.len(), 3);
    assert_eq!(m.target_subdir(Some("python")).unwrap(), Some("python"));
    assert_eq!(m.target_subdir(Some("node")).unwrap(), Some("node"));
    // Round-trips through TOML unchanged.
    assert_eq!(Manifest::parse(&m.to_toml_string().unwrap()).unwrap(), m);
}

#[test]
fn a_single_language_package_ignores_target_selection() {
    // No [targets] => always the whole tree, even if a consumer asks for one.
    // Existing packages must keep installing exactly as before.
    let m = Manifest::parse(SAMPLE).unwrap();
    assert!(!m.is_polyglot());
    assert_eq!(m.target_subdir(None).unwrap(), None);
    assert_eq!(m.target_subdir(Some("python")).unwrap(), None);
}

#[test]
fn a_polyglot_package_without_a_request_yields_the_whole_tree() {
    // A consumer that has not opted into a target still installs fine.
    let m = Manifest::parse(POLYGLOT).unwrap();
    assert_eq!(m.target_subdir(None).unwrap(), None);
}

#[test]
fn requesting_an_unpublished_target_is_an_error_listing_what_exists() {
    let m = Manifest::parse(POLYGLOT).unwrap();
    let err = m
        .target_subdir(Some("ruby"))
        .expect_err("a target the package does not publish must not silently fall back");
    let msg = err.to_string();
    assert!(msg.contains("ruby"), "{msg}");
    assert!(msg.contains("zedtest/polyglot-lib"), "{msg}");
    // The message enumerates the real targets so the fix is obvious.
    for target in ["go", "node", "python"] {
        assert!(msg.contains(target), "expected `{target}` listed in: {msg}");
    }
}

#[test]
fn target_dirs_and_names_are_validated() {
    // `..` in a target dir would escape the package on install.
    let escaping = POLYGLOT.replace(r#"dir = "python""#, r#"dir = "../../etc""#);
    assert!(matches!(
        Manifest::parse(&escaping),
        Err(ManifestError::InvalidTarget(_, _))
    ));

    // Absolute dirs likewise.
    let absolute = POLYGLOT.replace(r#"dir = "python""#, r#"dir = "/etc/passwd""#);
    assert!(matches!(
        Manifest::parse(&absolute),
        Err(ManifestError::InvalidTarget(_, _))
    ));

    // Target names are slugs.
    let bad_name = POLYGLOT.replace("[targets.node]", "[targets.\"Node JS\"]");
    assert!(matches!(
        Manifest::parse(&bad_name),
        Err(ManifestError::InvalidTarget(_, _))
    ));

    // And so is a consumer's requested target.
    let bad_request = format!("{SAMPLE}\n[install]\ntarget = \"Python 3\"\n");
    assert!(matches!(
        Manifest::parse(&bad_request),
        Err(ManifestError::InvalidTarget(_, _))
    ));
}

#[test]
fn nested_target_dirs_are_allowed() {
    // Real repos often nest, e.g. clients/go.
    let nested = POLYGLOT.replace(r#"dir = "go""#, r#"dir = "clients/go""#);
    let m = Manifest::parse(&nested).unwrap();
    assert_eq!(m.target_subdir(Some("go")).unwrap(), Some("clients/go"));
}

#[test]
fn consumer_requested_target_is_read_from_the_install_section() {
    let consumer = format!("{SAMPLE}\n[install]\ndir = \".vendor/.zed\"\ntarget = \"python\"\n");
    let m = Manifest::parse(&consumer).unwrap();
    assert_eq!(m.requested_target(), Some("python"));
    assert_eq!(m.modules_dir(), ".vendor/.zed");
    assert_eq!(Manifest::parse(&m.to_toml_string().unwrap()).unwrap(), m);

    // Absent or blank = no request.
    assert_eq!(Manifest::parse(SAMPLE).unwrap().requested_target(), None);
    let blank = format!("{SAMPLE}\n[install]\ntarget = \"  \"\n");
    assert_eq!(Manifest::parse(&blank).unwrap().requested_target(), None);
}
