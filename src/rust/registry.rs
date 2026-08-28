//! REST API contract between clients (CLI, language SDKs, web UI) and
//! `zed-api-server`. Paths are built by the helpers here so every consumer
//! agrees on the URL scheme; DTOs are the JSON bodies.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::artifact::ArtifactFormat;
use crate::binary_artifact::BinaryArchiveFormatV1;
use crate::manifest::Manifest;
use crate::mirror::MirrorDescriptorV1;
use crate::signing::{
    DetachedSignatureV1, IndexAttestationV1, PublisherKeyV1, SIGNED_VERSION_SCHEMA_V1,
    SignedVersionV1, SigningError, VersionAttestationV1,
};
use crate::vcs::Vcs;

/// Default public registry (production host: zpkg.net). Override with
/// `--registry` / `ZED_PKG_REGISTRY`; self-hosted deployments point this at
/// their own `zed-api-server`.
pub const DEFAULT_REGISTRY_URL: &str = "https://registry.zpkg.net";

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

/// `GET` — list every platform artifact published for one release.
pub fn binary_artifacts_path(org: &str, name: &str, version: &str) -> String {
    format!("{}/artifacts", version_path(org, name, version))
}

/// `GET` — fetch metadata for one immutable release/target/format artifact.
/// `PUT` (multipart, bearer token) — publish the same artifact identity.
///
/// The target and format are separate path segments and never SemVer build
/// metadata. Callers must pass a target already accepted by
/// `BinaryPlatformV1::validate`; path construction deliberately performs no
/// lossy URL normalization.
pub fn binary_artifact_path(
    org: &str,
    name: &str,
    version: &str,
    target: &str,
    format: BinaryArchiveFormatV1,
) -> String {
    format!(
        "{}/{target}/{format}",
        binary_artifacts_path(org, name, version)
    )
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

/// `GET ?limit=&action=&before=` (bearer token, org `owner` or admin) — the
/// org's audit log: who changed published state, what, and when (zed-docs
/// issue #7 governance). `action` filters to one action; `before` pages
/// backwards by taking entries with a lower `seq` than the one given.
pub fn audit_path(org: &str) -> String {
    format!("{API_V1}/orgs/{org}/audit")
}

/// `GET` (bearer token, org `owner` or admin) — walk the org's audit chain and
/// report whether it is intact. See [`audit_chain_preimage`].
pub fn audit_verify_path(org: &str) -> String {
    format!("{API_V1}/orgs/{org}/audit/verify")
}

/// `GET` — an org's publisher signing keys (public halves only).
/// `PUT` (bearer token, org `owner`) — enroll, retire, or revoke a key.
///
/// Anonymous read on purpose. These keys are the thing a client needs in order
/// to *stop* trusting the registry for metadata, so gating them behind a
/// credential would defeat the mechanism they exist for.
pub fn org_keys_path(org: &str) -> String {
    format!("{API_V1}/orgs/{org}/keys")
}

/// `GET` — the mirror set this registry advertises for its own contents.
///
/// Also served at [`crate::mirror::MIRROR_BOOTSTRAP_PATH`], which is where a
/// client looks when the registry is the thing that is down.
pub fn mirrors_path() -> String {
    format!("{API_V1}/mirrors")
}

/// `GET` — a package's signed version index, as mirrors serve it.
///
/// The same document a client would fetch from a bucket or a forge, served by
/// the registry too, so a client verifies one shape everywhere and switching
/// transports changes nothing about how the answer is checked.
pub fn signed_index_path(org: &str, name: &str) -> String {
    format!("{API_V1}/packages/{org}/{name}/signed-index")
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
    /// Where this package's artifacts and metadata can be fetched when this
    /// registry cannot answer, in try order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mirrors: Vec<MirrorDescriptorV1>,
    /// The org's publisher signing keys, inlined so a client that resolves a
    /// package also learns how to verify a mirror's answer next time — without
    /// a second round trip it may not get to make.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signing_keys: Vec<PublisherKeyV1>,
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
    /// RFC 3339 timestamp, asserted by the publisher and echoed verbatim.
    ///
    /// Verbatim matters: it is covered by [`Self::signatures`], so a registry
    /// that "helpfully" normalized it would invalidate every signature it
    /// serves.
    pub published_at: String,
    #[serde(default)]
    pub yanked: bool,
    /// Where these exact bytes can be fetched, in try order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mirrors: Vec<MirrorDescriptorV1>,
    /// Publisher signatures over [`Self::attestation`].
    ///
    /// Absent for versions published before signing, and for publishers who
    /// have not enrolled a key. Absence degrades cleanly: such a package still
    /// installs from any mirror against a lockfile pin, and only loses the
    /// ability to have its *ranges* resolved while the registry is down.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signatures: Vec<DetachedSignatureV1>,
}

impl VersionMetadata {
    /// Rebuild the signed document from the wire fields.
    ///
    /// Reconstruction rather than transport: sending the attestation alongside
    /// the fields it duplicates invites the two to disagree, and a verifier
    /// then has to pick one. Deriving it in shared code means the server, the
    /// CLI, and every SDK produce the same bytes by construction.
    pub fn attestation(&self) -> VersionAttestationV1 {
        VersionAttestationV1 {
            org: self.org.clone(),
            name: self.name.clone(),
            version: self.version.clone(),
            sha256: self.sha256.clone(),
            size: self.size,
            format: self.format,
            vcs_tag: self.vcs_tag.clone(),
            vcs_commit: self.vcs_commit.clone().unwrap_or_default(),
            published_at: self.published_at.clone(),
            mirrors: self.mirrors.clone(),
        }
    }

    /// The signed document a mirror would serve for this version.
    pub fn signed_version(&self) -> Option<SignedVersionV1> {
        if self.signatures.is_empty() {
            return None;
        }
        Some(SignedVersionV1 {
            schema: SIGNED_VERSION_SCHEMA_V1.to_owned(),
            payload: self.attestation(),
            signatures: self.signatures.clone(),
        })
    }

    /// The exact bytes [`Self::signatures`] cover.
    pub fn signing_preimage(&self) -> Result<Vec<u8>, SigningError> {
        crate::signing::version_attestation_preimage(&self.attestation())
    }
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
    /// Fully-resolved mirror set for this version. The server stores it and
    /// serves it back; it never derives one, because deriving it would mean
    /// re-reading the publisher's manifest at a time when the manifest may
    /// have moved on.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mirrors: Vec<MirrorDescriptorV1>,
    /// RFC 3339 publication timestamp asserted by the publisher.
    ///
    /// Publisher-assigned rather than server-assigned because it is inside the
    /// signed payload: a signature can only cover fields its signer knows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_at: Option<String>,
    /// Publisher signatures over the reconstructed attestation. The server
    /// verifies them against the org's enrolled keys before accepting, so an
    /// unverifiable signature fails the publish rather than becoming a
    /// permanent record nobody can check.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signatures: Vec<DetachedSignatureV1>,
}

/// Body of `PUT /v1/orgs/{org}/keys` — enroll, retire, or revoke a key.
///
/// The whole set is submitted, never a delta: a client that can only add keys
/// cannot express a revocation, and a client that can only remove them leaves
/// no record of why.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OrgKeysRequest {
    pub keys: Vec<PublisherKeyV1>,
}

/// Response for `GET /v1/orgs/{org}/keys`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OrgKeysResponse {
    pub org: String,
    pub keys: Vec<PublisherKeyV1>,
}

/// Response for `GET /v1/mirrors` and the well-known bootstrap path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MirrorsResponse {
    pub registry_url: String,
    pub mirrors: Vec<MirrorDescriptorV1>,
}

/// Response for `GET /v1/packages/{org}/{name}/signed-index`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SignedIndexResponse {
    pub schema: String,
    pub payload: IndexAttestationV1,
    pub signatures: Vec<DetachedSignatureV1>,
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
    /// Position in the org's append-only chain, starting at 1. Gaps mean
    /// entries were deleted. Defaults to 0 when read from a server that
    /// predates the chain.
    #[serde(default)]
    pub seq: u64,
    /// `sha256(audit_chain_preimage(..))` for this entry, lowercase hex. Empty
    /// from a pre-chain server.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub entry_hash: String,
    /// The previous entry's `entry_hash`; `None` for the first entry in an
    /// org's chain. Linking each entry to its predecessor is what makes a
    /// silent deletion or edit detectable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prev_hash: Option<String>,
}

/// The exact bytes an audit entry's `entry_hash` is computed over.
///
/// Defined here, in the shared contract, so the server and *any* client derive
/// byte-identical input and a client can verify the chain itself rather than
/// trusting the server's own verdict — the point of a tamper-evident log is
/// that the party who could tamper is not the only party who can check.
///
/// Every field is length-prefixed (`<byte-len>:<value>`). A plain separator
/// would be forgeable: a token named `x|publish` could otherwise shift field
/// boundaries and reproduce another entry's digest. Length prefixes make the
/// encoding unambiguous, so distinct entries can never share a preimage.
///
/// `at` must be the RFC 3339 timestamp exactly as stored/serialized, and
/// `prev_hash` is the empty string for the first entry in a chain.
///
/// The fields are named rather than positional on purpose: with ten strings in
/// a row, transposing `subject` and `actor_token_name` would compile silently
/// and quietly change every digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuditChainInput<'a> {
    pub org_id: &'a str,
    pub seq: u64,
    /// RFC 3339, exactly as stored and serialized.
    pub at: &'a str,
    pub action: &'a str,
    pub subject: &'a str,
    pub actor_token_id: Option<&'a str>,
    pub actor_token_name: &'a str,
    pub actor_role: &'a str,
    pub detail: Option<&'a str>,
    /// The previous entry's hash; empty for the first entry in a chain.
    pub prev_hash: &'a str,
}

/// Build the canonical preimage for [`AuditChainInput`]. See the module note
/// on why every field is length-prefixed.
pub fn audit_chain_preimage(input: &AuditChainInput<'_>) -> String {
    fn field(out: &mut String, value: &str) {
        out.push_str(&value.len().to_string());
        out.push(':');
        out.push_str(value);
    }
    let mut out = String::new();
    field(&mut out, input.org_id);
    field(&mut out, &input.seq.to_string());
    field(&mut out, input.at);
    field(&mut out, input.action);
    field(&mut out, input.subject);
    field(&mut out, input.actor_token_id.unwrap_or(""));
    field(&mut out, input.actor_token_name);
    field(&mut out, input.actor_role);
    field(&mut out, input.detail.unwrap_or(""));
    field(&mut out, input.prev_hash);
    out
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AuditLogResponse {
    pub org: String,
    /// Most recent first.
    pub entries: Vec<AuditEntry>,
}

/// The result of walking an org's audit chain end to end.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AuditIntegrityResponse {
    pub org: String,
    /// True only when every entry's hash recomputes and every link matches.
    pub intact: bool,
    pub entries_checked: u64,
    /// The `seq` where verification first failed, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_bad_seq: Option<u64>,
    /// Machine-readable failure kind: `hash_mismatch` (an entry was edited),
    /// `broken_link` (an entry's `prev_hash` does not match its predecessor),
    /// or `sequence_gap` (an entry was deleted).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub problem: Option<String>,
    /// The newest entry's hash — an anchor an operator can record externally
    /// so that later truncation of the whole tail is also detectable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub head_hash: Option<String>,
}

/// Error body returned with any non-2xx status.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ApiError {
    /// Stable machine-readable code, e.g. `not_found`, `sha256_mismatch`,
    /// `tag_not_found`, `unauthorized`, `org_taken`.
    pub code: String,
    pub message: String,
}
