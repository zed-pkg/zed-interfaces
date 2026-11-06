//! End-to-end manifest parsing contracts for canonical AttoLang source packages.
//! Backend labels are implementation choices, never interchangeable with the
//! language/ecosystem carried by a Zed package's public manifest.

use std::collections::BTreeSet;

use zed_interfaces::language::{Ecosystem, Language, detect_ecosystems};
use zed_interfaces::manifest::Manifest;

const SOURCE_MANIFEST: &str = r#"
[package]
org = "zed-e2e"
name = "attolang-source-bridge"
version = "0.1.0"
license = "MIT"
language = "attolang"

[package.repository]
vcs = "git"
url = "https://example.invalid/zed-e2e/attolang-source-bridge.git"
"#;

#[test]
fn public_attolang_manifest_is_typed_and_round_trips_canonically() {
    let manifest = Manifest::parse(SOURCE_MANIFEST).expect("canonical AttoLang package manifest");
    assert_eq!(manifest.package.language, Language::Attolang);
    assert_eq!(manifest.package.ecosystem(), Ecosystem::Attolang);
    assert_eq!(manifest.full_name(), "zed-e2e/attolang-source-bridge");

    let serialized = manifest
        .to_toml_string()
        .expect("serialize public manifest");
    assert!(
        serialized.contains("language = \"attolang\""),
        "{serialized}"
    );
    assert_eq!(
        Manifest::parse(&serialized).expect("reparse canonical manifest"),
        manifest
    );
}

#[test]
fn legacy_oreslang_and_display_case_are_canonicalized_not_ungated() {
    for token in ["attolang", "AttoLang", "oreslang", "ORESlang"] {
        let source = SOURCE_MANIFEST.replace(
            "language = \"attolang\"",
            &format!("language = \"{token}\""),
        );
        let manifest = Manifest::parse(&source)
            .unwrap_or_else(|error| panic!("alias {token} must parse: {error}"));
        assert_eq!(manifest.package.language, Language::Attolang);
        assert_eq!(manifest.package.ecosystem(), Ecosystem::Attolang);
        let normalized = manifest.to_toml_string().expect("serialize legacy alias");
        assert!(
            normalized.contains("language = \"attolang\""),
            "{normalized}"
        );
        assert!(
            !normalized.contains("language = \"oreslang\""),
            "{normalized}"
        );
    }

    let explicit_legacy = SOURCE_MANIFEST.replace(
        "language = \"attolang\"",
        "language = \"attolang\"\necosystem = \"oreslang\"",
    );
    let manifest = Manifest::parse(&explicit_legacy).expect("legacy ecosystem alias");
    assert_eq!(manifest.package.ecosystem(), Ecosystem::Attolang);
    assert!(
        manifest
            .to_toml_string()
            .expect("canonical ecosystem")
            .contains("ecosystem = \"attolang\"")
    );
}

#[test]
fn backend_names_cannot_be_misinterpreted_as_atto_package_languages() {
    for implementation in ["llvm", "graalvm", "nativeaot", "cranelift", "forge"] {
        let source = SOURCE_MANIFEST.replace(
            "language = \"attolang\"",
            &format!("language = \"{implementation}\""),
        );
        let error = Manifest::parse(&source)
            .expect_err("a compiler backend is not a package-language token");
        assert!(
            error.to_string().contains("unknown language"),
            "{implementation}: {error}"
        );
        assert_eq!(Language::from_token(implementation), None);
    }

    // "wasm" predates AttoLang as an explicit alias for RustWasm packages.
    // Preserve that public compatibility: it maps to npm, NOT to AttoLang's
    // compiler-source import ecosystem, so it cannot masquerade as .atto.
    let wasm_source = SOURCE_MANIFEST.replace(
        "language = \"attolang\"",
        "language = \"wasm\"",
    );
    let wasm = Manifest::parse(&wasm_source).expect("existing RustWasm alias");
    assert_eq!(wasm.package.language, Language::RustWasm);
    assert_eq!(wasm.package.ecosystem(), Ecosystem::Npm);
    assert_ne!(wasm.package.ecosystem(), Ecosystem::Attolang);
}

#[test]
fn unrecognized_atto_ecosystem_cannot_silently_become_universal() {
    let source = SOURCE_MANIFEST.replace(
        "language = \"attolang\"",
        "language = \"attolang\"\necosystem = \"attolang-native-llvm\"",
    );
    assert!(
        Manifest::parse(&source).is_err(),
        "unknown ecosystem must reject instead of disabling source-import guards"
    );
}

#[test]
fn source_project_marker_coexists_with_host_toolchain_ecosystems() {
    assert_eq!(
        detect_ecosystems([".ores.pkg.toml", "Cargo.toml", "package.json"]),
        BTreeSet::from([Ecosystem::Attolang, Ecosystem::Cargo, Ecosystem::Npm])
    );
    assert_eq!(
        detect_ecosystems([".ores.pkg.toml.bak", ".ores.pkg.tomlx", ".ores.pkg"]),
        BTreeSet::new()
    );
}
