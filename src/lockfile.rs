use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::artifact::ArtifactFormat;

/// The `.zpkg.lock` file written next to `.zpkg.toml` after resolution.
///
/// Serialized as TOML with one `[[package]]` table per locked package,
/// Cargo.lock-style. Every entry pins the exact artifact hash and the VCS
/// tag it was published from, so installs are reproducible and every
/// artifact is traceable back to source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Lockfile {
    pub version: u32,
    #[serde(default, rename = "package", skip_serializing_if = "Vec::is_empty")]
    pub packages: Vec<LockedPackage>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LockedPackage {
    pub org: String,
    pub name: String,
    pub version: String,
    /// Hex sha256 of the artifact archive; also its store address.
    pub sha256: String,
    /// Artifact size in bytes.
    pub size: u64,
    #[serde(default)]
    pub format: ArtifactFormat,
    /// VCS tag the version was published from, e.g. `v1.2.0`.
    pub vcs_tag: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vcs_commit: Option<String>,
    /// Base URL of the registry the artifact was resolved from.
    pub source: String,
}

#[derive(Debug, thiserror::Error)]
pub enum LockfileError {
    #[error("lockfile toml error: {0}")]
    Toml(String),
    #[error("unsupported lockfile version {0} (this build supports {1})")]
    UnsupportedVersion(u32, u32),
}

impl Default for Lockfile {
    fn default() -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            packages: Vec::new(),
        }
    }
}

impl Lockfile {
    pub const CURRENT_VERSION: u32 = 1;

    pub fn parse(input: &str) -> Result<Self, LockfileError> {
        let lockfile: Lockfile =
            toml::from_str(input).map_err(|e| LockfileError::Toml(e.to_string()))?;
        if lockfile.version > Self::CURRENT_VERSION {
            return Err(LockfileError::UnsupportedVersion(
                lockfile.version,
                Self::CURRENT_VERSION,
            ));
        }
        Ok(lockfile)
    }

    pub fn to_toml_string(&self) -> Result<String, LockfileError> {
        toml::to_string_pretty(self).map_err(|e| LockfileError::Toml(e.to_string()))
    }

    pub fn find(&self, org: &str, name: &str) -> Option<&LockedPackage> {
        self.packages
            .iter()
            .find(|p| p.org == org && p.name == name)
    }

    /// Insert or replace the entry for `org/name`, keeping entries sorted.
    pub fn upsert(&mut self, package: LockedPackage) {
        self.packages
            .retain(|p| !(p.org == package.org && p.name == package.name));
        self.packages.push(package);
        self.packages
            .sort_by(|a, b| (&a.org, &a.name).cmp(&(&b.org, &b.name)));
    }
}

impl LockedPackage {
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.org, self.name)
    }
}
