use zed_interfaces::manifest::{Manifest, ManifestError};

const BASE: &str = r#"
[package]
org = "fiducia"
name = "fiducia-clients"
version = "1.1.2"

[package.repository]
vcs = "git"
url = "https://github.com/fiducia-cloud/fiducia-clients"

[targets.nodejs]
dir = "clients/ts"
name = "fiducia-js-sdk"
adapter = "node"
"#;

fn with_repository(name: Option<&str>) -> String {
    let mut manifest = format!("{BASE}\n[targets.repository]\ndir = \".\"\n");
    if let Some(name) = name {
        manifest.push_str(&format!("name = \"{name}\"\n"));
    }
    manifest
}

#[test]
fn whole_repository_target_uses_canonical_root_identity() {
    for source in [
        with_repository(None),
        with_repository(Some("fiducia-clients")),
    ] {
        let manifest = Manifest::parse(&source).unwrap();
        assert_eq!(
            manifest.target_package_name("repository").as_deref(),
            Some("fiducia-clients")
        );
        assert!(
            manifest
                .target_package_names()
                .contains(&("repository".to_string(), "fiducia-clients".to_string()))
        );

        let packaged = manifest
            .manifest_for_target("repository")
            .expect("repository target exists");
        assert_eq!(packaged.package.name, "fiducia-clients");
        assert_eq!(packaged.full_name(), "fiducia/fiducia-clients");
        assert!(!packaged.is_polyglot());
    }
}

#[test]
fn whole_repository_target_rejects_conflicting_explicit_name() {
    let error = Manifest::parse(&with_repository(Some("fiducia-clients-repository")))
        .expect_err("root target cannot advertise an identity the packer will not publish");
    match error {
        ManifestError::InvalidTarget(target, reason) => {
            assert_eq!(target, "repository");
            assert!(reason.contains("whole-repository target"), "{reason}");
            assert!(reason.contains("fiducia-clients"), "{reason}");
            assert!(reason.contains("fiducia-clients-repository"), "{reason}");
        }
        other => panic!("unexpected error: {other}"),
    }
}

#[test]
fn non_root_explicit_target_name_remains_authoritative() {
    let manifest = Manifest::parse(BASE).unwrap();
    assert_eq!(
        manifest.target_package_name("nodejs").as_deref(),
        Some("fiducia-js-sdk")
    );
    let packaged = manifest
        .manifest_for_target("nodejs")
        .expect("nodejs target exists");
    assert_eq!(packaged.package.name, "fiducia-js-sdk");
}
