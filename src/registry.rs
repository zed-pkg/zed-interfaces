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

/// `GET ?q=` — search packages by name/description. Also accepts repeatable
/// `?tag=` filters (a package must carry all given tags) and `?limit=`.
pub fn search_path() -> String {
    format!("{API_V1}/search")
}

/// `GET` — list all packages, newest first. Accepts `?tag=` (repeatable),
/// `?limit=`, and `?offset=` for pagination.
pub fn packages_list_path() -> String {
    format!("{API_V1}/packages")
}

/// `POST` — RAG / semantic search: nearest packages to a query embedding
/// within one model's space. Body is [`SemanticSearchRequest`].
pub fn semantic_search_path() -> String {
    format!("{API_V1}/search/semantic")
}

/// `PUT` (bearer token) — upsert a package's embedding for one model. Body is
/// [`EmbeddingUpsertRequest`].
pub fn embedding_path(org: &str, name: &str) -> String {
    format!("{API_V1}/packages/{org}/{name}/embedding")
}

/// `GET` — serve one file out of a published artifact, unpkg-style
/// (`/v1/files/acme/http-kit/1.2.0/dist/style.css`). Lets the web consume
/// package contents directly from the edge without installing.
pub fn file_path(org: &str, name: &str, version: &str, path: &str) -> String {
    format!("{API_V1}/files/{org}/{name}/{version}/{path}")
}

/// `POST` (bearer token, org-scoped) — mark a published version as yanked
/// (or restore it). Yanked versions stay downloadable for existing
/// lockfiles but are hidden from resolution and search.
pub fn yank_path(org: &str, name: &str, version: &str) -> String {
    format!("{API_V1}/packages/{org}/{name}/versions/{version}/yank")
}

/// `POST` (bearer token) — claim an org namespace.
pub fn orgs_path() -> String {
    format!("{API_V1}/orgs")
}

/// `GET ?limit=` (bearer token, org `owner` or admin) — the org's audit log:
/// who changed published state, what, and when (zed-docs issue #7 governance).
pub fn audit_path(org: &str) -> String {
    format!("{API_V1}/orgs/{org}/audit")
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
    /// Free-form tags for filtering/discovery.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
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
    /// Free-form tags for filtering/discovery (multi-tag lookup).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
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

/// Body for the yank route. `yanked: false` restores a yanked version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct YankRequest {
    pub yanked: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct YankResponse {
    pub org: String,
    pub name: String,
    pub version: String,
    pub yanked: bool,
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

/// Response for `GET /v1/packages` (list all).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PackageListResponse {
    pub items: Vec<PackageSummary>,
    /// Total packages matching the filter (before limit/offset).
    pub total: u64,
}

/// Body of `POST /v1/search/semantic` (RAG). The caller computes the query
/// embedding with its model; the server ranks stored package embeddings from
/// the SAME model by cosine distance. Vectors up to 2050 dims are accepted and
/// zero-padded server-side.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SemanticSearchRequest {
    /// Embedding model id, e.g. `openai/text-embedding-3-small`. Only stored
    /// embeddings from this model are searched.
    pub model: String,
    /// The query embedding (native width; padded to 2050 server-side).
    pub embedding: Vec<f32>,
    #[serde(default = "default_semantic_limit")]
    pub limit: u32,
    /// Optional tag filter: results must carry all of these tags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

fn default_semantic_limit() -> u32 {
    20
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SemanticHit {
    pub org: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Cosine distance (0 = identical direction, 2 = opposite). Lower is nearer.
    pub distance: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SemanticSearchResponse {
    pub items: Vec<SemanticHit>,
}

/// Body of `PUT /v1/packages/{org}/{name}/embedding` — upsert a package's
/// embedding for one model (re-embedding replaces in place).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EmbeddingUpsertRequest {
    pub model: String,
    /// The embedding (native width; padded to 2050 server-side).
    pub embedding: Vec<f32>,
    /// The text that was embedded (kept for provenance / re-embedding).
    pub content: String,
}

/// A state-changing action recorded in an org's audit log. Reads are never
/// audited — only mutations of published state and of the namespace itself, so
/// the log answers "who changed what" without drowning in traffic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    /// A version was published.
    Publish,
    /// A version was yanked (hidden from fresh resolution).
    Yank,
    /// A previously yanked version was restored.
    Unyank,
    /// The org namespace was claimed.
    OrgClaim,
}

impl AuditAction {
    pub fn as_str(&self) -> &'static str {
        match self {
            AuditAction::Publish => "publish",
            AuditAction::Yank => "yank",
            AuditAction::Unyank => "unyank",
            AuditAction::OrgClaim => "org_claim",
        }
    }

    /// Parse a stored action string; unknown values are preserved as `None` so
    /// a newer server's rows never break an older client's read.
    pub fn parse(s: &str) -> Option<AuditAction> {
        match s {
            "publish" => Some(AuditAction::Publish),
            "yank" => Some(AuditAction::Yank),
            "unyank" => Some(AuditAction::Unyank),
            "org_claim" => Some(AuditAction::OrgClaim),
            _ => None,
        }
    }
}

/// One audit-log record. The actor is identified by the *token* that acted —
/// its name and role, never its secret — which is the identity a registry
/// actually has (zed-docs issue #7 governance).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AuditEntry {
    /// RFC 3339 timestamp of the action.
    pub at: String,
    /// Raw action string; `action_kind` is the parsed form when recognized.
    pub action: String,
    /// Parsed action, absent when this server build doesn't recognize it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_kind: Option<AuditAction>,
    /// What was acted on, e.g. `acme/http-kit@1.2.0` or the org slug.
    pub subject: String,
    /// Human-readable name of the token that acted.
    pub actor_token_name: String,
    /// The acting token's role (`owner`/`publisher`/`reader`, or `admin` for
    /// unscoped tokens).
    pub actor_role: String,
    /// Extra context, e.g. the artifact sha256 for a publish.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AuditLogResponse {
    pub org: String,
    /// Most recent first.
    pub entries: Vec<AuditEntry>,
}

/// Error body returned with any non-2xx status.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ApiError {
    /// Stable machine-readable code, e.g. `not_found`, `sha256_mismatch`,
    /// `tag_not_found`, `unauthorized`, `org_taken`.
    pub code: String,
    pub message: String,
}
