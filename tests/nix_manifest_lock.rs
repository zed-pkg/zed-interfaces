use zed_interfaces::{
    ArtifactFormat, Lockfile, Manifest, NixAdapterRecord, NixBuilderNetwork, NixInteropArtifact,
    NixOutputOrigin, NixPackageIdentity, NixPolicyEvidence, NixPolicyProfile, NixRealizedOutput,
};

const HEX_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HEX_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const NAR_A: &str = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
const STORE_A: &str = "/nix/store/00000000000000000000000000000000-tool-1.2.3";

fn strict_policy() -> NixPolicyEvidence {
    NixPolicyEvidence {
        profile: NixPolicyProfile::StrictV1,
        pure_evaluation: true,
        import_from_derivation: false,
        sandbox_required: true,
        builder_network: NixBuilderNetwork::Disabled,
        dirty_source: false,
        publishable: true,
    }
}

fn adapter(name: &str, system: &str) -> NixAdapterRecord {
    NixAdapterRecord::nix_to_zed(
        NixPackageIdentity {
            org: "acme".to_string(),
            name: name.to_string(),
            version: "1.2.3".to_string(),
            target: None,
        },
        NixOutputOrigin {
            locked_ref: format!("github:acme/{name}/{HEX_A}"),
            flake_lock_sha256: HEX_A.to_string(),
            attribute: format!("packages.{system}.{name}"),
            realized: NixRealizedOutput {
                system: system.to_string(),
                output: "out".to_string(),
                derivation_json_sha256: HEX_B.to_string(),
                store_path: STORE_A.to_string(),
                nar_hash: NAR_A.to_string(),
                nar_size: 512,
                references: Vec::new(),
                signatures: vec!["cache.example-1:signature".to_string()],
                nix_version: "2.35.2".to_string(),
                store_info_json_version: 3,
            },
        },
        NixInteropArtifact {
            format: ArtifactFormat::TarGz,
            sha256: HEX_B.to_string(),
            size: 1024,
        },
        strict_policy(),
    )
}

#[test]
fn single_package_nix_export_intent_round_trips_and_plans() {
    let manifest = Manifest::parse(
        r#"
[package]
org = "acme"
name = "tool"
version = "1.2.3"

[package.repository]
url = "https://github.com/acme/tool"

[publish.nix]
mode = "artifact"
attribute = "acme-tool"
systems = ["x86_64-linux", "aarch64-linux"]
outputs = ["out"]
"#,
    )
    .unwrap();

    let routes = manifest.nix_export_routes();
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].target, "repository");
    assert_eq!(routes[0].dir, ".");
    assert_eq!(routes[0].package, "tool");
    assert_eq!(routes[0].intent.resolved_attribute("tool"), "acme-tool");

    let encoded = manifest.to_toml_string().unwrap();
    assert_eq!(Manifest::parse(&encoded).unwrap(), manifest);
}

#[test]
fn a_polyglot_target_carries_its_nix_intent_into_the_rerooted_manifest() {
    let manifest = Manifest::parse(
        r#"
[package]
org = "acme"
name = "sdk"
version = "1.2.3"

[package.repository]
url = "https://github.com/acme/sdk"

[targets.rust]
dir = "clients/rust"

[targets.rust.nix]
systems = ["x86_64-linux"]
outputs = ["out"]
"#,
    )
    .unwrap();

    let routes = manifest.nix_export_routes();
    assert_eq!(routes.len(), 1);
    assert_eq!(routes[0].target, "rust");
    assert_eq!(routes[0].package, "sdk-rust");

    let rerooted = manifest.manifest_for_target("rust").unwrap();
    assert!(rerooted.targets.is_empty());
    assert!(rerooted.publish.nix.is_some());
    assert_eq!(rerooted.nix_export_routes()[0].package, "sdk-rust");
}

#[test]
fn root_nix_intent_is_rejected_for_a_polyglot_manifest() {
    let error = Manifest::parse(
        r#"
[package]
org = "acme"
name = "sdk"
version = "1.2.3"

[package.repository]
url = "https://github.com/acme/sdk"

[publish.nix]
systems = ["x86_64-linux"]
outputs = ["out"]

[targets.rust]
dir = "clients/rust"
"#,
    )
    .unwrap_err();

    assert!(error.to_string().contains("[targets.<language>.nix]"));
}

#[test]
fn duplicate_effective_nix_attributes_are_rejected() {
    let error = Manifest::parse(
        r#"
[package]
org = "acme"
name = "sdk"
version = "1.2.3"

[package.repository]
url = "https://github.com/acme/sdk"

[targets.node]
dir = "clients/node"

[targets.node.nix]
attribute = "sdk-client"
systems = ["x86_64-linux"]
outputs = ["out"]

[targets.rust]
dir = "clients/rust"

[targets.rust.nix]
attribute = "sdk-client"
systems = ["x86_64-linux"]
outputs = ["out"]
"#,
    )
    .unwrap_err();

    assert!(error.to_string().contains("already used by target"));
}

#[test]
fn old_manifests_and_lockfiles_remain_valid_without_nix_metadata() {
    let manifest = Manifest::parse(
        r#"
[package]
org = "acme"
name = "tool"
version = "1.2.3"

[package.repository]
url = "https://github.com/acme/tool"
"#,
    )
    .unwrap();
    assert!(manifest.nix_export_routes().is_empty());

    let lock = Lockfile::parse(
        r#"
version = 1

[[package]]
org = "acme"
name = "tool"
version = "1.2.3"
sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
size = 42
format = "tar.gz"
vcs_tag = "v1.2.3"
vcs_commit = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
source = "https://zpkg.example"
"#,
    )
    .unwrap();
    assert!(lock.nix_adapters.is_empty());
}

#[test]
fn lockfile_adapter_upsert_is_validated_deduplicated_and_deterministic() {
    let mut lock = Lockfile::default();
    lock.upsert_nix_adapter(adapter("z-tool", "x86_64-linux"))
        .unwrap();
    lock.upsert_nix_adapter(adapter("a-tool", "aarch64-linux"))
        .unwrap();
    lock.upsert_nix_adapter(adapter("z-tool", "x86_64-linux"))
        .unwrap();

    assert_eq!(lock.nix_adapters.len(), 2);
    let encoded = lock.to_toml_string().unwrap();
    let parsed = Lockfile::parse(&encoded).unwrap();
    assert_eq!(parsed, lock);
    assert!(encoded.find("a-tool").unwrap() < encoded.find("z-tool").unwrap());
}

#[test]
fn lockfile_refuses_invalid_adapter_provenance() {
    let mut invalid = adapter("tool", "x86_64-linux");
    if let NixAdapterRecord::NixToZed { source, .. } = &mut invalid {
        source.locked_ref = "github:acme/tool/main".to_string();
    }

    let mut lock = Lockfile::default();
    lock.nix_adapters.push(invalid);
    assert!(lock.to_toml_string().is_err());
}
