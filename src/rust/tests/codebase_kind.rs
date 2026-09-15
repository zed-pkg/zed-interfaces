use zed_interfaces::{CodebaseKind, Manifest};

fn manifest_with(package_kind: &str, target_kind: Option<&str>) -> String {
    let target_kind = target_kind
        .map(|kind| format!("kind = \"{kind}\"\n"))
        .unwrap_or_default();
    format!(
        r#"[package]
org = "acme"
name = "polyglot-sdk"
version = "1.2.3"
kind = "{package_kind}"

[package.repository]
vcs = "git"
url = "https://github.com/acme/polyglot-sdk"

[targets.nodejs]
dir = "clients/node"
{target_kind}"#
    )
}

fn manifest_without_kind() -> &'static str {
    r#"[package]
org = "acme"
name = "legacy-lib"
version = "1.2.3"

[package.repository]
vcs = "git"
url = "https://github.com/acme/legacy-lib"

[targets.nodejs]
dir = "clients/node"
"#
}

#[test]
fn all_five_codebase_kinds_parse_and_round_trip() {
    for (token, expected) in [
        ("contracts", CodebaseKind::Contracts),
        ("lib", CodebaseKind::Lib),
        ("sdk", CodebaseKind::Sdk),
        ("server", CodebaseKind::Server),
        ("cli", CodebaseKind::Cli),
    ] {
        let manifest = Manifest::parse(&manifest_with(token, None)).expect("kind parses");
        assert_eq!(manifest.package.kind, Some(expected));
        assert_eq!(expected.as_str(), token);

        let encoded = manifest.to_toml_string().expect("manifest serializes");
        let decoded = Manifest::parse(&encoded).expect("round trip parses");
        assert_eq!(decoded.package.kind, Some(expected));
    }
}

#[test]
fn legacy_manifest_without_kind_remains_compatible() {
    let manifest = Manifest::parse(manifest_without_kind()).expect("legacy manifest parses");
    assert_eq!(manifest.package.kind, None);
    assert_eq!(manifest.effective_target_kind("nodejs"), None);

    let encoded = manifest.to_toml_string().expect("legacy manifest serializes");
    assert!(!encoded.contains("kind ="));
}

#[test]
fn unknown_codebase_kind_is_rejected() {
    let error = Manifest::parse(&manifest_with("service", None)).expect_err("unknown kind fails");
    assert!(error.to_string().contains("service"));
}

#[test]
fn target_inherits_package_kind_when_no_override_exists() {
    let manifest = Manifest::parse(&manifest_with("sdk", None)).expect("manifest parses");
    assert_eq!(
        manifest.effective_target_kind("nodejs"),
        Some(CodebaseKind::Sdk)
    );

    let derived = manifest
        .manifest_for_target("nodejs")
        .expect("target derives");
    assert_eq!(derived.package.kind, Some(CodebaseKind::Sdk));
}

#[test]
fn target_kind_override_wins_and_is_stamped_into_derived_manifest() {
    let manifest = Manifest::parse(&manifest_with("sdk", Some("cli"))).expect("manifest parses");
    assert_eq!(
        manifest.effective_target_kind("nodejs"),
        Some(CodebaseKind::Cli)
    );
    assert_eq!(
        manifest.effective_target_kind("node"),
        Some(CodebaseKind::Cli)
    );

    let derived = manifest
        .manifest_for_target("nodejs")
        .expect("target derives");
    assert_eq!(derived.package.kind, Some(CodebaseKind::Cli));
}
