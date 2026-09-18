//! The `[tool-dependencies]` / `[[tool]]` contract (zed-docs 36).
//!
//! A declared tool is a package this project pins but deliberately does not
//! materialize: the bytes live once per version in a central store, not once
//! per project. Two properties carry that whole design, and both live here
//! rather than in the CLI, because the CLI is only one consumer of the
//! contract:
//!
//!   * a tool pin is held to exactly the integrity standard a package pin is,
//!     because it is the same kind of immutable artifact; and
//!   * a tool pin is *invisible* to everything that materializes, because it
//!     lives in its own array and nothing that links reads that array.

use zed_interfaces::artifact::ArtifactFormat;
use zed_interfaces::lockfile::{LockedPackage, Lockfile, LockfileError};
use zed_interfaces::manifest::{Manifest, ManifestError};

const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const OTHER_DIGEST: &str = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210";
const REVISION: &str = "fedcba9876543210fedcba9876543210fedcba98";

/// One mutation applied to an otherwise valid tool pin.
type Mutation = Box<dyn Fn(&mut LockedPackage)>;

fn locked(name: &str, version: &str, sha256: &str) -> LockedPackage {
    LockedPackage {
        org: "acme".to_string(),
        name: name.to_string(),
        version: version.to_string(),
        sha256: sha256.to_string(),
        size: 42,
        format: ArtifactFormat::TarGz,
        vcs_tag: format!("v{version}"),
        vcs_commit: Some(REVISION.to_string()),
        source: "file:///tmp/registry".to_string(),
    }
}

fn manifest_with(sections: &str) -> Result<Manifest, ManifestError> {
    Manifest::parse(&format!(
        r#"
[package]
org = "acme"
name = "web-app"
version = "1.4.0"

[package.repository]
vcs = "git"
url = "https://github.com/acme/web-app"
{sections}
"#
    ))
}

// ---------------------------------------------------------------------------
// Manifest

#[test]
fn the_canonical_key_and_its_snake_case_alias_both_read() {
    for spelling in ["tool-dependencies", "tool_dependencies"] {
        let manifest = manifest_with(&format!("\n[{spelling}]\n\"acme/lint\" = \"^9\"\n"))
            .unwrap_or_else(|error| panic!("{spelling}: {error}"));
        assert_eq!(
            manifest
                .tool_dependencies
                .get("acme/lint")
                .map(String::as_str),
            Some("^9"),
            "{spelling}"
        );
    }
}

#[test]
fn tool_dependencies_round_trip_through_the_canonical_spelling() {
    let manifest = manifest_with("\n[tool_dependencies]\n\"acme/lint\" = \"^9\"\n").unwrap();
    let rendered = manifest.to_toml_string().unwrap();
    assert!(
        rendered.contains("[tool-dependencies]"),
        "the writer emits one canonical spelling: {rendered}"
    );
    assert_eq!(Manifest::parse(&rendered).unwrap(), manifest);
}

#[test]
fn an_absent_table_is_absent_from_the_output() {
    let manifest = manifest_with("").unwrap();
    assert!(manifest.tool_dependencies.is_empty());
    assert!(
        !manifest
            .to_toml_string()
            .unwrap()
            .contains("tool-dependencies")
    );
}

#[test]
fn a_package_is_either_linked_or_run_but_never_both() {
    let error = manifest_with(
        "\n[dependencies]\n\"acme/lint\" = \"^9\"\n\n[tool-dependencies]\n\"acme/lint\" = \"^9\"\n",
    )
    .unwrap_err();
    assert!(
        matches!(&error, ManifestError::ConflictingToolDependency(key) if key == "acme/lint"),
        "unexpected error: {error}"
    );
}

#[test]
fn a_build_dependency_and_a_tool_dependency_may_share_a_name() {
    // Only `[dependencies]` conflicts: a build dependency is scoped to this
    // package's own `[build]` sandbox and never reaches `zed run`, so there is
    // no name for the two to fight over.
    let manifest = manifest_with(
        "\n[build-dependencies]\n\"acme/lint\" = \"^9\"\n\n[tool-dependencies]\n\"acme/lint\" = \"^9\"\n",
    )
    .unwrap();
    assert!(manifest.build_dependencies.contains_key("acme/lint"));
    assert!(manifest.tool_dependencies.contains_key("acme/lint"));
}

#[test]
fn tool_requirements_and_keys_are_validated_like_every_other_dependency() {
    let bad_requirement =
        manifest_with("\n[tool-dependencies]\n\"acme/lint\" = \"^not-a-range\"\n").unwrap_err();
    assert!(
        matches!(&bad_requirement, ManifestError::InvalidDependencyReq(key, ..) if key == "acme/lint"),
        "unexpected error: {bad_requirement}"
    );

    let empty_requirement =
        manifest_with("\n[tool-dependencies]\n\"acme/lint\" = \"\"\n").unwrap_err();
    assert!(matches!(
        empty_requirement,
        ManifestError::InvalidDependencyReq(..)
    ));

    let bad_key = manifest_with("\n[tool-dependencies]\n\"lint\" = \"^9\"\n").unwrap_err();
    assert!(
        matches!(&bad_key, ManifestError::InvalidDependencyKey(key) if key == "lint"),
        "unexpected error: {bad_key}"
    );
}

// ---------------------------------------------------------------------------
// Lockfile

#[test]
fn a_tool_pin_is_never_visible_to_anything_that_materializes() {
    let mut lock = Lockfile::default();
    lock.upsert(locked("http-kit", "1.2.4", DIGEST));
    lock.upsert_tool(locked("lint", "9.12.0", OTHER_DIGEST));

    assert_eq!(lock.packages.len(), 1, "the tool is not in `packages`");
    assert_eq!(lock.packages[0].name, "http-kit");
    assert!(lock.find("acme", "lint").is_none());
    assert_eq!(
        lock.find_tool("acme", "lint").map(|t| t.version.as_str()),
        Some("9.12.0")
    );
    assert!(lock.find_tool("acme", "http-kit").is_none());

    let rendered = lock.to_toml_string().unwrap();
    assert!(rendered.contains("[[package]]"));
    assert!(rendered.contains("[[tool]]"));
    let parsed = Lockfile::parse(&rendered).unwrap();
    assert_eq!(parsed.packages, lock.packages);
    assert_eq!(parsed.tools, lock.tools);
}

#[test]
fn upsert_tool_replaces_in_place_and_keeps_the_array_sorted() {
    let mut lock = Lockfile::default();
    lock.upsert_tool(locked("lint", "9.12.0", DIGEST));
    lock.upsert_tool(locked("fmt", "2.4.1", OTHER_DIGEST));
    assert_eq!(
        lock.tools
            .iter()
            .map(|t| t.name.as_str())
            .collect::<Vec<_>>(),
        ["fmt", "lint"]
    );

    lock.upsert_tool(locked("lint", "9.13.0", DIGEST));
    assert_eq!(lock.tools.len(), 2, "one entry per identity");
    assert_eq!(lock.find_tool("acme", "lint").unwrap().version, "9.13.0");
}

#[test]
fn a_lockfile_without_tools_stays_byte_compatible() {
    let mut lock = Lockfile::default();
    lock.upsert(locked("http-kit", "1.2.4", DIGEST));
    let rendered = lock.to_toml_string().unwrap();
    assert!(
        !rendered.contains("[[tool]]"),
        "an empty array must not appear: {rendered}"
    );
    assert!(Lockfile::parse(&rendered).unwrap().tools.is_empty());
}

#[test]
fn one_identity_cannot_be_both_an_installed_package_and_a_tool() {
    let mut lock = Lockfile::default();
    lock.upsert(locked("lint", "9.12.0", DIGEST));
    lock.upsert_tool(locked("lint", "9.12.0", DIGEST));
    let error = lock.to_toml_string().unwrap_err();
    assert!(
        matches!(&error, LockfileError::ConflictingToolIdentity(key) if key == "acme/lint"),
        "unexpected error: {error}"
    );
}

#[test]
fn duplicate_tool_identities_are_rejected_on_read() {
    let entry = r#"
[[tool]]
org = "acme"
name = "lint"
version = "9.12.0"
sha256 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
size = 42
format = "tar.gz"
vcs_tag = "v9.12.0"
vcs_commit = "fedcba9876543210fedcba9876543210fedcba98"
source = "file:///tmp/registry"
"#;
    let error = Lockfile::parse(&format!("version = 1\n{entry}{entry}")).unwrap_err();
    assert!(
        matches!(&error, LockfileError::DuplicateTool(key) if key == "acme/lint"),
        "unexpected error: {error}"
    );
}

#[test]
fn a_tool_pin_is_held_to_the_same_integrity_standard_as_a_package_pin() {
    // Each mutation is one thing a package pin may not do either. A tool that
    // could be pinned to a mutable ref, a zero digest, or an empty artifact
    // would be a hole straight through the store's identity guarantees.
    let cases: Vec<(&str, Mutation)> = vec![
        (
            "all-zero digest",
            Box::new(|t: &mut LockedPackage| t.sha256 = "0".repeat(64)),
        ),
        (
            "short digest",
            Box::new(|t: &mut LockedPackage| t.sha256 = "abc".to_string()),
        ),
        ("zero size", Box::new(|t: &mut LockedPackage| t.size = 0)),
        (
            "empty tag",
            Box::new(|t: &mut LockedPackage| t.vcs_tag = String::new()),
        ),
        (
            "mutable revision",
            Box::new(|t: &mut LockedPackage| t.vcs_commit = Some("main".to_string())),
        ),
        (
            "empty source",
            Box::new(|t: &mut LockedPackage| t.source = String::new()),
        ),
        (
            "upper-case org",
            Box::new(|t: &mut LockedPackage| t.org = "Acme".to_string()),
        ),
    ];

    for (label, mutate) in cases {
        let mut tool = locked("lint", "9.12.0", DIGEST);
        mutate(&mut tool);
        let mut lock = Lockfile::default();
        lock.tools.push(tool);
        assert!(
            lock.to_toml_string().is_err(),
            "a tool pin with {label} must be rejected"
        );
    }
}
