use schemars::schema_for;
use serde_json::Value;
use zed_interfaces::Lockfile;

const LOCKFILE_SCHEMA: &str = include_str!("../../../schemas/lockfile.json");

#[test]
fn checked_in_lockfile_schema_includes_native_dependency_provenance() {
    let checked_in: Value =
        serde_json::from_str(LOCKFILE_SCHEMA).expect("checked-in lockfile schema must parse");
    let generated = serde_json::to_value(schema_for!(Lockfile)).unwrap();
    assert_eq!(checked_in, generated);

    let text = LOCKFILE_SCHEMA;
    assert!(text.contains("native-dependency"));
    assert!(text.contains("NativeDependencyLock"));
    assert!(text.contains("NativeVersionRequirement"));
    assert!(text.contains("declared"));
    assert!(text.contains("canonical"));
    assert!(text.contains("sha256"));
    assert!(text.contains("npm"));
    assert!(text.contains("cargo"));
}
