//! REST API contract between clients (CLI, language SDKs, web UI) and
//! `zed-api-server`. Paths are built by the helpers here so every consumer
//! agrees on the URL scheme; DTOs are the JSON bodies.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::artifact::ArtifactFormat;
use crate::manifest::Manifest;
use crate::vcs::Vcs;

/// Default public registry (production host: zpkg.tech). Override with
/// `--registry` / `ZED_PKG_REGISTRY`; self-hosted deployments point this at
/// their own `zed-api-server`.
pub const DEFAULT_REGISTRY_URL: &str = "https://registry.zpkg.tech";

pub const API_V1: &str = "/v1";

/// Multipart field carrying the JSON-encoded [`PublishMeta`].
pub const PUBLISH_META_FIELD: &str = "meta";
/// Multipart field carrying the artifact archive bytes.
pub const PUBLISH_ARTIFACT_FIELD: &str = "artifact";

/// `GET` — package metadata and version list.
pub fn package_path(org: &str, name: &str) -> String {
    format!("{API_V1}/packages/{org}/{name}")
}

/// `GET` — metadata for one published version.
/// `PUT` (multipart, bearer token) — publish this version.
pub fn version_path(org: &str, name: &str, version: &str) -> String {
    format!("{API_V1}/packages/{org}/{name}/versions/{version}")
}

/// `GET` — download (or get redirected to) the artifact with this sha256.
pub fn artifact_path(sha256: &str) -> String {
    format!("{API_V1}/artifacts/{sha256}")
}

/// `GET ?q=` — search packages.
pub fn search_path() -> String {
    format!("{API_V1}/search")
}

/// `GET` — serve one file out of a published artifact, unpkg-style
/// (`/v1/files/acme/http-kit/1.2.0/dist/style.css`). Lets the web consume
/// package contents directly from the edge without installing.
pub fn file_path(org: &str, name: &str, version: &str, path: &str) -> String {
    format!("{API_V1}/files/{org}/{name}/{version}/{path}")
}

/// `POST` (bearer token) — claim an org namespace.
pub fn orgs_path() -> String {
    format!("{API_V1}/orgs")
}

/// `GET` — liveness probe.
pub fn healthz_path() -> String {
    "/healthz".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PackageSummary {
    pub org: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PackageMetadata {
    pub org: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub vcs: Vcs,
    pub repo_url: String,
    /// How this package's versions should be interpreted (semver by default).
    #[serde(
        default,
        skip_serializing_if = "crate::version::VersionScheme::is_default"
    )]
    pub version_scheme: crate::version::VersionScheme,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest: Option<String>,
    /// All published, non-yanked versions, newest first.
    pub versions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct VersionMetadata {
    pub org: String,
    pub name: String,
    pub version: String,
    pub sha256: String,
    pub size: u64,
    #[serde(default)]
    pub format: ArtifactFormat,
    pub vcs_tag: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vcs_commit: Option<String>,
    /// Absolute or registry-relative URL the artifact can be fetched from.
    pub download_url: String,
    /// RFC 3339 timestamp.
    pub published_at: String,
    #[serde(default)]
    pub yanked: bool,
}

/// JSON half of the multipart publish request; the artifact bytes travel in
/// the [`PUBLISH_ARTIFACT_FIELD`] part.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PublishMeta {
    pub manifest: Manifest,
    /// Tag that exists in the source repository for this version.
    pub vcs_tag: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vcs_commit: Option<String>,
    /// Client-computed sha256 of the uploaded archive; the server recomputes
    /// and rejects on mismatch.
    pub sha256: String,
    pub size: u64,
    #[serde(default)]
    pub format: ArtifactFormat,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PublishResponse {
    pub org: String,
    pub name: String,
    pub version: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ClaimOrgRequest {
    pub slug: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ClaimOrgResponse {
    pub slug: String,
    /// False when the caller already owned the org.
    pub created: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SearchResponse {
    pub query: String,
    pub items: Vec<PackageSummary>,
}

/// Error body returned with any non-2xx status.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ApiError {
    /// Stable machine-readable code, e.g. `not_found`, `sha256_mismatch`,
    /// `tag_not_found`, `unauthorized`, `org_taken`.
    pub code: String,
    pub message: String,
}
