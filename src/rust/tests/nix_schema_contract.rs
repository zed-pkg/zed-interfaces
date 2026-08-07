use schemars::schema_for;
use serde_json::Value;
use zed_interfaces::{NixAdapterRecord, NixExportSection};

const NIX_EXPORT_SCHEMA: &str = include_str!("../../../schemas/nix-export-section.json");
const NIX_ADAPTER_SCHEMA: &str = include_str!("../../../schemas/nix-adapter-record.json");

fn checked_in_schema(input: &str) -> Value {
    serde_json::from_str(input).expect("checked-in JSON schema must parse")
}

#[test]
fn checked_in_nix_export_schema_matches_the_public_contract() {
    let generated = serde_json::to_value(schema_for!(NixExportSection)).unwrap();
    assert_eq!(checked_in_schema(NIX_EXPORT_SCHEMA), generated);

    let text = NIX_EXPORT_SCHEMA;
    assert!(text.contains("systems"));
    assert!(text.contains("outputs"));
    assert!(text.contains("artifact"));
}

#[test]
fn checked_in_nix_adapter_schema_matches_the_public_contract() {
    let generated = serde_json::to_value(schema_for!(NixAdapterRecord)).unwrap();
    assert_eq!(checked_in_schema(NIX_ADAPTER_SCHEMA), generated);

    let text = NIX_ADAPTER_SCHEMA;
    assert!(text.contains("direction"));
    assert!(text.contains("schema"));
    assert!(text.contains("zed-to-nix"));
    assert!(text.contains("nix-to-zed"));
}
