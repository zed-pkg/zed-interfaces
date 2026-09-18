//! Typed registry / CDN / GitHub route keys.
//!
//! Generated from `route-maps/zed-api.route-map.json` by
//! `github.com/oresoftware/api-docs` (`scripts/generate-routes.py`). The JSON
//! map is the source; do not edit `generated/` by hand. HTTP, TCP NDJSON, and
//! WebSocket share the same call/receipt frame — only the wire changes.

#[path = "../../generated/rust/src/zed_api.rs"]
#[rustfmt::skip]
mod generated;

pub use generated::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::ArtifactFormat;
    use crate::registry::{
        artifact_path, audit_path, audit_verify_path, embedding_path, file_path, healthz_path,
        orgs_path, package_path, packages_list_path, search_path, semantic_search_path,
        version_path, yank_path,
    };
    use crate::source::{
        ArtifactQuery, DEFAULT_GITHUB_WEB, DEFAULT_R2_PUBLIC_BASE, R2_CONTENT_PREFIX,
        R2_GITHUB_PREFIX, R2_PACKAGE_PREFIX, github_identity_for, github_release_asset_names,
        github_release_download_url, r2_object_keys,
    };

    fn expand(template: &str, vars: &[(&str, &str)]) -> String {
        let mut out = template.to_string();
        for (key, value) in vars {
            out = out.replace(&format!("{{{key}}}"), value);
        }
        out
    }

    #[test]
    fn generated_registry_templates_match_helpers() {
        let org = "acme";
        let name = "http-kit";
        let version = "1.2.0";
        let sha = "abc";
        let pkg = &[("org", org), ("name", name)];
        let ver = &[("org", org), ("name", name), ("version", version)];
        assert_eq!(
            expand(RouteKey::GetPackage.path(), pkg),
            package_path(org, name)
        );
        assert_eq!(
            expand(RouteKey::GetVersion.path(), ver),
            version_path(org, name, version)
        );
        assert_eq!(
            expand(RouteKey::GetArtifact.path(), &[("sha256", sha)]),
            artifact_path(sha)
        );
        assert_eq!(RouteKey::Search.path(), search_path());
        assert_eq!(RouteKey::ListPackages.path(), packages_list_path());
        assert_eq!(RouteKey::SemanticSearch.path(), semantic_search_path());
        assert_eq!(
            expand(RouteKey::UpsertEmbedding.path(), pkg),
            embedding_path(org, name)
        );
        assert_eq!(
            expand(
                RouteKey::GetFile.path(),
                &[
                    ("org", org),
                    ("name", name),
                    ("version", version),
                    ("path", "dist/style.css"),
                ],
            ),
            file_path(org, name, version, "dist/style.css")
        );
        assert_eq!(
            expand(RouteKey::Yank.path(), ver),
            yank_path(org, name, version)
        );
        assert_eq!(RouteKey::ClaimOrg.path(), orgs_path());
        assert_eq!(
            expand(RouteKey::GetAudit.path(), &[("org", org)]),
            audit_path(org)
        );
        assert_eq!(
            expand(RouteKey::VerifyAudit.path(), &[("org", org)]),
            audit_verify_path(org)
        );
        assert_eq!(RouteKey::Healthz.path(), healthz_path());
    }

    #[test]
    fn get_version_is_http_and_tcp_ndjson() {
        assert_eq!(RouteKey::GetVersion.methods(), &["GET", "PUT"]);
        assert_eq!(RouteKey::GetVersion.transports(), &["http", "tcp"]);
        assert_eq!(RouteKey::RegistryEvents.transports(), &["websocket"]);
    }

    #[test]
    fn cdn_and_github_templates_match_guessable_keys() {
        let org = "acme";
        let name = "http-kit";
        let version = "1.2.0";
        let tag = "v1.2.0";
        let sha = "ab".repeat(32);
        let ext = "tar.gz";
        let filename = format!("{name}-{version}.{ext}");
        let query = ArtifactQuery {
            org,
            name,
            version,
            vcs_tag: tag,
            sha256: Some(sha.as_str()),
            format: ArtifactFormat::TarGz,
            repo_url: Some("https://github.com/acme/http-kit"),
            artifacts: None,
            registry_base: None,
            r2_public_base: None,
            r2_public_key: None,
        };
        let keys = r2_object_keys(&query);
        let github_key = format!("{R2_GITHUB_PREFIX}/acme/http-kit/{tag}/{filename}");
        let package_key = format!("{R2_PACKAGE_PREFIX}/{org}/{name}/{version}/{filename}");
        let content_key = format!("{R2_CONTENT_PREFIX}/{sha}.{ext}");
        assert_eq!(
            keys,
            vec![github_key.clone(), package_key.clone(), content_key.clone()]
        );
        assert_eq!(
            expand(
                RouteKey::CdnGithubObject.path(),
                &[
                    ("owner", "acme"),
                    ("repo", "http-kit"),
                    ("tag", tag),
                    ("filename", &filename),
                ],
            ),
            format!("/{github_key}")
        );
        assert_eq!(
            expand(
                RouteKey::CdnPackageObject.path(),
                &[
                    ("org", org),
                    ("name", name),
                    ("version", version),
                    ("filename", &filename),
                ],
            ),
            format!("/{package_key}")
        );
        assert_eq!(
            expand(
                RouteKey::CdnContentObject.path(),
                &[("sha256", &sha), ("ext", ext)],
            ),
            format!("/{content_key}")
        );
        let cdn = format!("{DEFAULT_R2_PUBLIC_BASE}/{package_key}");
        assert!(cdn.starts_with("https://cdn.zpkg.net/packages/"));

        let identity = github_identity_for(org, name, Some("https://github.com/acme/http-kit"));
        let asset = &github_release_asset_names(org, name, version, ext)[0];
        assert_eq!(
            format!(
                "{DEFAULT_GITHUB_WEB}{}",
                expand(
                    RouteKey::GithubReleaseAsset.path(),
                    &[
                        ("owner", "acme"),
                        ("repo", "http-kit"),
                        ("tag", tag),
                        ("asset", asset),
                    ],
                )
            ),
            github_release_download_url(&identity, tag, asset)
        );
    }
}
