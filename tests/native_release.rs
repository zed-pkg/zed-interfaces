use zed_interfaces::manifest::{Manifest, ManifestError, NativeRegistry};

fn manifest(targets: &str) -> String {
    format!(
        r#"
[package]
org = "acme"
name = "clients"
version = "1.2.3"

[package.repository]
url = "https://github.com/acme/clients"

{targets}
"#
    )
}

#[test]
fn native_routes_parse_roundtrip_and_stay_sorted() {
    let parsed = Manifest::parse(&manifest(
        r#"
[targets.rust]
dir = "clients/rust"

[targets.rust.native]
registry = "crates-io"
package = "acme-client"

[targets.nodejs]
dir = "clients/typescript"
adapter = "node"

[targets.nodejs.native]
registry = "npm"
package = "@acme/client"

[targets.dart]
dir = "clients/dart"

[targets.dart.native]
registry = "pub.dev"
package = "acme_client"
"#,
    ))
    .unwrap();

    let routes = parsed.native_release_routes();
    assert_eq!(routes.len(), 3);
    assert_eq!(routes[0].target, "dart");
    assert_eq!(routes[0].registry, NativeRegistry::PubDev);
    assert_eq!(routes[0].package, "acme_client");
    assert_eq!(routes[1].target, "nodejs");
    assert_eq!(routes[1].registry, NativeRegistry::Npm);
    assert_eq!(routes[1].package, "@acme/client");
    assert_eq!(routes[2].target, "rust");
    assert_eq!(routes[2].registry, NativeRegistry::CratesIo);

    let encoded = parsed.to_toml_string().unwrap();
    assert!(encoded.contains("[targets.nodejs.native]"));
    assert!(encoded.contains("registry = \"npm\""));
    assert!(encoded.contains("registry = \"pub.dev\""));
    Manifest::parse(&encoded).unwrap();
}

#[test]
fn whole_repository_target_cannot_route_to_a_native_registry() {
    let error = Manifest::parse(&manifest(
        r#"
[targets.repository]
dir = "."

[targets.repository.native]
registry = "npm"
package = "acme-repository"
"#,
    ))
    .unwrap_err();
    assert!(matches!(error, ManifestError::InvalidNativeRoute(_, _)));
}

#[test]
fn native_package_identities_are_registry_specific() {
    for targets in [
        r#"
[targets.nodejs]
dir = "clients/typescript"
[targets.nodejs.native]
registry = "npm"
package = "Bad Package"
"#,
        r#"
[targets.rust]
dir = "clients/rust"
[targets.rust.native]
registry = "crates-io"
package = "acme/client"
"#,
        r#"
[targets.dart]
dir = "clients/dart"
[targets.dart.native]
registry = "pub.dev"
package = "Bad-Dart-Package"
"#,
        r#"
[targets.dart]
dir = "clients/dart"
[targets.dart.native]
registry = "pub.dev"
package = "123_client"
"#,
        r#"
[targets.dart]
dir = "clients/dart"
[targets.dart.native]
registry = "pub.dev"
package = "class"
"#,
    ] {
        assert!(matches!(
            Manifest::parse(&manifest(targets)),
            Err(ManifestError::InvalidNativeRoute(_, _))
        ));
    }
}

#[test]
fn duplicate_native_destinations_are_rejected() {
    let error = Manifest::parse(&manifest(
        r#"
[targets.nodejs]
dir = "clients/typescript"
[targets.nodejs.native]
registry = "npm"
package = "@acme/client"

[targets.browser]
dir = "clients/browser"
[targets.browser.native]
registry = "npm"
package = "@acme/client"
"#,
    ))
    .unwrap_err();
    assert!(matches!(error, ManifestError::InvalidNativeRoute(_, _)));
}
