use schemars::schema_for;
use serde_json::Value;
use zed_interfaces::NativeDependencyLock;

const NATIVE_DEPENDENCY_LOCK_SCHEMA: &str =
    include_str!("../../../schemas/native-dependency-lock.json");

#[test]
fn checked_in_native_dependency_schema_matches_the_public_contract() {
    let checked_in: Value = serde_json::from_str(NATIVE_DEPENDENCY_LOCK_SCHEMA)
        .expect("checked-in native dependency schema must parse");
    let generated = serde_json::to_value(schema_for!(NativeDependencyLock)).unwrap();
    assert_eq!(checked_in, generated);

    let text = NATIVE_DEPENDENCY_LOCK_SCHEMA;
    assert!(text.contains("NativeDependencyLock"));
    assert!(text.contains("NativeVersionRequirement"));
    assert!(text.contains("declared"));
    assert!(text.contains("canonical"));
    assert!(text.contains("sha256"));
    assert!(text.contains("npm"));
    assert!(text.contains("cargo"));
}
