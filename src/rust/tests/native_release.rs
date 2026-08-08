use zed_interfaces::manifest::{ForgeRegistry, Manifest, ManifestError, NativeRegistry};

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

[targets.python]
dir = "clients/python"

[targets.python.native]
registry = "pypi"
package = "Acme.Client"
"#,
    ))
    .unwrap();

    let routes = parsed.native_release_routes();
    assert_eq!(routes.len(), 4);
    assert_eq!(routes[0].target, "dart");
    assert_eq!(routes[0].registry, NativeRegistry::PubDev);
    assert_eq!(routes[0].package, "acme_client");
    assert_eq!(routes[1].target, "nodejs");
    assert_eq!(routes[1].registry, NativeRegistry::Npm);
    assert_eq!(routes[1].package, "@acme/client");
    assert_eq!(routes[2].target, "python");
    assert_eq!(routes[2].registry, NativeRegistry::PyPi);
    assert_eq!(routes[2].package, "Acme.Client");
    assert_eq!(routes[3].target, "rust");
    assert_eq!(routes[3].registry, NativeRegistry::CratesIo);

    let encoded = parsed.to_toml_string().unwrap();
    assert!(encoded.contains("[targets.nodejs.native]"));
    assert!(encoded.contains("registry = \"npm\""));
    assert!(encoded.contains("registry = \"pub.dev\""));
    Manifest::parse(&encoded).unwrap();
}

#[test]
fn a_single_language_repository_can_declare_native_and_forge_routes() {
    let parsed = Manifest::parse(&manifest(
        r#"
[publish.native]
registry = "npm"
package = "@acme/client"
forge = ["github-packages", "gitlab-packages", "bitbucket-packages"]
"#,
    ))
    .unwrap();

    let native = parsed.native_release_routes();
    assert_eq!(native.len(), 1);
    assert_eq!(native[0].target, "repository");
    assert_eq!(native[0].dir, ".");
    assert_eq!(native[0].registry, NativeRegistry::Npm);
    assert_eq!(native[0].package, "@acme/client");
    assert_eq!(native[0].vcs_tag, "v1.2.3");

    let forge = parsed.forge_release_routes();
    assert_eq!(forge.len(), 3);
    assert!(forge.iter().all(|route| route.target == "repository"));
    assert!(forge.iter().all(|route| route.dir == "."));
    assert_eq!(forge[0].registry, ForgeRegistry::GithubPackages);
    assert_eq!(forge[1].registry, ForgeRegistry::GitlabPackages);
    assert_eq!(forge[2].registry, ForgeRegistry::BitbucketPackages);
    assert!(forge.iter().all(|route| route.vcs_tag == "v1.2.3"));

    let encoded = parsed.to_toml_string().unwrap();
    assert!(encoded.contains("[publish.native]"));
    assert_eq!(Manifest::parse(&encoded).unwrap(), parsed);
}

#[test]
fn a_polyglot_package_cannot_mix_root_and_target_native_routes() {
    let error = Manifest::parse(&manifest(
        r#"
[publish.native]
registry = "npm"
package = "@acme/all-clients"

[targets.nodejs]
dir = "clients/typescript"

[targets.nodejs.native]
registry = "npm"
package = "@acme/client"
"#,
    ))
    .unwrap_err();

    let message = error.to_string();
    assert!(matches!(error, ManifestError::InvalidNativeRoute(_, _)));
    assert!(message.contains("single-language"), "{message}");
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
        r#"
[targets.python]
dir = "clients/python"
[targets.python.native]
registry = "pypi"
package = "-bad-name"
"#,
        r#"
[targets.python]
dir = "clients/python"
[targets.python.native]
registry = "pypi"
package = "bad name"
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

#[test]
fn pypi_duplicate_destinations_use_normalized_names() {
    let error = Manifest::parse(&manifest(
        r#"
[targets.python]
dir = "clients/python"
[targets.python.native]
registry = "pypi"
package = "Friendly_Bard"

[targets.python-async]
dir = "clients/python-async"
[targets.python-async.native]
registry = "pypi"
package = "friendly...bard"
"#,
    ))
    .unwrap_err();
    let message = error.to_string();
    assert!(message.contains("already routed"), "{message}");
}

// --- the two ecosystem axes must agree ------------------------------------

#[test]
fn a_native_route_that_contradicts_the_install_ecosystem_is_rejected() {
    // `native` is outbound (mirror to npm), `ecosystem` is inbound (what a
    // consumer needs to install). A python slice routed to npm means one of the
    // two is wrong, and either way it stays silent until someone hits it.
    let err = Manifest::parse(&manifest(
        r#"
[targets.python]
dir = "clients/python"

[targets.python.native]
registry = "npm"
package = "acme-client"
"#,
    ))
    .expect_err("a python target mirrored to npm must not validate");
    let msg = err.to_string();
    assert!(msg.contains("npm"), "{msg}");
    assert!(msg.contains("pypi"), "{msg}");
}

#[test]
fn a_native_route_matching_the_target_language_validates() {
    for (target, dir, registry, package) in [
        ("nodejs", "clients/ts", "npm", "@acme/client"),
        ("rust", "clients/rust", "crates-io", "acme-client"),
        ("python", "clients/python", "pypi", "acme-client"),
        ("dart", "clients/dart", "pub.dev", "acme_client"),
    ] {
        let toml = manifest(&format!(
            r#"
[targets.{target}]
dir = "{dir}"

[targets.{target}.native]
registry = "{registry}"
package = "{package}"
"#
        ));
        let parsed = Manifest::parse(&toml)
            .unwrap_or_else(|e| panic!("{target} -> {registry} must validate: {e}"));
        let route = &parsed.native_release_routes()[0];
        assert_eq!(route.target, target);
        assert_eq!(
            route.registry.ecosystem(),
            parsed.targets[target].ecosystem_for(target)
        );
    }
}

#[test]
fn an_explicit_ecosystem_override_reconciles_a_deliberate_mismatch() {
    // rust-wasm is Rust source consumed by a JS bundler: the mirror really is
    // npm, so declaring the inbound ecosystem makes the pair consistent rather
    // than requiring the check to be weakened.
    let toml = manifest(
        r#"
[targets.rust-wasm]
dir = "clients/rust-wasm"
ecosystem = "npm"

[targets.rust-wasm.native]
registry = "npm"
package = "@acme/client-wasm"
"#,
    );
    let parsed = Manifest::parse(&toml).expect("an explicit npm ecosystem must reconcile");
    assert_eq!(
        parsed.targets["rust-wasm"].ecosystem_for("rust-wasm"),
        NativeRegistry::Npm.ecosystem()
    );
}

#[test]
fn a_target_key_that_is_not_a_language_never_contradicts_its_route() {
    // An arbitrary target slug carries no ecosystem claim, so there is nothing
    // for the native route to disagree with.
    let toml = manifest(
        r#"
[targets.wasm3]
dir = "clients/wasm3"

[targets.wasm3.native]
registry = "npm"
package = "acme-wasm3"
"#,
    );
    assert!(Manifest::parse(&toml).is_ok());
}

#[test]
fn forge_package_routes_roundtrip_and_flatten_deterministically() {
    let parsed = Manifest::parse(&manifest(
        r#"
[targets.nodejs]
dir = "clients/typescript"

[targets.nodejs.native]
registry = "npm"
package = "@acme/client"
forge = ["github-packages", "gitlab-packages", "bitbucket-packages"]

[targets.python]
dir = "clients/python"

[targets.python.native]
registry = "pypi"
package = "acme-client"
forge = ["gitlab-packages"]
"#,
    ))
    .unwrap();

    let routes = parsed.forge_release_routes();
    assert_eq!(routes.len(), 4);
    assert_eq!(routes[0].target, "nodejs");
    assert_eq!(routes[0].registry, ForgeRegistry::GithubPackages);
    assert_eq!(routes[0].format, NativeRegistry::Npm);
    assert_eq!(routes[1].registry, ForgeRegistry::GitlabPackages);
    assert_eq!(routes[2].registry, ForgeRegistry::BitbucketPackages);
    assert_eq!(routes[3].target, "python");
    assert_eq!(routes[3].registry, ForgeRegistry::GitlabPackages);
    assert_eq!(routes[3].format, NativeRegistry::PyPi);

    let encoded = parsed.to_toml_string().unwrap();
    assert!(encoded.contains("github-packages"));
    assert_eq!(Manifest::parse(&encoded).unwrap(), parsed);

    let derived = parsed.manifest_for_target("nodejs").unwrap();
    assert!(derived.targets.is_empty());
    assert_eq!(derived.publish.native, parsed.targets["nodejs"].native);
    assert_eq!(derived.native_release_routes()[0].target, "repository");
    assert_eq!(
        Manifest::parse(&derived.to_toml_string().unwrap()).unwrap(),
        derived
    );
}

#[test]
fn unsupported_and_duplicate_forge_routes_are_rejected() {
    for targets in [
        r#"
[targets.rust]
dir = "clients/rust"
[targets.rust.native]
registry = "crates-io"
package = "acme-client"
forge = ["github-packages"]
"#,
        r#"
[targets.python]
dir = "clients/python"
[targets.python.native]
registry = "pypi"
package = "acme-client"
forge = ["bitbucket-packages"]
"#,
        r#"
[targets.nodejs]
dir = "clients/typescript"
[targets.nodejs.native]
registry = "npm"
package = "@acme/client"
forge = ["github-packages", "github-packages"]
"#,
    ] {
        assert!(matches!(
            Manifest::parse(&manifest(targets)),
            Err(ManifestError::InvalidNativeRoute(_, _))
        ));
    }
}

#[test]
fn major_native_registry_identities_and_ecosystems_validate() {
    for (target, registry, package, ecosystem) in [
        ("java", "maven-central", "com.acme:client", "jvm"),
        ("ruby", "rubygems", "acme-client", "gem"),
        ("csharp", "nuget", "Acme.Client", "nuget"),
        ("php", "packagist", "acme/client", "composer"),
        ("golang", "go-modules", "github.com/acme/client", "gomod"),
    ] {
        let tag_format = if registry == "go-modules" {
            format!("tag_format = \"clients/{target}/v{{version}}\"")
        } else {
            String::new()
        };
        let parsed = Manifest::parse(&manifest(&format!(
            r#"
[targets.{target}]
dir = "clients/{target}"

[targets.{target}.native]
registry = "{registry}"
package = "{package}"
{tag_format}
"#
        )))
        .unwrap_or_else(|error| panic!("{registry} route must validate: {error}"));
        let route = &parsed.native_release_routes()[0];
        assert_eq!(route.registry.ecosystem().as_str(), ecosystem);
        if registry == "go-modules" {
            assert_eq!(route.vcs_tag, format!("clients/{target}/v1.2.3"));
        }
    }
}

#[test]
fn a_subdirectory_go_module_requires_its_native_tag_prefix() {
    for tag_format in ["", "tag_format = \"v{version}\""] {
        let error = Manifest::parse(&manifest(&format!(
            r#"
[targets.golang]
dir = "clients/go"

[targets.golang.native]
registry = "go-modules"
package = "github.com/acme/client"
{tag_format}
"#
        )))
        .unwrap_err();
        assert!(error.to_string().contains("clients/go/"));
    }
}
