//! `oci-registry` mirrors: GitHub Packages as an installable artifact source.
//!
//! GitHub exposes two entirely separate surfaces, and conflating them is the
//! mistake these tests exist to prevent. Release assets live on a repository's
//! **Releases** page and are reachable with a plain unauthenticated GET;
//! they never appear under `github.com/orgs/<org>/packages`. Container
//! packages live on **ghcr.io**, do appear there, and require a bearer token
//! even when public. zed uses both, for different reasons, and a descriptor
//! has to say which one it means.

use zed_interfaces::ArtifactFormat;
use zed_interfaces::mirror::{
    GHCR_URL, MirrorCoordinateV1, MirrorDescriptorV1, MirrorKindV1, is_oci_repository_path,
};

const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const REPOSITORY: &str = "https://github.com/acme/http-kit";

fn coord() -> MirrorCoordinateV1<'static> {
    MirrorCoordinateV1 {
        org: "acme",
        name: "http-kit",
        version: "1.2.0",
        sha256: DIGEST,
        format: ArtifactFormat::TarGz,
        vcs_tag: "v1.2.0",
    }
}

#[test]
fn ghcr_of_needs_nothing_but_the_repository_the_package_already_declares() {
    let mirror = MirrorDescriptorV1::ghcr_of(REPOSITORY);
    mirror
        .validate()
        .expect("a ghcr mirror of a real repository");
    assert_eq!(mirror.kind, MirrorKindV1::OciRegistry);
    assert_eq!(mirror.kind.as_str(), "oci-registry");
    assert_eq!(mirror.url.as_deref(), Some(GHCR_URL));
    assert_eq!(mirror.oci_repository_path().unwrap(), "acme/http-kit");
    assert_eq!(mirror.identifier(), "oci-registry:ghcr.io/acme/http-kit");
}

#[test]
fn the_artifact_url_is_the_lockfile_pin_addressed_as_a_blob() {
    // zed addresses artifacts by sha256; the distribution spec addresses blobs
    // by digest. They are the same coordinate, so no template is involved and
    // a mirror cannot serve a different artifact than the lock pinned.
    let mirror = MirrorDescriptorV1::ghcr_of(REPOSITORY);
    assert_eq!(
        mirror.artifact_urls(&coord()).unwrap(),
        vec![format!(
            "https://ghcr.io/v2/acme/http-kit/blobs/sha256:{DIGEST}"
        )]
    );
}

#[test]
fn a_mixed_case_owner_is_lowercased_because_the_registry_requires_it() {
    // `github.com/ORESoftware/Zed-CLI` is a legal forge path and an illegal
    // OCI repository name. Deriving the un-normalized form would produce a
    // mirror that every registry rejects.
    let mirror = MirrorDescriptorV1::ghcr_of("https://github.com/ORESoftware/Zed-CLI");
    mirror.validate().unwrap();
    assert_eq!(mirror.oci_repository_path().unwrap(), "oresoftware/zed-cli");
}

#[test]
fn one_repository_publishing_several_packages_overrides_the_path() {
    let mut mirror = MirrorDescriptorV1::ghcr_of("https://github.com/acme/clients");
    mirror.oci_repository = Some("acme/clients/http-kit-rust".to_owned());
    mirror.validate().unwrap();
    assert!(
        mirror.artifact_urls(&coord()).unwrap()[0]
            .contains("/v2/acme/clients/http-kit-rust/blobs/")
    );
}

#[test]
fn an_oci_mirror_serves_artifacts_and_claims_no_signed_metadata() {
    // An OCI blob is self-verifying against the lockfile pin. A signed version
    // document is not, and there is no agreed layout for one in a container
    // repository, so the descriptor must not assert it can resolve versions.
    let mirror = MirrorDescriptorV1::ghcr_of(REPOSITORY);
    assert!(mirror.serves.artifacts);
    assert!(!mirror.serves.metadata);
    assert!(!mirror.serves.index);
    assert!(mirror.version_metadata_urls(&coord()).is_err());
    assert!(mirror.package_index_urls("acme", "http-kit").is_err());
}

#[test]
fn package_defaults_fill_the_registry_and_the_repository() {
    let mut bare = MirrorDescriptorV1::ghcr_of(REPOSITORY);
    bare.url = None;
    bare.repository = None;
    let filled = bare.with_package_defaults(REPOSITORY, "v{version}");
    assert_eq!(filled.url.as_deref(), Some(GHCR_URL));
    assert_eq!(filled.oci_repository_path().unwrap(), "acme/http-kit");
    // A tag template belongs to release assets, not to a blob address.
    assert!(filled.tag_template.is_none());
}

#[test]
fn a_descriptor_that_names_no_package_is_refused() {
    // A registry base URL names a host, not a package. Accepting it would
    // produce a mirror that can never build an artifact URL.
    let mut mirror = MirrorDescriptorV1::ghcr_of(REPOSITORY);
    mirror.repository = None;
    assert!(mirror.validate().is_err());
}

#[test]
fn fields_belonging_to_another_kind_are_refused_in_both_directions() {
    let mut oci = MirrorDescriptorV1::ghcr_of(REPOSITORY);
    oci.tag_template = Some("v{version}".to_owned());
    assert!(
        oci.validate().is_err(),
        "a blob address has no tag template"
    );

    let mut release = MirrorDescriptorV1::github_release_of(REPOSITORY);
    release.oci_repository = Some("acme/http-kit".to_owned());
    assert!(
        release.validate().is_err(),
        "a release asset has no OCI repository path"
    );
}

#[test]
fn the_reference_reads_the_way_a_puller_would_type_it() {
    let mirror = MirrorDescriptorV1::ghcr_of(REPOSITORY);
    let reference = mirror.oci_reference(&coord()).unwrap();
    assert_eq!(reference.registry, GHCR_URL);
    assert_eq!(reference.repository, "acme/http-kit");
    assert_eq!(reference.tag, "1.2.0");
    assert_eq!(reference.image_reference(), "ghcr.io/acme/http-kit:1.2.0");
}

#[test]
fn the_repository_grammar_matches_what_a_registry_will_accept() {
    for path in ["acme/http-kit", "acme/clients/http_kit.rs", "a1/b2"] {
        assert!(is_oci_repository_path(path), "{path}");
    }
    for path in [
        "Acme/Kit",
        "acme//kit",
        "-acme/kit",
        "acme/kit-",
        "acme/kit!",
        "",
    ] {
        assert!(!is_oci_repository_path(path), "{path}");
    }
}

#[test]
fn an_oci_mirror_is_tried_after_release_assets_and_before_a_raw_tree() {
    // Release assets are one redirect to a CDN; an OCI pull costs an extra
    // token round trip; a raw tree is the worst transport for artifact bytes.
    assert!(
        MirrorKindV1::GithubRelease.default_priority()
            < MirrorKindV1::OciRegistry.default_priority()
    );
    assert!(
        MirrorKindV1::OciRegistry.default_priority() < MirrorKindV1::GithubRaw.default_priority()
    );
}

#[test]
fn identifying_a_descriptor_with_no_derivable_path_does_not_recurse() {
    // `MirrorError` embeds an identifier and `identifier()` needs the
    // repository path, so the fallible derivation must never be used to build
    // one. This is a regression test for a stack overflow, not a style point.
    let mut mirror = MirrorDescriptorV1::ghcr_of(REPOSITORY);
    mirror.repository = None;
    assert_eq!(mirror.identifier(), "oci-registry:ghcr.io");
    assert!(mirror.oci_repository_path().is_err());
}

#[test]
fn the_two_github_surfaces_are_separate_mirrors_with_separate_urls() {
    // The distinction this whole kind exists for: the same package, both
    // surfaces, different transports. A repository that publishes to both gets
    // two descriptors, tried in priority order.
    let release = MirrorDescriptorV1::github_release_of(REPOSITORY)
        .with_package_defaults(REPOSITORY, "v{version}");
    let packages = MirrorDescriptorV1::ghcr_of(REPOSITORY);

    let release_url = &release.artifact_urls(&coord()).unwrap()[0];
    let packages_url = &packages.artifact_urls(&coord()).unwrap()[0];
    assert!(
        release_url.contains("github.com/acme/http-kit/releases/"),
        "{release_url}"
    );
    assert!(
        packages_url.starts_with("https://ghcr.io/v2/"),
        "{packages_url}"
    );
    assert_ne!(release.identifier(), packages.identifier());
    assert!(release.order_key() < packages.order_key());
}
