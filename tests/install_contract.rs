use std::collections::BTreeMap;

use zed_interfaces::manifest::{
    InstallHooksSection, Manifest, ManifestError, NATIVE_PACKAGE_MANAGERS,
};

fn manifest(extra: &str) -> String {
    format!(
        r#"
[package]
org = "acme"
name = "native-tool"
version = "1.2.3"

[package.repository]
url = "https://github.com/acme/native-tool"

{extra}
"#
    )
}

#[test]
fn native_dependencies_and_hooks_roundtrip_canonically() {
    let parsed = Manifest::parse(&manifest(
        r#"
[native_dependencies]
apt = ["pkg-config", "libssl-dev"]
brew = ["pkg-config", "openssl@3"]
nix = ["pkg-config", "openssl"]

[hooks]
pre_install = ["./scripts/pre-install.sh"]
post_install = ["./scripts/post-install.sh"]
"#,
    ))
    .unwrap();

    assert_eq!(
        parsed.native_dependencies["apt"],
        vec!["pkg-config", "libssl-dev"]
    );
    assert_eq!(
        parsed.hooks,
        InstallHooksSection {
            pre_install: vec!["./scripts/pre-install.sh".to_string()],
            post_install: vec!["./scripts/post-install.sh".to_string()],
        }
    );

    let encoded = parsed.to_toml_string().unwrap();
    assert!(encoded.contains("[native-dependencies]"), "{encoded}");
    assert!(encoded.contains("pre-install ="), "{encoded}");
    assert!(encoded.contains("post-install ="), "{encoded}");
    assert!(!encoded.contains("native_dependencies"), "{encoded}");
    assert!(!encoded.contains("pre_install"), "{encoded}");
    assert_eq!(Manifest::parse(&encoded).unwrap(), parsed);
}

#[test]
fn target_native_dependencies_and_hooks_merge_in_order() {
    let parsed = Manifest::parse(&manifest(
        r#"
[native-dependencies]
apt = ["pkg-config", "libssl-dev"]
brew = ["pkg-config"]

[hooks]
pre-install = ["echo package-pre"]
post-install = ["echo package-post"]

[targets.rust]
dir = "clients/rust"

[targets.rust.native-dependencies]
apt = ["clang", "libssl-dev"]
brew = ["llvm"]

[targets.rust.hooks]
pre-install = ["echo target-pre"]
post-install = ["echo target-post"]
"#,
    ))
    .unwrap();

    let native = parsed.effective_native_dependencies(Some("rust")).unwrap();
    assert_eq!(native["apt"], vec!["pkg-config", "libssl-dev", "clang"]);
    assert_eq!(native["brew"], vec!["pkg-config", "llvm"]);

    let hooks = parsed.effective_install_hooks(Some("rust")).unwrap();
    assert_eq!(
        hooks.pre_install,
        vec!["echo package-pre", "echo target-pre"]
    );
    assert_eq!(
        hooks.post_install,
        vec!["echo package-post", "echo target-post"]
    );

    let derived = parsed.manifest_for_target("rust").unwrap();
    assert!(derived.targets.is_empty());
    assert_eq!(derived.native_dependencies, native);
    assert_eq!(derived.hooks, hooks);
}

#[test]
fn an_unselected_target_does_not_leak_native_install_metadata() {
    let parsed = Manifest::parse(&manifest(
        r#"
[native-dependencies]
apt = ["pkg-config"]

[targets.rust]
dir = "clients/rust"
[targets.rust.native-dependencies]
apt = ["clang"]

[targets.node]
dir = "clients/node"
[targets.node.native-dependencies]
apt = ["nodejs"]
"#,
    ))
    .unwrap();

    assert_eq!(
        parsed.effective_native_dependencies(Some("node")).unwrap()["apt"],
        vec!["pkg-config", "nodejs"]
    );
    assert_eq!(
        parsed.effective_native_dependencies(None).unwrap()["apt"],
        vec!["pkg-config"]
    );
}

#[test]
fn every_documented_native_manager_parses_even_with_no_packages() {
    let mut source = manifest("");
    source.push_str("\n[native-dependencies]\n");
    for manager in NATIVE_PACKAGE_MANAGERS {
        source.push_str(&format!("{manager} = []\n"));
    }
    let parsed = Manifest::parse(&source).unwrap();
    assert_eq!(
        parsed.native_dependencies.len(),
        NATIVE_PACKAGE_MANAGERS.len()
    );
}

#[test]
fn unsafe_or_ambiguous_native_package_specs_are_rejected() {
    for package in ["", "-y", "two words", "line\nbreak", "\u{7f}"] {
        let source = manifest(&format!(
            "[native-dependencies]\napt = [{}]\n",
            toml::Value::String(package.to_string())
        ));
        assert!(matches!(
            Manifest::parse(&source),
            Err(ManifestError::InvalidNativeDependency(_, _))
        ));
    }

    let duplicate = manifest(
        r#"
[native-dependencies]
apt = ["pkg-config", "pkg-config"]
"#,
    );
    assert!(matches!(
        Manifest::parse(&duplicate),
        Err(ManifestError::InvalidNativeDependency(_, _))
    ));
}

#[test]
fn unsupported_native_manager_is_rejected_with_the_allowlist() {
    let error = Manifest::parse(&manifest(
        r#"
[native-dependencies]
curl-pipe-sh = ["anything"]
"#,
    ))
    .unwrap_err();
    let message = error.to_string();
    assert!(matches!(
        error,
        ManifestError::InvalidNativeDependency(_, _)
    ));
    assert!(message.contains("unsupported"), "{message}");
    assert!(message.contains("apt"), "{message}");
    assert!(message.contains("nix"), "{message}");
}

#[test]
fn empty_and_oversized_install_hooks_are_rejected() {
    for (key, value) in [
        ("pre-install", "   ".to_string()),
        ("post-install", "x".repeat(32 * 1024 + 1)),
    ] {
        let source = manifest(&format!(
            "[hooks]\n{key} = [{}]\n",
            toml::Value::String(value)
        ));
        assert!(matches!(
            Manifest::parse(&source),
            Err(ManifestError::InvalidInstallHook(_, _))
        ));
    }
}

#[test]
fn effective_install_metadata_rejects_an_unknown_explicit_target() {
    let parsed = Manifest::parse(&manifest(
        r#"
[targets.rust]
dir = "clients/rust"
"#,
    ))
    .unwrap();
    assert!(matches!(
        parsed.effective_native_dependencies(Some("python")),
        Err(ManifestError::InvalidTarget(_, _))
    ));
    assert!(matches!(
        parsed.effective_install_hooks(Some("python")),
        Err(ManifestError::InvalidTarget(_, _))
    ));
}

#[test]
fn empty_default_sections_are_omitted() {
    let parsed = Manifest::parse(&manifest("")).unwrap();
    assert_eq!(parsed.native_dependencies, BTreeMap::new());
    assert!(parsed.hooks.is_empty());
    let encoded = parsed.to_toml_string().unwrap();
    assert!(!encoded.contains("native-dependencies"));
    assert!(!encoded.contains("[hooks]"));
}
