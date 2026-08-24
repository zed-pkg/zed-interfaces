use zed_interfaces::manifest::{
    Manifest, ManifestError, ProjectLifecycleHook, ProjectLifecycleMode,
};

fn manifest(extra: &str) -> String {
    format!(
        r#"
[package]
org = "acme"
name = "lifecycle-tool"
version = "1.2.3"

[package.repository]
url = "https://github.com/acme/lifecycle-tool"

{extra}
"#
    )
}

#[test]
fn every_phase_and_documented_value_shape_roundtrips_canonically() {
    let parsed = Manifest::parse(&manifest(
        r#"
[lifecycle]
pre-install = "./scripts/pre-install"
post-install = ["./scripts/post-install-a", "./scripts/post-install-b"]
pre-test = true
post-test = false

[lifecycle.pre-build]
mode = "prepend"
shell = "bash -eu"
env = { ZED_CONTRACT_MODE = "strict" }
commands = ["cargo fmt --check", "cargo clippy -- -D warnings"]

[lifecycle.post-build]
mode = "replace"
command = "./scripts/post-build"

[lifecycle.pre-pack]
mode = "append"
command = "./scripts/pre-pack"

[lifecycle.post-pack]
mode = "supplement"
commands = ["./scripts/post-pack"]

[lifecycle.pre-publish]
mode = "override"
command = "./scripts/pre-publish"

[lifecycle.post-publish]
mode = "complement"
commands = ["./scripts/post-publish"]

[lifecycle.pre-uninstall]
mode = "disable"

[lifecycle.post-uninstall]
mode = "append"
"#,
    ))
    .unwrap();

    assert!(matches!(
        parsed.lifecycle.pre_install,
        Some(ProjectLifecycleHook::Command(_))
    ));
    assert!(matches!(
        parsed.lifecycle.post_install,
        Some(ProjectLifecycleHook::Commands(_))
    ));
    let ProjectLifecycleHook::Config(pre_build) =
        parsed.lifecycle.pre_build.as_ref().expect("pre-build")
    else {
        panic!("pre-build must use the full configuration")
    };
    assert_eq!(pre_build.mode, ProjectLifecycleMode::Prepend);
    assert_eq!(pre_build.shell.as_deref(), Some("bash -eu"));
    assert_eq!(pre_build.env["ZED_CONTRACT_MODE"], "strict");
    let ProjectLifecycleHook::Config(post_pack) =
        parsed.lifecycle.post_pack.as_ref().expect("post-pack")
    else {
        panic!("post-pack must use the full configuration")
    };
    assert_eq!(post_pack.mode, ProjectLifecycleMode::Append);
    let ProjectLifecycleHook::Config(pre_publish) =
        parsed.lifecycle.pre_publish.as_ref().expect("pre-publish")
    else {
        panic!("pre-publish must use the full configuration")
    };
    assert_eq!(pre_publish.mode, ProjectLifecycleMode::Replace);

    let encoded = parsed.to_toml_string().unwrap();
    for phase in [
        "pre-install",
        "post-install",
        "pre-build",
        "post-build",
        "pre-test",
        "post-test",
        "pre-pack",
        "post-pack",
        "pre-publish",
        "post-publish",
        "pre-uninstall",
        "post-uninstall",
    ] {
        assert!(encoded.contains(phase), "missing `{phase}` in:\n{encoded}");
    }
    assert!(!encoded.contains("pre_build"), "{encoded}");
    assert!(!encoded.contains("supplement"), "{encoded}");
    assert!(!encoded.contains("override"), "{encoded}");
    assert_eq!(Manifest::parse(&encoded).unwrap(), parsed);
}

#[test]
fn snake_case_phase_alias_serializes_to_the_canonical_key() {
    let parsed = Manifest::parse(&manifest(
        r#"
[lifecycle.pre_build]
command = "cargo check"
"#,
    ))
    .unwrap();

    let encoded = parsed.to_toml_string().unwrap();
    assert!(encoded.contains("[lifecycle.pre-build]"), "{encoded}");
    assert!(!encoded.contains("pre_build"), "{encoded}");
}

#[test]
fn unknown_phase_and_unknown_config_field_fail_closed() {
    let unknown_phase = Manifest::parse(&manifest(
        r#"
[lifecycle.pre-buid]
command = "must-not-run"
"#,
    ));
    assert!(matches!(unknown_phase, Err(ManifestError::Toml(_))));

    let unknown_field = Manifest::parse(&manifest(
        r#"
[lifecycle.pre-build]
commands = ["cargo check"]
credential = "must-not-be-ignored"
"#,
    ));
    assert!(matches!(unknown_field, Err(ManifestError::Toml(_))));
}

#[test]
fn disabled_phases_cannot_smuggle_execution_configuration() {
    for extra in [
        r#"
[lifecycle.pre-build]
mode = "disable"
commands = ["must-not-run"]
"#,
        r#"
[lifecycle.pre-build]
mode = "disable"
shell = "bash"
"#,
        r#"
[lifecycle.pre-build]
mode = "disable"
env = { BUILD_MODE = "hidden" }
"#,
    ] {
        assert!(matches!(
            Manifest::parse(&manifest(extra)),
            Err(ManifestError::InvalidProjectLifecycle(phase, _)) if phase == "pre-build"
        ));
    }
}

#[test]
fn empty_commands_shell_expressions_and_secret_env_fail_closed() {
    for extra in [
        r#"
[lifecycle.pre-build]
command = "   "
"#,
        r#"
[lifecycle.pre-build]
shell = "bash; curl"
command = "cargo check"
"#,
        r#"
[lifecycle.pre-build]
env = { REGISTRY_TOKEN = "do-not-put-secrets-in-manifests" }
command = "cargo check"
"#,
        r#"
[lifecycle.pre-build]
env = { "INVALID-NAME" = "value" }
command = "cargo check"
"#,
    ] {
        assert!(matches!(
            Manifest::parse(&manifest(extra)),
            Err(ManifestError::InvalidProjectLifecycle(phase, _)) if phase == "pre-build"
        ));
    }
}

#[test]
fn generated_manifest_schema_has_the_closed_phase_vocabulary() {
    let schema = serde_json::to_value(schemars::schema_for!(Manifest)).unwrap();
    let lifecycle = &schema["$defs"]["ProjectLifecycleSection"];
    assert_eq!(lifecycle["additionalProperties"], false);
    let properties = lifecycle["properties"].as_object().unwrap();
    assert_eq!(properties.len(), 12);
    for phase in [
        "pre-install",
        "post-install",
        "pre-build",
        "post-build",
        "pre-test",
        "post-test",
        "pre-pack",
        "post-pack",
        "pre-publish",
        "post-publish",
        "pre-uninstall",
        "post-uninstall",
    ] {
        assert!(properties.contains_key(phase), "schema omitted {phase}");
    }
}
