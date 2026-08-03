use zed_interfaces::manifest::{Manifest, ManifestError};

fn manifest(extra: &str) -> String {
    format!(
        r#"
[package]
org = "acme"
name = "native-edge"
version = "1.2.3"

[package.repository]
url = "https://github.com/acme/native-edge"

{extra}
"#
    )
}

#[test]
fn target_language_synonyms_select_the_same_native_metadata() {
    let parsed = Manifest::parse(&manifest(
        r#"
[native-dependencies]
apt = ["pkg-config"]

[hooks]
pre-install = ["echo package-pre"]

[targets.nodejs]
dir = "clients/node"

[targets.nodejs.native-dependencies]
apt = ["nodejs"]

[targets.nodejs.hooks]
pre-install = ["echo node-pre"]
post-install = ["echo node-post"]
"#,
    ))
    .unwrap();

    let native = parsed.effective_native_dependencies(Some("node")).unwrap();
    assert_eq!(native["apt"], vec!["pkg-config", "nodejs"]);

    let hooks = parsed.effective_install_hooks(Some("node")).unwrap();
    assert_eq!(hooks.pre_install, vec!["echo package-pre", "echo node-pre"]);
    assert_eq!(hooks.post_install, vec!["echo node-post"]);
}

#[test]
fn target_aliases_serialize_to_the_canonical_keys() {
    let parsed = Manifest::parse(&manifest(
        r#"
[targets.rust]
dir = "clients/rust"

[targets.rust.native_dependencies]
apt = ["clang"]

[targets.rust.hooks]
pre_install = ["echo pre"]
post_install = ["echo post"]
"#,
    ))
    .unwrap();

    let encoded = parsed.to_toml_string().unwrap();
    assert!(encoded.contains("[targets.rust.native-dependencies]"));
    assert!(encoded.contains("pre-install ="));
    assert!(encoded.contains("post-install ="));
    assert!(!encoded.contains("native_dependencies"));
    assert!(!encoded.contains("pre_install"));
    assert!(!encoded.contains("post_install"));
}

#[test]
fn manifest_target_projection_applies_lifecycle_metadata_exactly_once() {
    let parsed = Manifest::parse(&manifest(
        r#"
[native-dependencies]
apt = ["pkg-config", "libssl-dev"]

[hooks]
pre-install = ["echo package-pre"]
post-install = ["echo package-post"]

[targets.rust]
dir = "clients/rust"

[targets.rust.native-dependencies]
apt = ["clang", "libssl-dev"]

[targets.rust.hooks]
pre-install = ["echo target-pre"]
post-install = ["echo target-post"]
"#,
    ))
    .unwrap();

    let projected = parsed.manifest_for_target("rust").unwrap();
    assert!(projected.targets.is_empty());
    assert_eq!(
        projected.effective_native_dependencies(None).unwrap()["apt"],
        vec!["pkg-config", "libssl-dev", "clang"]
    );
    assert_eq!(
        projected.effective_install_hooks(None).unwrap().pre_install,
        vec!["echo package-pre", "echo target-pre"]
    );
    assert_eq!(
        projected
            .effective_install_hooks(None)
            .unwrap()
            .post_install,
        vec!["echo package-post", "echo target-post"]
    );
    assert!(projected.manifest_for_target("rust").is_none());

    let reparsed = Manifest::parse(&projected.to_toml_string().unwrap()).unwrap();
    assert_eq!(reparsed, projected);
}

#[test]
fn empty_target_manager_routes_preserve_support_without_adding_packages() {
    let parsed = Manifest::parse(&manifest(
        r#"
[native-dependencies]
apt = ["pkg-config"]

[targets.rust]
dir = "clients/rust"

[targets.rust.native-dependencies]
apt = []
nix = []
"#,
    ))
    .unwrap();

    let native = parsed.effective_native_dependencies(Some("rust")).unwrap();
    assert_eq!(native["apt"], vec!["pkg-config"]);
    assert_eq!(native["nix"], Vec::<String>::new());
}

#[test]
fn native_and_hook_size_limits_accept_the_exact_boundary() {
    let package = "a".repeat(256);
    let hook = "x".repeat(32 * 1024);
    let source = manifest(&format!(
        "[native-dependencies]\napt = [{}]\n\n[hooks]\npre-install = [{}]\n",
        toml::Value::String(package.clone()),
        toml::Value::String(hook.clone())
    ));

    let parsed = Manifest::parse(&source).unwrap();
    assert_eq!(parsed.native_dependencies["apt"], vec![package]);
    assert_eq!(parsed.hooks.pre_install, vec![hook]);
}

#[test]
fn native_package_size_limit_rejects_the_first_oversized_value() {
    let package = "a".repeat(257);
    let source = manifest(&format!(
        "[native-dependencies]\napt = [{}]\n",
        toml::Value::String(package)
    ));

    assert!(matches!(
        Manifest::parse(&source),
        Err(ManifestError::InvalidNativeDependency(_, _))
    ));
}

#[test]
fn shell_metacharacters_remain_opaque_native_package_arguments() {
    let package = "libssl-dev;echo-not-a-shell";
    let parsed = Manifest::parse(&manifest(&format!(
        "[native-dependencies]\napt = [{}]\n",
        toml::Value::String(package.to_string())
    )))
    .unwrap();

    assert_eq!(parsed.native_dependencies["apt"], vec![package]);
}
