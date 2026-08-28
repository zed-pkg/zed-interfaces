//! Mirror descriptors: where a published artifact can be fetched from when the
//! canonical registry cannot answer.
//!
//! zed's trust model is content addressing. A `.zpkg.lock` entry pins the
//! artifact's sha256, and [`crate::lockfile::LockedPackage`] is only satisfied
//! by bytes that hash to that pin. That property is what makes mirroring safe:
//! a mirror is a *transport*, never an authority. It cannot substitute an
//! artifact, only fail to produce one.
//!
//! Resolution — turning `^1.2` into `1.2.4` — is a different question, because
//! there is no pin yet. A mirror that serves metadata must therefore carry a
//! publisher signature over that metadata (see [`crate::signing`]); a mirror
//! that serves only artifact bytes needs no signature at all, and the common
//! `zed install --frozen` path works against an unsigned bucket.
//!
//! The kinds here are deliberately concrete rather than a generic "URL
//! template" escape hatch. A `github-release` mirror knows that GitHub serves
//! release assets at a predictable public path, so a package that declares
//! nothing beyond its own `[package.repository]` already has a working mirror.
//! Templates exist for the non-standard layouts that would otherwise be
//! unrepresentable, not as the primary interface.
//!
//! Nothing in this module performs I/O. It computes candidate URLs; deciding
//! which to try, in what order, and what to do when one fails belongs to the
//! client.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::artifact::ArtifactFormat;

/// Mirror-descriptor schema marker.
pub const MIRROR_DESCRIPTOR_SCHEMA_V1: &str = "zpkg.mirror-descriptor/v1";
/// Mirror bootstrap-document schema marker.
pub const MIRROR_BOOTSTRAP_SCHEMA_V1: &str = "zpkg.mirror-bootstrap/v1";

/// Well-known path serving the [`MirrorBootstrapV1`] document.
///
/// A client that cannot reach the registry also cannot ask the registry where
/// its mirrors are. The bootstrap document breaks that circularity: it is
/// served by every mirror that can serve metadata, so reaching *any* one of
/// them recovers the full set — including hosts in DNS zones the outage does
/// not touch.
pub const MIRROR_BOOTSTRAP_PATH: &str = "/.well-known/zpkg-mirrors.json";

/// Default object-store artifact layout.
///
/// Byte-identical to `artifact_key()` in `zed-api-server` and to the
/// `artifacts/` directory of a `file://` registry, so a bucket sync, a
/// directory mirror, and the production store are the same layout at three
/// scales.
pub const DEFAULT_ARTIFACT_TEMPLATE: &str = "artifacts/{sha256}.{ext}";
/// Default object-store layout for one version's signed metadata document.
pub const DEFAULT_VERSION_TEMPLATE: &str = "metadata/{org}/{name}/versions/{version}.json";
/// Default object-store layout for a package's signed version index.
pub const DEFAULT_INDEX_TEMPLATE: &str = "metadata/{org}/{name}/index.json";

/// Default prefix for zed-owned GitHub release assets. The prefix keeps zed's
/// assets from colliding with the human-facing binaries a project already
/// attaches to the same release.
pub const DEFAULT_ASSET_PREFIX: &str = "zpkg-";
/// Default tag of the rolling release that carries package-level indexes.
///
/// Version metadata lives on the immutable release for that version; the index
/// changes every publish and therefore cannot. A single rolling release per
/// repository keeps the mutable part to exactly one well-known place.
pub const DEFAULT_INDEX_TAG: &str = "zpkg-index";
/// Default branch holding a raw-served mirror tree.
pub const DEFAULT_RAW_BRANCH: &str = "zpkg-mirror";
/// Default path prefix within a raw-served mirror tree.
pub const DEFAULT_RAW_PREFIX: &str = "zpkg";

/// Host serving `raw.githubusercontent.com` content for github.com.
pub const GITHUB_RAW_HOST: &str = "raw.githubusercontent.com";
/// Canonical github.com host.
pub const GITHUB_HOST: &str = "github.com";

/// Public content-addressed CDN in front of the production artifact bucket.
pub const DEFAULT_CDN_URL: &str = "https://cdn.zpkg.net";
/// The same CDN, reached through a hostname in a different DNS zone.
///
/// This is the whole point of the alternate: `cdn.zpkg.net` and
/// `registry.zpkg.net` share a zone, so a zone-level failure — an expired
/// registration, a bad DNS change, a Cloudflare zone suspension — takes both
/// down at once. A `workers.dev` hostname resolves through a zone zed does not
/// own and cannot break, in front of the same bucket. It is the route that
/// survives losing `zpkg.net` entirely.
pub const DEFAULT_CDN_ALTERNATE_URL: &str = "https://zpkg-cdn.zed-pkg.workers.dev";

/// Mirrors every client knows about without configuring anything.
///
/// Applied only when the client is pointed at the public registry. A
/// self-hosted deployment gets no implicit hosts: silently reaching out to
/// zed's CDN from an air-gapped or enterprise install would be a surprise, and
/// a bad one.
pub fn default_public_mirrors(registry_url: &str) -> Vec<MirrorDescriptorV1> {
    let trimmed = registry_url.trim().trim_end_matches('/');
    let is_public = trimmed == crate::registry::DEFAULT_REGISTRY_URL
        || trimmed.ends_with("://zpkg.net")
        || trimmed.ends_with(".zpkg.net");
    if !is_public {
        return Vec::new();
    }
    let mut cdn = MirrorDescriptorV1::object_store(DEFAULT_CDN_URL);
    cdn.id = Some("zpkg-cdn".to_owned());
    cdn.alternate_urls = vec![DEFAULT_CDN_ALTERNATE_URL.to_owned()];
    vec![cdn]
}

/// Maximum mirrors one package may declare. Bounded so a hostile or careless
/// manifest cannot turn a single failed install into a hundred-host scan.
pub const MAX_MIRRORS: usize = 16;

/// What a mirror is able to serve.
///
/// Artifact bytes are self-verifying against the lockfile pin. Metadata and
/// indexes are not, so a mirror that advertises them is asserting that it
/// carries publisher signatures; a client that cannot verify one must treat
/// the answer as absent rather than as trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MirrorServesV1 {
    /// Artifact archives, addressed by sha256.
    #[serde(default = "yes")]
    pub artifacts: bool,
    /// Signed per-version metadata documents.
    #[serde(default = "yes")]
    pub metadata: bool,
    /// Signed package-level version indexes (what makes range resolution
    /// possible while the registry is unreachable).
    #[serde(default = "yes")]
    pub index: bool,
}

fn yes() -> bool {
    true
}

impl Default for MirrorServesV1 {
    fn default() -> Self {
        Self {
            artifacts: true,
            metadata: true,
            index: true,
        }
    }
}

impl MirrorServesV1 {
    /// A mirror that carries bytes only. The safest thing to point at an
    /// unsigned bucket, and enough for every frozen install.
    pub const ARTIFACTS_ONLY: Self = Self {
        artifacts: true,
        metadata: false,
        index: false,
    };

    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }

    pub fn serves_nothing(&self) -> bool {
        !self.artifacts && !self.metadata && !self.index
    }
}

/// Transport family of a mirror. Each kind implies a URL derivation and a set
/// of fields that are meaningful; [`MirrorDescriptorV1::validate`] rejects
/// fields belonging to a different kind rather than ignoring them, so a
/// misplaced key is a loud error instead of a mirror that silently never
/// resolves.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum MirrorKindV1 {
    /// Another `zed-api-server`, spoken to with the ordinary registry API.
    ZedRegistry,
    /// A content-addressed HTTP object store or CDN in front of one (R2, S3,
    /// MinIO). Public read, no credential, no presign: the sha256 in the path
    /// is both the address and the integrity check.
    ObjectStore,
    /// Release assets on a Git forge that serves them at a public, predictable
    /// path. Named for GitHub because that is the deployment that matters, but
    /// GitHub Enterprise, Gitea, and Forgejo use the same route shape.
    GithubRelease,
    /// A branch or Pages site served as raw files. Cheap and cacheable, which
    /// makes it the best index transport; poor for artifact bytes, which is
    /// why `serves.artifacts` defaults off for this kind.
    GithubRaw,
    /// A local directory in `file://` registry layout — an air-gapped mirror,
    /// a warmed CI cache, or a Nix store input.
    Directory,
}

impl MirrorKindV1 {
    pub fn as_str(&self) -> &'static str {
        match self {
            MirrorKindV1::ZedRegistry => "zed-registry",
            MirrorKindV1::ObjectStore => "object-store",
            MirrorKindV1::GithubRelease => "github-release",
            MirrorKindV1::GithubRaw => "github-raw",
            MirrorKindV1::Directory => "directory",
        }
    }

    /// Default priority when the descriptor does not state one. Ascending, so
    /// the cheapest and most available transport is tried first: a CDN edge
    /// before a forge API, a forge before another full registry.
    pub fn default_priority(&self) -> u32 {
        match self {
            MirrorKindV1::Directory => 10,
            MirrorKindV1::ObjectStore => 20,
            MirrorKindV1::GithubRelease => 30,
            MirrorKindV1::GithubRaw => 40,
            MirrorKindV1::ZedRegistry => 50,
        }
    }
}

/// One place a package's artifacts and/or metadata can be fetched from.
///
/// Authors write the shortest form that identifies the mirror and the derived
/// defaults fill in the rest — a `github-release` mirror with no fields at all
/// means "the release assets on my own `[package.repository]`".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MirrorDescriptorV1 {
    pub kind: MirrorKindV1,
    /// Stable identifier, used in diagnostics and to deduplicate a merged
    /// mirror set. Derived from the kind and host when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Base URL. Required for `object-store` and `zed-registry`; optional for
    /// the forge kinds, where it overrides the host derived from `repository`
    /// (a Pages origin, or a CDN in front of raw content).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Additional base URLs serving byte-identical content at a different
    /// hostname.
    ///
    /// This is the field that survives a zone-level outage. `cdn.zpkg.net` and
    /// a `workers.dev` hostname front the same bucket, but they fail
    /// independently, because the second does not resolve through the
    /// `zpkg.net` zone at all. A client tries every base URL of a mirror
    /// before moving to the next mirror.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternate_urls: Vec<String>,
    /// Ascending try order. Ties break on `id` so ordering is total and a
    /// merged set from several sources is deterministic.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<u32>,
    #[serde(default, skip_serializing_if = "MirrorServesV1::is_default")]
    pub serves: MirrorServesV1,
    /// Source repository, for the forge kinds. Accepts the same spellings as
    /// `[package.repository].url`, including scp-style `git@host:owner/repo`.
    /// Defaults to the package's own declared repository.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    /// Release tag carrying this version's assets. Defaults to the package's
    /// `[publish].tag_format`, so mirrored artifacts hang off the exact tag
    /// zed already verifies for provenance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag_template: Option<String>,
    /// Prefix on zed-owned release asset names. Defaults to [`DEFAULT_ASSET_PREFIX`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_prefix: Option<String>,
    /// Tag of the rolling release carrying package indexes. Defaults to
    /// [`DEFAULT_INDEX_TAG`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index_tag: Option<String>,
    /// Branch of a raw-served mirror tree. Defaults to [`DEFAULT_RAW_BRANCH`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Artifact key template. Only for a store whose layout is not
    /// [`DEFAULT_ARTIFACT_TEMPLATE`] — a sharded bucket, or one where zed's
    /// objects live under a prefix shared with other tooling.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_template: Option<String>,
    /// Version-metadata key template; see [`DEFAULT_VERSION_TEMPLATE`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version_template: Option<String>,
    /// Package-index key template; see [`DEFAULT_INDEX_TEMPLATE`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index_template: Option<String>,
    /// Absolute local path, for `directory` mirrors.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// The coordinates a URL is derived for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MirrorCoordinateV1<'a> {
    pub org: &'a str,
    pub name: &'a str,
    pub version: &'a str,
    /// Lowercase hex digest of the artifact archive.
    pub sha256: &'a str,
    pub format: ArtifactFormat,
    /// The provenance tag this version was published from, e.g. `v1.2.0`.
    pub vcs_tag: &'a str,
}

/// A forge repository split into the parts a URL needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoRefV1 {
    pub host: String,
    pub owner: String,
    pub repo: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MirrorError {
    #[error("mirror `{id}` of kind `{kind}` requires field `{field}`")]
    MissingField {
        id: String,
        kind: &'static str,
        field: &'static str,
    },
    #[error("mirror `{id}` of kind `{kind}` does not accept field `{field}`")]
    UnexpectedField {
        id: String,
        kind: &'static str,
        field: &'static str,
    },
    #[error("mirror `{id}` field `{field}` has invalid value `{value}`")]
    InvalidValue {
        id: String,
        field: &'static str,
        value: String,
    },
    #[error("mirror `{id}` template `{field}` uses unknown placeholder `{token}`")]
    UnknownPlaceholder {
        id: String,
        field: &'static str,
        token: String,
    },
    #[error(
        "mirror `{id}` serves nothing; remove it or enable at least one of artifacts/metadata/index"
    )]
    ServesNothing { id: String },
    #[error("mirror `{id}` cannot serve {what}")]
    Unsupported { id: String, what: &'static str },
    #[error("duplicate mirror id `{0}`")]
    DuplicateId(String),
    #[error("too many mirrors: {0} declared, at most {MAX_MIRRORS} allowed")]
    TooMany(usize),
    #[error("could not read `{value}` as a repository reference")]
    UnparsableRepository { value: String },
}

impl MirrorDescriptorV1 {
    /// A mirror pointing at the release assets of the package's own repository.
    /// The zero-configuration case: provenance tag, artifact, and signed
    /// metadata all on one immutable GitHub release.
    pub fn github_release_of(repository: &str) -> Self {
        Self {
            kind: MirrorKindV1::GithubRelease,
            repository: Some(repository.to_owned()),
            ..Self::empty(MirrorKindV1::GithubRelease)
        }
    }

    /// A public content-addressed store, e.g. `https://cdn.zpkg.net`.
    pub fn object_store(url: &str) -> Self {
        Self {
            kind: MirrorKindV1::ObjectStore,
            url: Some(url.to_owned()),
            ..Self::empty(MirrorKindV1::ObjectStore)
        }
    }

    fn empty(kind: MirrorKindV1) -> Self {
        Self {
            kind,
            id: None,
            url: None,
            alternate_urls: Vec::new(),
            priority: None,
            serves: if kind == MirrorKindV1::GithubRaw {
                MirrorServesV1 {
                    artifacts: false,
                    metadata: true,
                    index: true,
                }
            } else {
                MirrorServesV1::default()
            },
            repository: None,
            tag_template: None,
            asset_prefix: None,
            index_tag: None,
            branch: None,
            artifact_template: None,
            version_template: None,
            index_template: None,
            path: None,
        }
    }

    /// Stable identity: the declared `id`, else `<kind>:<host-or-path>`.
    pub fn identifier(&self) -> String {
        if let Some(id) = self.id.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
            return id.to_owned();
        }
        let detail = match self.kind {
            MirrorKindV1::Directory => self.path.clone(),
            MirrorKindV1::ObjectStore | MirrorKindV1::ZedRegistry => self.url.clone(),
            MirrorKindV1::GithubRelease | MirrorKindV1::GithubRaw => self
                .repository
                .as_deref()
                .and_then(|value| parse_repo_ref(value).ok())
                .map(|repo| format!("{}/{}/{}", repo.host, repo.owner, repo.repo))
                .or_else(|| self.url.clone()),
        };
        match detail {
            Some(detail) => format!("{}:{}", self.kind.as_str(), host_or_value(&detail)),
            None => self.kind.as_str().to_owned(),
        }
    }

    /// Effective try order.
    pub fn effective_priority(&self) -> u32 {
        self.priority
            .unwrap_or_else(|| self.kind.default_priority())
    }

    /// Total ordering key. Priority first, then id, so merging mirror sets
    /// from the manifest, the registry, and local configuration yields the
    /// same order on every machine.
    pub fn order_key(&self) -> (u32, String) {
        (self.effective_priority(), self.identifier())
    }

    /// Fill in the defaults this descriptor inherits from its package, so a
    /// consumer never needs the publisher's manifest to interpret it.
    ///
    /// Applied at publish time: what lands in the registry and in a consumer's
    /// lockfile is always the fully-resolved form.
    pub fn with_package_defaults(mut self, repository: &str, tag_format: &str) -> Self {
        if matches!(
            self.kind,
            MirrorKindV1::GithubRelease | MirrorKindV1::GithubRaw
        ) {
            if self.repository.is_none() {
                self.repository = Some(repository.to_owned());
            }
            if self.kind == MirrorKindV1::GithubRelease && self.tag_template.is_none() {
                self.tag_template = Some(tag_format.to_owned());
            }
        }
        self
    }

    /// Every base URL for this mirror, primary first.
    pub fn base_urls(&self) -> Vec<String> {
        let mut out = Vec::new();
        if let Some(url) = self.url.as_deref() {
            out.push(trim_base(url));
        }
        for alternate in &self.alternate_urls {
            let trimmed = trim_base(alternate);
            if !out.contains(&trimmed) {
                out.push(trimmed);
            }
        }
        out
    }

    /// Candidate URLs for the artifact archive, in try order.
    pub fn artifact_urls(
        &self,
        coord: &MirrorCoordinateV1<'_>,
    ) -> Result<Vec<String>, MirrorError> {
        if !self.serves.artifacts {
            return Err(MirrorError::Unsupported {
                id: self.identifier(),
                what: "artifact bytes",
            });
        }
        match self.kind {
            MirrorKindV1::ZedRegistry => Ok(self
                .base_urls()
                .into_iter()
                .map(|base| format!("{base}/v1/artifacts/{}", coord.sha256))
                .collect()),
            MirrorKindV1::ObjectStore | MirrorKindV1::Directory => {
                let template = self
                    .artifact_template
                    .as_deref()
                    .unwrap_or(DEFAULT_ARTIFACT_TEMPLATE);
                let key = self.expand(template, "artifact_template", coord)?;
                Ok(self.roots()?.into_iter().map(|r| join(&r, &key)).collect())
            }
            MirrorKindV1::GithubRelease => {
                let asset = format!(
                    "{}{}.{}",
                    self.effective_asset_prefix(),
                    coord.sha256,
                    coord.format.extension()
                );
                self.release_asset_urls(&self.effective_tag(coord)?, &asset)
            }
            MirrorKindV1::GithubRaw => {
                let template = self
                    .artifact_template
                    .as_deref()
                    .unwrap_or(DEFAULT_ARTIFACT_TEMPLATE);
                let key = self.expand(template, "artifact_template", coord)?;
                self.raw_urls(&key)
            }
        }
    }

    /// Candidate URLs for this version's signed metadata document.
    pub fn version_metadata_urls(
        &self,
        coord: &MirrorCoordinateV1<'_>,
    ) -> Result<Vec<String>, MirrorError> {
        if !self.serves.metadata {
            return Err(MirrorError::Unsupported {
                id: self.identifier(),
                what: "version metadata",
            });
        }
        match self.kind {
            MirrorKindV1::ZedRegistry => Ok(self
                .base_urls()
                .into_iter()
                .map(|base| {
                    format!(
                        "{base}/v1/packages/{}/{}/versions/{}",
                        coord.org, coord.name, coord.version
                    )
                })
                .collect()),
            MirrorKindV1::ObjectStore | MirrorKindV1::Directory => {
                let template = self
                    .version_template
                    .as_deref()
                    .unwrap_or(DEFAULT_VERSION_TEMPLATE);
                let key = self.expand(template, "version_template", coord)?;
                Ok(self.roots()?.into_iter().map(|r| join(&r, &key)).collect())
            }
            MirrorKindV1::GithubRelease => {
                let asset = format!("{}version.json", self.effective_asset_prefix());
                self.release_asset_urls(&self.effective_tag(coord)?, &asset)
            }
            MirrorKindV1::GithubRaw => {
                let template = self
                    .version_template
                    .as_deref()
                    .unwrap_or(DEFAULT_VERSION_TEMPLATE);
                let key = self.expand(template, "version_template", coord)?;
                self.raw_urls(&key)
            }
        }
    }

    /// Candidate URLs for the package's signed version index.
    pub fn package_index_urls(&self, org: &str, name: &str) -> Result<Vec<String>, MirrorError> {
        if !self.serves.index {
            return Err(MirrorError::Unsupported {
                id: self.identifier(),
                what: "a package index",
            });
        }
        match self.kind {
            MirrorKindV1::ZedRegistry => Ok(self
                .base_urls()
                .into_iter()
                .map(|base| format!("{base}/v1/packages/{org}/{name}"))
                .collect()),
            MirrorKindV1::ObjectStore | MirrorKindV1::Directory => {
                let template = self
                    .index_template
                    .as_deref()
                    .unwrap_or(DEFAULT_INDEX_TEMPLATE);
                let key = expand_index(template, "index_template", &self.identifier(), org, name)?;
                Ok(self.roots()?.into_iter().map(|r| join(&r, &key)).collect())
            }
            MirrorKindV1::GithubRelease => {
                let tag = self.index_tag.as_deref().unwrap_or(DEFAULT_INDEX_TAG);
                let asset = format!("{}index-{org}-{name}.json", self.effective_asset_prefix());
                self.release_asset_urls(tag, &asset)
            }
            MirrorKindV1::GithubRaw => {
                let template = self
                    .index_template
                    .as_deref()
                    .unwrap_or(DEFAULT_INDEX_TEMPLATE);
                let key = expand_index(template, "index_template", &self.identifier(), org, name)?;
                self.raw_urls(&key)
            }
        }
    }

    /// Candidate URLs for the mirror-set bootstrap document.
    pub fn bootstrap_urls(&self) -> Vec<String> {
        match self.kind {
            MirrorKindV1::GithubRelease | MirrorKindV1::GithubRaw => self
                .raw_urls(&format!("{DEFAULT_RAW_PREFIX}/mirrors.json"))
                .unwrap_or_default(),
            _ => self
                .base_urls()
                .into_iter()
                .map(|base| format!("{base}{MIRROR_BOOTSTRAP_PATH}"))
                .collect(),
        }
    }

    /// The repository this mirror reads, parsed.
    pub fn repo_ref(&self) -> Result<RepoRefV1, MirrorError> {
        let raw = self
            .repository
            .as_deref()
            .ok_or_else(|| MirrorError::MissingField {
                id: self.identifier(),
                kind: self.kind.as_str(),
                field: "repository",
            })?;
        parse_repo_ref(raw)
    }

    fn effective_asset_prefix(&self) -> String {
        self.asset_prefix
            .as_deref()
            .unwrap_or(DEFAULT_ASSET_PREFIX)
            .to_owned()
    }

    fn effective_tag(&self, coord: &MirrorCoordinateV1<'_>) -> Result<String, MirrorError> {
        match self.tag_template.as_deref() {
            // A locked entry already carries the exact tag the artifact was
            // published from. Preferring it over re-rendering a template means
            // a package that changed its tag format keeps resolving.
            None if !coord.vcs_tag.is_empty() => Ok(coord.vcs_tag.to_owned()),
            None => Ok(format!("v{}", coord.version)),
            Some(template) => {
                let rendered = self.expand(template, "tag_template", coord)?;
                if rendered.is_empty() {
                    return Err(MirrorError::InvalidValue {
                        id: self.identifier(),
                        field: "tag_template",
                        value: template.to_owned(),
                    });
                }
                Ok(rendered)
            }
        }
    }

    /// Public release-asset download URLs. No API call and no token: GitHub
    /// (and Gitea/Forgejo/GHES) serve assets of a public release at this path,
    /// which is what makes a forge usable as a mirror during an API outage.
    fn release_asset_urls(&self, tag: &str, asset: &str) -> Result<Vec<String>, MirrorError> {
        let mut out = Vec::new();
        for base in self.base_urls() {
            out.push(format!("{base}/{}", encode_path(asset)));
        }
        let repo = self.repo_ref()?;
        out.push(format!(
            "https://{}/{}/{}/releases/download/{}/{}",
            repo.host,
            repo.owner,
            repo.repo,
            encode_path(tag),
            encode_path(asset)
        ));
        Ok(dedupe(out))
    }

    fn raw_urls(&self, key: &str) -> Result<Vec<String>, MirrorError> {
        let mut out: Vec<String> = self
            .base_urls()
            .into_iter()
            .map(|base| join(&base, key))
            .collect();
        if let Ok(repo) = self.repo_ref() {
            let branch = self.branch.as_deref().unwrap_or(DEFAULT_RAW_BRANCH);
            let root = if repo.host == GITHUB_HOST {
                format!(
                    "https://{GITHUB_RAW_HOST}/{}/{}/{}",
                    repo.owner,
                    repo.repo,
                    encode_path(branch)
                )
            } else {
                format!(
                    "https://{}/{}/{}/raw/{}",
                    repo.host,
                    repo.owner,
                    repo.repo,
                    encode_path(branch)
                )
            };
            out.push(join(&root, key));
        }
        if out.is_empty() {
            return Err(MirrorError::MissingField {
                id: self.identifier(),
                kind: self.kind.as_str(),
                field: "repository",
            });
        }
        Ok(dedupe(out))
    }

    fn roots(&self) -> Result<Vec<String>, MirrorError> {
        let roots = match self.kind {
            MirrorKindV1::Directory => {
                let path = self
                    .path
                    .as_deref()
                    .ok_or_else(|| MirrorError::MissingField {
                        id: self.identifier(),
                        kind: self.kind.as_str(),
                        field: "path",
                    })?;
                vec![format!("file://{}", trim_base(path))]
            }
            _ => self.base_urls(),
        };
        if roots.is_empty() {
            return Err(MirrorError::MissingField {
                id: self.identifier(),
                kind: self.kind.as_str(),
                field: "url",
            });
        }
        Ok(roots)
    }

    fn expand(
        &self,
        template: &str,
        field: &'static str,
        coord: &MirrorCoordinateV1<'_>,
    ) -> Result<String, MirrorError> {
        let id = self.identifier();
        expand_template(template, field, &id, |token| match token {
            "sha256" => Some(coord.sha256.to_owned()),
            "sha256_prefix2" => Some(coord.sha256.chars().take(2).collect()),
            "sha256_prefix4" => Some(coord.sha256.chars().take(4).collect()),
            "ext" => Some(coord.format.extension().to_owned()),
            "org" => Some(coord.org.to_owned()),
            "name" => Some(coord.name.to_owned()),
            "version" => Some(coord.version.to_owned()),
            "tag" => Some(coord.vcs_tag.to_owned()),
            _ => None,
        })
    }

    /// Reject a descriptor whose fields do not belong to its kind, or whose
    /// URLs and templates could escape the mirror root.
    pub fn validate(&self) -> Result<(), MirrorError> {
        let id = self.identifier();
        let kind = self.kind.as_str();
        if self.serves.serves_nothing() {
            return Err(MirrorError::ServesNothing { id });
        }

        let uses_repository = matches!(
            self.kind,
            MirrorKindV1::GithubRelease | MirrorKindV1::GithubRaw
        );
        let is_store = matches!(
            self.kind,
            MirrorKindV1::ObjectStore | MirrorKindV1::Directory | MirrorKindV1::GithubRaw
        );

        for (present, field, allowed) in [
            (self.repository.is_some(), "repository", uses_repository),
            (
                self.tag_template.is_some(),
                "tag_template",
                self.kind == MirrorKindV1::GithubRelease,
            ),
            (
                self.asset_prefix.is_some(),
                "asset_prefix",
                self.kind == MirrorKindV1::GithubRelease,
            ),
            (
                self.index_tag.is_some(),
                "index_tag",
                self.kind == MirrorKindV1::GithubRelease,
            ),
            (
                self.branch.is_some(),
                "branch",
                self.kind == MirrorKindV1::GithubRaw,
            ),
            (
                self.artifact_template.is_some(),
                "artifact_template",
                is_store,
            ),
            (
                self.version_template.is_some(),
                "version_template",
                is_store,
            ),
            (self.index_template.is_some(), "index_template", is_store),
            (
                self.path.is_some(),
                "path",
                self.kind == MirrorKindV1::Directory,
            ),
        ] {
            if present && !allowed {
                return Err(MirrorError::UnexpectedField {
                    id: id.clone(),
                    kind,
                    field,
                });
            }
        }

        match self.kind {
            MirrorKindV1::ObjectStore | MirrorKindV1::ZedRegistry => {
                if self.url.is_none() {
                    return Err(MirrorError::MissingField {
                        id: id.clone(),
                        kind,
                        field: "url",
                    });
                }
            }
            MirrorKindV1::Directory => {
                let path = self
                    .path
                    .as_deref()
                    .ok_or_else(|| MirrorError::MissingField {
                        id: id.clone(),
                        kind,
                        field: "path",
                    })?;
                if !path.starts_with('/') {
                    return Err(MirrorError::InvalidValue {
                        id: id.clone(),
                        field: "path",
                        value: path.to_owned(),
                    });
                }
                if self.url.is_some() {
                    return Err(MirrorError::UnexpectedField {
                        id: id.clone(),
                        kind,
                        field: "url",
                    });
                }
            }
            MirrorKindV1::GithubRelease | MirrorKindV1::GithubRaw => {
                if self.repository.is_none() && self.url.is_none() {
                    return Err(MirrorError::MissingField {
                        id: id.clone(),
                        kind,
                        field: "repository",
                    });
                }
                if let Some(repository) = self.repository.as_deref() {
                    parse_repo_ref(repository)?;
                }
            }
        }

        for url in self
            .url
            .iter()
            .map(String::as_str)
            .chain(self.alternate_urls.iter().map(String::as_str))
        {
            validate_base_url(&id, url)?;
        }

        for (template, field) in [
            (self.artifact_template.as_deref(), "artifact_template"),
            (self.version_template.as_deref(), "version_template"),
            (self.index_template.as_deref(), "index_template"),
        ] {
            if let Some(template) = template {
                validate_key_template(&id, field, template)?;
            }
        }
        if let Some(template) = self.tag_template.as_deref() {
            expand_template(template, "tag_template", &id, |token| {
                matches!(token, "version" | "org" | "name" | "sha256").then(|| "x".to_owned())
            })?;
        }
        if let Some(prefix) = self.asset_prefix.as_deref()
            && (prefix.contains('/') || prefix.contains("..") || prefix.len() > 64)
        {
            return Err(MirrorError::InvalidValue {
                id,
                field: "asset_prefix",
                value: prefix.to_owned(),
            });
        }
        Ok(())
    }
}

/// Validate and canonically order a package's declared mirror set.
///
/// Ordering happens here rather than at each call site so that the manifest,
/// the registry response, and the lockfile all agree on try order without
/// re-deriving it.
pub fn normalize_mirrors(
    mirrors: &[MirrorDescriptorV1],
) -> Result<Vec<MirrorDescriptorV1>, MirrorError> {
    if mirrors.len() > MAX_MIRRORS {
        return Err(MirrorError::TooMany(mirrors.len()));
    }
    let mut seen = std::collections::BTreeSet::new();
    for mirror in mirrors {
        mirror.validate()?;
        let id = mirror.identifier();
        if !seen.insert(id.clone()) {
            return Err(MirrorError::DuplicateId(id));
        }
    }
    let mut sorted = mirrors.to_vec();
    sorted.sort_by_key(MirrorDescriptorV1::order_key);
    Ok(sorted)
}

/// The document served at [`MIRROR_BOOTSTRAP_PATH`].
///
/// Deliberately small and unsigned-optional: its only job is to name hosts. A
/// hostile bootstrap can send a client to a mirror of its choosing, and that
/// mirror still cannot produce bytes matching the lockfile pin, nor metadata
/// carrying a publisher signature it does not have.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MirrorBootstrapV1 {
    pub schema: String,
    /// RFC 3339 timestamp this document was generated.
    pub generated_at: String,
    /// Canonical registry base URL, for clients that recover mid-outage.
    pub registry_url: String,
    /// Every mirror the operator publishes, in try order.
    pub mirrors: Vec<MirrorDescriptorV1>,
}

impl MirrorBootstrapV1 {
    pub const SCHEMA_V1: &'static str = MIRROR_BOOTSTRAP_SCHEMA_V1;

    pub fn validate(&self) -> Result<(), MirrorError> {
        if self.schema != Self::SCHEMA_V1 {
            return Err(MirrorError::InvalidValue {
                id: "bootstrap".to_owned(),
                field: "schema",
                value: self.schema.clone(),
            });
        }
        normalize_mirrors(&self.mirrors).map(|_| ())
    }
}

/// Parse `https://github.com/acme/http-kit`, `git@github.com:acme/http-kit.git`,
/// `ssh://git@github.com/acme/http-kit`, or a bare `github.com/acme/http-kit`.
pub fn parse_repo_ref(value: &str) -> Result<RepoRefV1, MirrorError> {
    let trimmed = value.trim();
    let unparsable = || MirrorError::UnparsableRepository {
        value: trimmed.to_owned(),
    };
    let rest = if let Some(rest) = trimmed.strip_prefix("https://") {
        rest.to_owned()
    } else if let Some(rest) = trimmed.strip_prefix("http://") {
        rest.to_owned()
    } else if let Some(rest) = trimmed.strip_prefix("ssh://") {
        rest.to_owned()
    } else if let Some(rest) = trimmed.strip_prefix("git://") {
        rest.to_owned()
    } else if let Some((head, tail)) = trimmed.split_once(':')
        && !tail.starts_with("//")
        && head.contains('@')
    {
        // scp-like: git@github.com:acme/http-kit.git
        let host = head.split_once('@').map(|(_, h)| h).unwrap_or(head);
        format!("{host}/{tail}")
    } else {
        trimmed.to_owned()
    };
    // Strip any remaining userinfo (`git@host/...`) before splitting.
    let rest = match rest.split_once('/') {
        Some((authority, tail)) => {
            let host = authority.rsplit('@').next().unwrap_or(authority);
            format!("{host}/{tail}")
        }
        None => rest,
    };
    let rest = rest.split(['?', '#']).next().unwrap_or(&rest).to_owned();
    let mut parts = rest.split('/').filter(|part| !part.is_empty());
    let host = parts.next().ok_or_else(unparsable)?.to_ascii_lowercase();
    let owner = parts.next().ok_or_else(unparsable)?.to_owned();
    let repo = parts
        .next()
        .ok_or_else(unparsable)?
        .trim_end_matches(".git")
        .to_owned();
    if parts.next().is_some() {
        return Err(unparsable());
    }
    let valid = |segment: &str| {
        !segment.is_empty()
            && segment.len() <= 128
            && segment != "."
            && segment != ".."
            && segment
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
    };
    if !valid(&owner) || !valid(&repo) || host.is_empty() || !host.contains('.') {
        return Err(unparsable());
    }
    Ok(RepoRefV1 { host, owner, repo })
}

fn validate_base_url(id: &str, url: &str) -> Result<(), MirrorError> {
    let invalid = |field: &'static str| MirrorError::InvalidValue {
        id: id.to_owned(),
        field,
        value: url.to_owned(),
    };
    let is_https = url.starts_with("https://");
    let is_file = url.starts_with("file://");
    // Plaintext is allowed only for loopback, so a development mirror works
    // without inviting an unencrypted metadata fetch from the public internet.
    let is_local_http = url.starts_with("http://127.0.0.1")
        || url.starts_with("http://localhost")
        || url.starts_with("http://[::1]");
    if !(is_https || is_file || is_local_http) {
        return Err(invalid("url"));
    }
    if url.contains('@')
        || url.contains('?')
        || url.contains('#')
        || url.contains("..")
        || url
            .bytes()
            .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
    {
        return Err(invalid("url"));
    }
    Ok(())
}

fn validate_key_template(id: &str, field: &'static str, template: &str) -> Result<(), MirrorError> {
    let rendered = expand_template(template, field, id, |token| {
        matches!(
            token,
            "sha256"
                | "sha256_prefix2"
                | "sha256_prefix4"
                | "ext"
                | "org"
                | "name"
                | "version"
                | "tag"
        )
        .then(|| "x".to_owned())
    })?;
    let unsafe_key = rendered.is_empty()
        || rendered.len() > 512
        || rendered.starts_with('/')
        || rendered.contains('\\')
        || rendered.contains("//")
        || rendered
            .split('/')
            .any(|segment| segment.is_empty() || matches!(segment, "." | ".."));
    if unsafe_key {
        return Err(MirrorError::InvalidValue {
            id: id.to_owned(),
            field,
            value: template.to_owned(),
        });
    }
    Ok(())
}

fn expand_index(
    template: &str,
    field: &'static str,
    id: &str,
    org: &str,
    name: &str,
) -> Result<String, MirrorError> {
    expand_template(template, field, id, |token| match token {
        "org" => Some(org.to_owned()),
        "name" => Some(name.to_owned()),
        _ => None,
    })
}

/// Substitute `{token}` placeholders, failing closed on any token the resolver
/// does not recognize. An unknown placeholder is a typo that would otherwise
/// become a permanent 404 discovered only during an outage.
fn expand_template<F>(
    template: &str,
    field: &'static str,
    id: &str,
    resolve: F,
) -> Result<String, MirrorError>
where
    F: Fn(&str) -> Option<String>,
{
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let end = after.find('}').ok_or_else(|| MirrorError::InvalidValue {
            id: id.to_owned(),
            field,
            value: template.to_owned(),
        })?;
        let token = &after[..end];
        let value = resolve(token).ok_or_else(|| MirrorError::UnknownPlaceholder {
            id: id.to_owned(),
            field,
            token: token.to_owned(),
        })?;
        out.push_str(&value);
        rest = &after[end + 1..];
    }
    if rest.contains('}') {
        return Err(MirrorError::InvalidValue {
            id: id.to_owned(),
            field,
            value: template.to_owned(),
        });
    }
    out.push_str(rest);
    Ok(out)
}

fn trim_base(url: &str) -> String {
    url.trim().trim_end_matches('/').to_owned()
}

fn join(base: &str, key: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        key.trim_start_matches('/')
    )
}

fn host_or_value(value: &str) -> String {
    value
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(value)
        .trim_end_matches('/')
        .to_owned()
}

fn dedupe(values: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(values.len());
    for value in values {
        if !out.contains(&value) {
            out.push(value);
        }
    }
    out
}

/// Percent-encode the few characters that would otherwise change a URL's
/// structure. Tags and asset names are already constrained upstream; this is
/// the belt to that suspenders.
fn encode_path(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'+' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}
