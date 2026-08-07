use zed_interfaces::{
    ArtifactFormat, NixAdapterRecord, NixBuilderNetwork, NixInteropArtifact, NixOutputOrigin,
    NixPackageIdentity, NixPolicyEvidence, NixPolicyProfile, NixRealizedOutput, NixStoreReference,
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

fn source(references: Vec<NixStoreReference>) -> NixOutputOrigin {
    NixOutputOrigin {
        locked_ref: format!("github:acme/tool/{HEX_A}"),
        flake_lock_sha256: HEX_A.to_string(),
        attribute: "packages.x86_64-linux.tool".to_string(),
        realized: NixRealizedOutput {
            system: "x86_64-linux".to_string(),
            output: "out".to_string(),
            derivation_json_sha256: HEX_B.to_string(),
            store_path: STORE_A.to_string(),
            nar_hash: NAR_A.to_string(),
            nar_size: 512,
            references,
            signatures: vec!["cache.example-1:signature".to_string()],
            nix_version: "2.35.2".to_string(),
            store_info_json_version: 3,
        },
    }
}

fn package() -> NixPackageIdentity {
    NixPackageIdentity {
        org: "acme".to_string(),
        name: "tool".to_string(),
        version: "1.2.3".to_string(),
        target: None,
    }
}

fn artifact() -> NixInteropArtifact {
    NixInteropArtifact {
        format: ArtifactFormat::TarGz,
        sha256: HEX_B.to_string(),
        size: 1024,
    }
}

#[test]
fn public_contract_round_trips_canonical_json() {
    let record =
        NixAdapterRecord::nix_to_zed(package(), source(Vec::new()), artifact(), strict_policy());

    let canonical = record.canonical_json_string().unwrap();
    let parsed: NixAdapterRecord = serde_json::from_str(&canonical).unwrap();

    assert_eq!(record, parsed);
    assert_eq!(canonical, parsed.canonical_json_string().unwrap());
    assert!(canonical.contains("\"direction\":\"nix-to-zed\""));
}

#[test]
fn public_contract_rejects_a_mutable_flake_selector() {
    let mut mutable = source(Vec::new());
    mutable.locked_ref = "github:acme/tool/main".to_string();
    let record = NixAdapterRecord::nix_to_zed(package(), mutable, artifact(), strict_policy());

    assert!(record.validate().is_err());
}

#[test]
fn public_contract_rejects_a_closure_bearing_import() {
    let reference = NixStoreReference {
        store_path: "/nix/store/11111111111111111111111111111111-glibc".to_string(),
        nar_hash: Some(NAR_A.to_string()),
        nar_size: Some(2048),
    };
    let record = NixAdapterRecord::nix_to_zed(
        package(),
        source(vec![reference]),
        artifact(),
        strict_policy(),
    );

    assert!(record.validate().is_err());
}
