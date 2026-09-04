//! Public mirror descriptors shared by the CLI and the registry DTOs.
//!
//! A mirror is an anonymous transport for the same bytes the lockfile pins.
//! Credentials never belong here.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::artifact::ArtifactFormat;
use crate::source::{
    ArtifactLocator, ArtifactQuery, ArtifactSourceKind, DEFAULT_R2_PUBLIC_BASE,
    github_identity_for, github_release_asset_names, github_release_download_url, r2_object_keys,
};

pub const MAX_MIRRORS: usize = 16;
pub const MIRROR_BOOTSTRAP_PATH: &str = "/.well-known/zpkg-mirrors.json";
pub const DEFAULT_ASSET_PREFIX: &str = "zpkg-";
pub const DEFAULT_INDEX_TAG: &str = "zpkg-index";
pub const DEFAULT_RAW_BRANCH: &str = "zpkg-mirror";
pub const DEFAULT_RAW_PREFIX: &str = "zpkg/";
pub const GITHUB_HOST: &str = "github.com";

#[derive(Debug, thiserror::Error)]
pub enum MirrorError {
    #[error("{0}")]
    Message(String),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MirrorKindV1 {
    ZedRegistry,
    #[default]
    ObjectStore,
    Directory,
    GithubRelease,
    GithubRaw,
}

impl MirrorKindV1 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ZedRegistry => "zed-registry",
            Self::ObjectStore => "object-store",
            Self::Directory => "directory",
            Self::GithubRelease => "github-release",
            Self::GithubRaw => "github-raw",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MirrorServesV1 {
    #[serde(default = "default_true")]
    pub artifacts: bool,
    #[serde(default = "default_true")]
    pub metadata: bool,
    #[serde(default = "default_true")]
    pub index: bool,
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

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepoRefV1 {
    pub host: String,
    pub owner: String,
    pub repo: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MirrorDescriptorV1 {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(default)]
    pub kind: MirrorKindV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternate_urls: Vec<String>,
    #[serde(default)]
    pub serves: MirrorServesV1,
}

impl MirrorDescriptorV1 {
    pub fn object_store(url: &str) -> Self {
        Self {
            id: None,
            kind: MirrorKindV1::ObjectStore,
            url: Some(url.trim_end_matches('/').to_owned()),
            path: None,
            repository: None,
            branch: None,
            asset_prefix: None,
            priority: None,
            alternate_urls: Vec::new(),
            serves: MirrorServesV1::default(),
        }
    }

    pub fn from_locator(locator: &ArtifactLocator) -> Self {
        Self {
            id: None,
            kind: match locator.kind {
                ArtifactSourceKind::Registry => MirrorKindV1::ZedRegistry,
                ArtifactSourceKind::R2 => MirrorKindV1::ObjectStore,
                ArtifactSourceKind::GithubRelease => MirrorKindV1::GithubRelease,
                ArtifactSourceKind::GithubPackages | ArtifactSourceKind::GithubArchive => {
                    MirrorKindV1::GithubRaw
                }
            },
            url: Some(locator.url.clone()),
            path: None,
            repository: None,
            branch: None,
            asset_prefix: None,
            priority: None,
            alternate_urls: Vec::new(),
            serves: MirrorServesV1::default(),
        }
    }

    pub fn github_release_of(repo_url: &str) -> Self {
        Self {
            id: None,
            kind: MirrorKindV1::GithubRelease,
            url: Some(repo_url.trim_end_matches('/').to_owned()),
            path: None,
            repository: Some(repo_url.trim_end_matches('/').to_owned()),
            branch: None,
            asset_prefix: None,
            priority: None,
            alternate_urls: Vec::new(),
            serves: MirrorServesV1::default(),
        }
    }

    pub fn identifier(&self) -> String {
        self.id
            .clone()
            .filter(|value| !value.is_empty())
            .or_else(|| self.url.clone())
            .or_else(|| self.repository.clone())
            .or_else(|| self.path.clone())
            .unwrap_or_else(|| self.kind.as_str().to_owned())
    }

    pub fn order_key(&self) -> u32 {
        self.effective_priority()
    }

    pub fn effective_priority(&self) -> u32 {
        self.priority.unwrap_or(100)
    }

    pub fn base_urls(&self) -> Vec<String> {
        self.url
            .iter()
            .cloned()
            .chain(self.alternate_urls.iter().cloned())
            .map(|base| base.trim_end_matches('/').to_owned())
            .filter(|base| !base.is_empty())
            .collect()
    }

    pub fn repo_ref(&self) -> Result<RepoRefV1, MirrorError> {
        if let Some(repository) = self.repository.as_deref() {
            return parse_repo_ref(repository);
        }
        if let Some(url) = self.url.as_deref() {
            return parse_repo_ref(url);
        }
        Err(MirrorError::Message(
            "github mirror is missing repository".into(),
        ))
    }

    pub fn validate(&self) -> Result<(), MirrorError> {
        match self.kind {
            MirrorKindV1::Directory => {
                if self.path.as_deref().unwrap_or("").is_empty() {
                    return Err(MirrorError::Message("directory mirror needs a path".into()));
                }
            }
            MirrorKindV1::GithubRelease | MirrorKindV1::GithubRaw => {
                self.repo_ref()?;
            }
            _ => {
                if self.url.as_deref().unwrap_or("").is_empty() {
                    return Err(MirrorError::Message("mirror needs a url".into()));
                }
            }
        }
        Ok(())
    }

    pub fn bootstrap_urls(&self) -> Vec<String> {
        self.base_urls()
            .into_iter()
            .map(|base| format!("{base}{MIRROR_BOOTSTRAP_PATH}"))
            .collect()
    }

    pub fn package_index_urls(&self, org: &str, name: &str) -> Result<Vec<String>, MirrorError> {
        self.validate()?;
        let prefix = self.asset_prefix.as_deref().unwrap_or(DEFAULT_ASSET_PREFIX);
        Ok(match self.kind {
            MirrorKindV1::ZedRegistry | MirrorKindV1::ObjectStore => self
                .base_urls()
                .into_iter()
                .flat_map(|base| {
                    vec![
                        format!("{base}/v1/packages/{org}/{name}"),
                        format!("{base}/v1/packages/{org}/{name}/index.json"),
                    ]
                })
                .collect(),
            MirrorKindV1::GithubRelease => {
                let repo = self.repo_ref()?;
                let identity = crate::source::GithubIdentity {
                    owner: repo.owner,
                    repo: repo.repo,
                };
                vec![github_release_download_url(
                    &identity,
                    DEFAULT_INDEX_TAG,
                    &format!("{prefix}index-{org}-{name}.json"),
                )]
            }
            MirrorKindV1::GithubRaw => {
                let repo = self.repo_ref()?;
                let branch = self.branch.as_deref().unwrap_or(DEFAULT_RAW_BRANCH);
                let raw_prefix = if prefix == DEFAULT_ASSET_PREFIX {
                    DEFAULT_RAW_PREFIX
                } else {
                    prefix
                };
                vec![format!(
                    "https://{}/{}/{}/{branch}/{raw_prefix}index-{org}-{name}.json",
                    if repo.host == GITHUB_HOST {
                        "raw.githubusercontent.com"
                    } else {
                        repo.host.as_str()
                    },
                    repo.owner,
                    repo.repo
                )]
            }
            MirrorKindV1::Directory => Vec::new(),
        })
    }

    pub fn version_metadata_urls(
        &self,
        coord: &MirrorCoordinateV1<'_>,
    ) -> Result<Vec<String>, MirrorError> {
        self.validate()?;
        let prefix = self.asset_prefix.as_deref().unwrap_or(DEFAULT_ASSET_PREFIX);
        Ok(match self.kind {
            MirrorKindV1::ZedRegistry | MirrorKindV1::ObjectStore => self
                .base_urls()
                .into_iter()
                .map(|base| {
                    format!(
                        "{base}/v1/packages/{}/{}/versions/{}",
                        coord.org, coord.name, coord.version
                    )
                })
                .collect(),
            MirrorKindV1::GithubRelease => {
                let identity =
                    github_identity_for(coord.org, coord.name, self.repository.as_deref());
                crate::source::git_tags_for_version(coord.version)
                    .into_iter()
                    .map(|tag| {
                        github_release_download_url(
                            &identity,
                            &tag,
                            &format!("{prefix}version.json"),
                        )
                    })
                    .collect()
            }
            MirrorKindV1::GithubRaw => {
                let repo = self.repo_ref()?;
                let branch = self.branch.as_deref().unwrap_or(DEFAULT_RAW_BRANCH);
                vec![format!(
                    "https://raw.githubusercontent.com/{}/{}/{branch}/{prefix}{}-{}-{}.json",
                    repo.owner, repo.repo, coord.org, coord.name, coord.version
                )]
            }
            MirrorKindV1::Directory => Vec::new(),
        })
    }

    pub fn artifact_urls(
        &self,
        coord: &MirrorCoordinateV1<'_>,
    ) -> Result<Vec<String>, MirrorError> {
        self.validate()?;
        let mut urls = Vec::new();
        let bases = self.base_urls();
        let prefix = self.asset_prefix.as_deref().unwrap_or(DEFAULT_ASSET_PREFIX);
        match self.kind {
            MirrorKindV1::ObjectStore | MirrorKindV1::ZedRegistry => {
                let query = ArtifactQuery {
                    org: coord.org,
                    name: coord.name,
                    version: coord.version,
                    vcs_tag: if coord.vcs_tag.is_empty() {
                        coord.version
                    } else {
                        coord.vcs_tag
                    },
                    sha256: if coord.sha256.is_empty() {
                        None
                    } else {
                        Some(coord.sha256)
                    },
                    format: coord.format,
                    repo_url: self.repository.as_deref().or(self.url.as_deref()),
                    artifacts: None,
                    registry_base: None,
                    r2_public_base: bases.first().map(String::as_str),
                    r2_public_key: None,
                };
                for base in &bases {
                    for key in r2_object_keys(&query) {
                        urls.push(format!("{base}/{key}"));
                    }
                    if !coord.sha256.is_empty() {
                        urls.push(format!(
                            "{base}/artifacts/{}.{}",
                            coord.sha256,
                            coord.format.extension()
                        ));
                    }
                }
            }
            MirrorKindV1::GithubRelease | MirrorKindV1::GithubRaw => {
                let identity = github_identity_for(
                    coord.org,
                    coord.name,
                    self.repository.as_deref().or(self.url.as_deref()),
                );
                for tag in crate::source::git_tags_for_version(coord.version) {
                    urls.push(github_release_download_url(
                        &identity,
                        &tag,
                        &format!("{prefix}{}.{}", coord.sha256, coord.format.extension()),
                    ));
                    for asset in github_release_asset_names(
                        coord.org,
                        coord.name,
                        coord.version,
                        coord.format.extension(),
                    ) {
                        urls.push(github_release_download_url(&identity, &tag, &asset));
                    }
                }
            }
            MirrorKindV1::Directory => {}
        }
        Ok(urls)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirrorCoordinateV1<'a> {
    pub org: &'a str,
    pub name: &'a str,
    pub version: &'a str,
    pub sha256: &'a str,
    pub format: ArtifactFormat,
    pub vcs_tag: &'a str,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MirrorBootstrapV1 {
    #[serde(default)]
    pub generated_at: String,
    #[serde(default)]
    pub registry_url: String,
    #[serde(default)]
    pub mirrors: Vec<MirrorDescriptorV1>,
}

impl MirrorBootstrapV1 {
    pub fn validate(&self) -> Result<(), MirrorError> {
        if self.mirrors.len() > MAX_MIRRORS {
            return Err(MirrorError::Message(format!(
                "bootstrap lists {} mirrors, max is {MAX_MIRRORS}",
                self.mirrors.len()
            )));
        }
        for mirror in &self.mirrors {
            mirror.validate()?;
        }
        Ok(())
    }
}

pub fn default_public_mirrors(registry: &str) -> Vec<MirrorDescriptorV1> {
    let mut mirrors = vec![MirrorDescriptorV1::object_store(DEFAULT_R2_PUBLIC_BASE)];
    if !registry.is_empty() {
        let mut registry_mirror = MirrorDescriptorV1::object_store(registry);
        registry_mirror.kind = MirrorKindV1::ZedRegistry;
        registry_mirror.priority = Some(0);
        mirrors.insert(0, registry_mirror);
    }
    mirrors
}

pub fn parse_repo_ref(url: &str) -> Result<RepoRefV1, MirrorError> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err(MirrorError::Message("empty repository url".into()));
    }
    let normalized = trimmed.replace('\\', "/");
    let normalized = normalized.strip_prefix("git+").unwrap_or(&normalized);

    let (host, rest) = if let Some(rest) = normalized.strip_prefix("git@") {
        let (host, path) = rest
            .split_once(':')
            .ok_or_else(|| MirrorError::Message("scp-like git url needs host:path".into()))?;
        (host.to_string(), path.to_string())
    } else if let Some(rest) = normalized.strip_prefix("ssh://git@") {
        split_host_path(rest)?
    } else if let Some(rest) = normalized.strip_prefix("ssh://") {
        split_host_path(rest)?
    } else if let Some(rest) = normalized.strip_prefix("https://") {
        split_host_path(rest)?
    } else if let Some(rest) = normalized.strip_prefix("http://") {
        split_host_path(rest)?
    } else if let Some(rest) = normalized.strip_prefix("git://") {
        split_host_path(rest)?
    } else {
        return Err(MirrorError::Message(format!(
            "unrecognized repository url `{trimmed}`"
        )));
    };

    let rest = rest.trim_start_matches('/').trim_end_matches('/');
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let mut parts = rest.split('/');
    let owner = parts
        .next()
        .filter(|part| !part.is_empty())
        .ok_or_else(|| MirrorError::Message("repository url is missing owner".into()))?;
    let repo = parts
        .next()
        .filter(|part| !part.is_empty())
        .ok_or_else(|| MirrorError::Message("repository url is missing repo".into()))?;
    if parts.next().is_some() {
        return Err(MirrorError::Message(
            "repository url has extra path segments".into(),
        ));
    }
    if !valid_segment(owner) || !valid_segment(repo) {
        return Err(MirrorError::Message(
            "repository owner/repo contains illegal characters".into(),
        ));
    }
    Ok(RepoRefV1 {
        host,
        owner: owner.to_string(),
        repo: repo.to_string(),
    })
}

fn split_host_path(rest: &str) -> Result<(String, String), MirrorError> {
    let rest = rest.split_once('@').map(|(_, rest)| rest).unwrap_or(rest);
    let (host, path) = rest
        .split_once('/')
        .ok_or_else(|| MirrorError::Message("url is missing owner/repo".into()))?;
    let host = host.split(':').next().unwrap_or(host);
    if host.is_empty() {
        return Err(MirrorError::Message("url is missing host".into()));
    }
    Ok((host.to_string(), path.to_string()))
}

fn valid_segment(value: &str) -> bool {
    !value.is_empty()
        && !value.contains("..")
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
}
