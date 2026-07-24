use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::vcs::Vcs;
use crate::version::{Requirement, VersionScheme};

/// The `.zpkg.toml` manifest at the root of every package repository.
/// TOML only — never YAML or JSON.
///
/// ```toml
/// [package]
/// org = "acme"
/// name = "http-kit"
/// version = "1.2.0"
/// description = "Tiny HTTP helpers"
/// license = "MIT"
///
/// [package.repository]
/// vcs = "git"
/// url = "https://github.com/acme/http-kit"
///
/// [dependencies]
/// "acme/logkit" = "^0.3"
///
/// [publish]
/// exclude = ["benches/**"]
/// smoke_test = "sh scripts/smoke.sh"
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Manifest {
    pub package: PackageSection,
    /// Dependencies keyed by `org/name`, valued by a semver requirement.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dependencies: BTreeMap<String, String>,
    /// Dependencies needed only while running this package's `[build]`
    /// command. Never linked into a consumer's zed_modules/.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub build_dependencies: BTreeMap<String, String>,
    #[serde(default)]
    pub publish: PublishSection,
    #[serde(default)]
    pub scripts: ScriptsSection,
    /// Executables this package exposes, keyed by command name, valued by a
    /// path relative to the package root. Consumers get them hoisted into
    /// `zed_modules/.bin/` and runnable via `zed run <name>`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bin: BTreeMap<String, String>,
    /// Optional post-extract build step (compiled extensions, codegen).
    /// Builds run in an isolated staging copy — never inside the immutable
    /// source store — and results are cached per (sha256, platform).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build: Option<BuildSection>,
    /// Monorepo workspace configuration; only meaningful in a workspace
    /// root manifest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<WorkspaceSection>,
    /// Consumer-side patches for dependencies (e.g. fixing a dependency's
    /// broken or missing build command without waiting on upstream).
    #[serde(default, skip_serializing_if = "OverridesSection::is_empty")]
    pub overrides: OverridesSection,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PackageSection {
    /// Namespace the package is published under. Lowercase slug.
    pub org: String,
    /// Package name, unique within the org. Lowercase slug.
    pub name: String,
    /// Version of this package, interpreted according to `version_scheme`
    /// (semver by default).
    pub version: String,
    /// How `version` (and published tags) should be interpreted. Semver by
    /// default; `calver` for calendar versions, `opaque` for arbitrary tags.
    #[serde(
        default,
        skip_serializing_if = "crate::version::VersionScheme::is_default"
    )]
    pub version_scheme: VersionScheme,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    pub repository: RepositorySection,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub keywords: Vec<String>,
}

/// Where the package's source of truth lives. Any Git or Mercurial host
/// works: GitHub, GitLab, Bitbucket, or a self-hosted server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RepositorySection {
    #[serde(default)]
    pub vcs: Vcs,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct PublishSection {
    /// Extra glob patterns to exclude on top of the built-in defaults.
    pub exclude: Vec<String>,
    /// Keep README files in the published artifact (stripped by default).
    pub include_readme: bool,
    /// Command run by `zed test-local` inside a throwaway consumer project
    /// that has this package installed the same way a real consumer would.
    pub smoke_test: Option<String>,
    /// VCS tag template that must exist and point at the published commit.
    /// `{version}` is substituted with `package.version`.
    pub tag_format: String,
}

impl Default for PublishSection {
    fn default() -> Self {
        Self {
            exclude: Vec::new(),
            include_readme: false,
            smoke_test: None,
            tag_format: "v{version}".to_string(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ScriptsSection {
    /// Test command run from the repository (not from published artifacts;
    /// tests are stripped at publish time).
    pub test: Option<String>,
}

/// A post-extract build step. `command` runs via `sh -c` inside a staging
/// copy of the package; `outputs` optionally narrows what is promoted into
/// the per-platform build cache (default: the whole staged tree).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BuildSection {
    /// Command executed after extraction, e.g. `make` or `cargo build --release`.
    pub command: String,
    /// Paths (relative to the package root) to keep from the staging build.
    /// Empty means keep everything.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<String>,
}

/// Workspace configuration for monorepos: member globs relative to the
/// workspace root, e.g. `["packages/*", "apps/*"]`. Dependencies that
/// resolve to a member are symlinked straight to the member's source
/// directory instead of going through the registry.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct WorkspaceSection {
    pub members: Vec<String>,
}

/// Consumer-side dependency patches, keyed by `org/name`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct OverridesSection {
    /// Replace or provide a dependency's `[build]` step.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub build: BTreeMap<String, BuildSection>,
}

impl OverridesSection {
    pub fn is_empty(&self) -> bool {
        self.build.is_empty()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("invalid org slug `{0}`: must match [a-z0-9][a-z0-9-]*[a-z0-9]")]
    InvalidOrg(String),
    #[error("invalid package name `{0}`: must match [a-z0-9][a-z0-9-]*[a-z0-9]")]
    InvalidName(String),
    #[error("invalid version `{0}`: {1}")]
    InvalidVersion(String, String),
    #[error("invalid dependency key `{0}`: expected `org/name`")]
    InvalidDependencyKey(String),
    #[error("invalid requirement `{1}` for dependency `{0}`: {2}")]
    InvalidDependencyReq(String, String, String),
    #[error("invalid repository url `{0}`: {1}")]
    InvalidRepositoryUrl(String, String),
    #[error("invalid bin entry `{0}`: {1}")]
    InvalidBin(String, String),
    #[error("invalid build section: {0}")]
    InvalidBuild(String),
    #[error("manifest toml error: {0}")]
    Toml(String),
}

/// True for the lowercase slugs zed-pkg accepts as org and package names.
pub fn is_slug(s: &str) -> bool {
    !s.is_empty()
        && !s.starts_with('-')
        && !s.ends_with('-')
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// True for a 64-character lowercase-hex sha256 digest. Registry responses
/// and lockfiles feed digests into filesystem paths, so anything else is
/// rejected before it can reach the disk layer.
pub fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64
        && s.chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

/// True when `path` is a relative path with no `..` components — the only
/// shape allowed for manifest-declared paths (bin targets, build outputs).
pub fn is_safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.starts_with('\\')
        && !path.contains('\0')
        // windows drive/UNC prefixes
        && !(path.len() >= 2 && path.as_bytes()[1] == b':')
        && !path
            .split(['/', '\\'])
            .any(|seg| seg == ".." || seg.is_empty())
}

fn is_allowed_repo_url(url: &str) -> bool {
    // The repo URL renders as a link in registry UIs and is shelled to VCS
    // tooling, so restrict it to the schemes those consumers expect.
    ["https://", "http://", "ssh://", "git://", "git+ssh://"]
        .iter()
        .any(|scheme| url.starts_with(scheme))
        // scp-like git syntax: git@github.com:org/repo.git
        || (url.contains('@') && url.contains(':') && !url.contains("://"))
}

impl Manifest {
    /// Parse and validate a `.zpkg.toml` document.
    pub fn parse(input: &str) -> Result<Self, ManifestError> {
        let manifest: Manifest =
            toml::from_str(input).map_err(|e| ManifestError::Toml(e.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    pub fn to_toml_string(&self) -> Result<String, ManifestError> {
        toml::to_string_pretty(self).map_err(|e| ManifestError::Toml(e.to_string()))
    }

    pub fn validate(&self) -> Result<(), ManifestError> {
        if !is_slug(&self.package.org) {
            return Err(ManifestError::InvalidOrg(self.package.org.clone()));
        }
        if !is_slug(&self.package.name) {
            return Err(ManifestError::InvalidName(self.package.name.clone()));
        }
        self.package
            .version_scheme
            .validate_version(&self.package.version)
            .map_err(|e| ManifestError::InvalidVersion(self.package.version.clone(), e))?;
        for (key, req) in &self.dependencies {
            let mut parts = key.splitn(2, '/');
            let (org, name) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
            if !is_slug(org) || !is_slug(name) {
                return Err(ManifestError::InvalidDependencyKey(key.clone()));
            }
            // A requirement is either a semver range or an exact (opaque) tag;
            // only an empty string is invalid.
            if req.trim().is_empty() {
                return Err(ManifestError::InvalidDependencyReq(
                    key.clone(),
                    req.clone(),
                    "requirement must not be empty".to_string(),
                ));
            }
            let _ = Requirement::parse(req);
        }
        Ok(())
    }

    /// `org/name`, the canonical package identifier.
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.package.org, self.package.name)
    }

    /// Parsed semver version. Only call after `validate()` has passed.
    pub fn version(&self) -> Result<semver::Version, ManifestError> {
        semver::Version::parse(&self.package.version)
            .map_err(|e| ManifestError::InvalidVersion(self.package.version.clone(), e.to_string()))
    }

    /// The VCS tag that must exist for this version, e.g. `v1.2.0`.
    pub fn vcs_tag(&self) -> String {
        self.publish
            .tag_format
            .replace("{version}", &self.package.version)
    }
}
