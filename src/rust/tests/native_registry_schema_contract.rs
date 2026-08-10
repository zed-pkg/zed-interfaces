use schemars::schema_for;
use serde_json::Value;
use zed_interfaces::NativeRegistryAdapterRecord;

const NATIVE_REGISTRY_ADAPTER_SCHEMA: &str =
    include_str!("../../../schemas/native-registry-adapter-record.json");

#[test]
fn checked_in_native_registry_schema_matches_the_public_contract() {
    let checked_in: Value = serde_json::from_str(NATIVE_REGISTRY_ADAPTER_SCHEMA)
        .expect("checked-in native-registry schema must parse");
    let generated = serde_json::to_value(schema_for!(NativeRegistryAdapterRecord)).unwrap();
    assert_eq!(checked_in, generated);

    let text = NATIVE_REGISTRY_ADAPTER_SCHEMA;
    assert!(text.contains("platform_packages"));
    assert!(text.contains("sha256"));
    assert!(text.contains("npm"));
    assert!(text.contains("cargo"));
}
