use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::vcs::Vcs;

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
    #[serde(default)]
    pub publish: PublishSection,
    #[serde(default)]
    pub scripts: ScriptsSection,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PackageSection {
    /// Namespace the package is published under. Lowercase slug.
    pub org: String,
    /// Package name, unique within the org. Lowercase slug.
    pub name: String,
    /// Semver version of this package.
    pub version: String,
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
        semver::Version::parse(&self.package.version).map_err(|e| {
            ManifestError::InvalidVersion(self.package.version.clone(), e.to_string())
        })?;
        for (key, req) in &self.dependencies {
            let mut parts = key.splitn(2, '/');
            let (org, name) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
            if !is_slug(org) || !is_slug(name) {
                return Err(ManifestError::InvalidDependencyKey(key.clone()));
            }
            semver::VersionReq::parse(req).map_err(|e| {
                ManifestError::InvalidDependencyReq(key.clone(), req.clone(), e.to_string())
            })?;
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
