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

#[test]
fn local_path_overrides_are_typed_and_reject_shell_execution() {
    let valid = r#"
[package]
org = "acme"
name = "consumer"
version = "1.0.0"
license = "MIT"

[package.repository]
vcs = "git"
url = "https://github.com/acme/consumer"

[overrides.path]
"acme/lib" = "${HOME}/codes/acme/lib"
"#;
    let manifest = Manifest::parse(valid).expect("safe env-expanded local path override");
    assert_eq!(
        manifest.dependency_path_override("acme/lib"),
        Some("${HOME}/codes/acme/lib")
    );

    for dangerous in ["$(touch /tmp/zed-owned)", "`touch /tmp/zed-owned`"] {
        let input = valid.replace("${HOME}/codes/acme/lib", dangerous);
        let error = Manifest::parse(&input).expect_err("shell syntax must be rejected");
        assert!(error.to_string().contains("command substitution"));
    }
}

#[test]
fn source_composition_is_manifest_authoritative_and_layout_safe() {
    let valid = r#"
[package]
org = "acme"
name = "consumer"
version = "1.0.0"
license = "MIT"

[package.repository]
vcs = "git"
url = "https://github.com/acme/consumer"

[install]
dir = ".zed/pkg"

[dependencies]
"acme/lib" = "=1.2.3"

[interop.source-composition]
checkout_dir = ".zed/vcs"
git_submodule_dir = "submodules"

[interop.source-composition.sources.lib]
vcs = "git"
url = "https://github.com/acme/lib.git"
role = "workspace"
projection = "git-submodule"
path = "apps/lib"
package = "acme/lib"
branch = "main"
recursive = true
"#;
    let manifest = Manifest::parse(valid).expect("manifest-owned source composition");
    let sources = &manifest.interop.source_composition;
    assert_eq!(sources.checkout_dir(), ".zed/vcs");
    assert_eq!(sources.git_submodule_dir(), "submodules");
    assert_eq!(sources.sources["lib"].package.as_deref(), Some("acme/lib"));

    for (needle, replacement) in [
        ("path = \"apps/lib\"", "path = \".zed/pkg/acme/lib\""),
        ("vcs = \"git\"", "vcs = \"hg\""),
    ] {
        let input = if needle == "vcs = \"git\"" {
            valid.replacen(needle, replacement, 2)
        } else {
            valid.replace(needle, replacement)
        };
        let error = Manifest::parse(&input).expect_err("unsafe source composition must fail");
        assert!(error.to_string().contains("source-composition"), "{error}");
    }
}

#[test]
fn workspace_sources_require_canonical_package_identity() {
    let input = r#"
[package]
org = "acme"
name = "consumer"
version = "1.0.0"
license = "MIT"

[package.repository]
vcs = "git"
url = "https://github.com/acme/consumer"

[interop.source-composition.sources.lib]
vcs = "git"
url = "https://github.com/acme/lib.git"
role = "workspace"
"#;
    let error = Manifest::parse(input).expect_err("workspace source without package must fail");
    assert!(error.to_string().contains("must declare package"));
}
