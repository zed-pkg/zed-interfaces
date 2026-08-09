use schemars::schema_for;
use serde_json::Value;
use zed_interfaces::Manifest;

const MANIFEST_SCHEMA: &str = include_str!("../../../schemas/manifest.json");
const SLUG_PATTERN: &str = r"^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$";

#[test]
fn checked_in_manifest_schema_matches_the_public_contract() {
    let checked_in: Value =
        serde_json::from_str(MANIFEST_SCHEMA).expect("checked-in manifest schema must parse");
    let generated = serde_json::to_value(schema_for!(Manifest)).unwrap();
    assert_eq!(checked_in, generated);

    let package = &checked_in["$defs"]["PackageSection"];
    let required = package["required"].as_array().unwrap();
    for identity in ["org", "name"] {
        assert!(required.iter().any(|field| field == identity));
        assert_eq!(package["properties"][identity]["minLength"], 1);
        assert_eq!(package["properties"][identity]["pattern"], SLUG_PATTERN);
    }
}
