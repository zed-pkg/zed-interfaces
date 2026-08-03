use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::artifact::ArtifactFormat;
use crate::nix::NixAdapterRecord;

type NixAdapterKey = (String, String, String, Option<String>, u8, String, String);

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
    /// Optional immutable provenance for completed Nix interoperability
    /// translations. This additive field keeps lockfile version 1 readable by
    /// current consumers while allowing newer writers to preserve evidence.
    #[serde(default, rename = "nix-adapter", skip_serializing_if = "Vec::is_empty")]
    pub nix_adapters: Vec<NixAdapterRecord>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LockedPackage {
    pub org: String,
    pub name: String,
    pub version: String,
    /// Canonical lowercase hex sha256 of the artifact archive; also its store address.
    pub sha256: String,
    /// Artifact size in bytes. Zero-byte package artifacts are invalid.
    pub size: u64,
    /// Explicit archive format. A missing value must never be inferred during
    /// a frozen install because the format is part of immutable artifact identity.
    pub format: ArtifactFormat,
    /// VCS tag the version was published from, e.g. `v1.2.0`.
    pub vcs_tag: String,
    /// Exact source revision associated with the published artifact. The
    /// optional Rust representation preserves API compatibility for builders,
    /// but parsing and serialization reject `None` so committed lockfiles are
    /// always complete.
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
    #[error("invalid locked package metadata for `{package}`: {reason}")]
    InvalidPackageMetadata { package: String, reason: String },
    #[error("duplicate locked package identity `{0}`")]
    DuplicatePackage(String),
    #[error("invalid Nix adapter provenance: {0}")]
    InvalidNixAdapter(String),
    #[error("duplicate Nix adapter provenance key `{0}`")]
    DuplicateNixAdapter(String),
}

impl Default for Lockfile {
    fn default() -> Self {
        Self {
            version: Self::CURRENT_VERSION,
            packages: Vec::new(),
            nix_adapters: Vec::new(),
        }
    }
}

impl Lockfile {
    pub const CURRENT_VERSION: u32 = 1;

    pub fn parse(input: &str) -> Result<Self, LockfileError> {
        let lockfile: Lockfile =
            toml::from_str(input).map_err(|error| LockfileError::Toml(error.to_string()))?;
        if lockfile.version > Self::CURRENT_VERSION {
            return Err(LockfileError::UnsupportedVersion(
                lockfile.version,
                Self::CURRENT_VERSION,
            ));
        }
        lockfile.validate_packages()?;
        lockfile.validate_nix_adapters()?;
        Ok(lockfile)
    }

    pub fn to_toml_string(&self) -> Result<String, LockfileError> {
        self.validate_packages()?;
        self.validate_nix_adapters()?;
        let mut normalized = self.clone();
        normalized.nix_adapters.sort_by_key(nix_adapter_key);
        toml::to_string_pretty(&normalized).map_err(|error| LockfileError::Toml(error.to_string()))
    }

    pub fn find(&self, org: &str, name: &str) -> Option<&LockedPackage> {
        self.packages
            .iter()
            .find(|package| package.org == org && package.name == name)
    }

    /// Insert or replace the entry for `org/name`, keeping entries sorted.
    pub fn upsert(&mut self, package: LockedPackage) {
        self.packages
            .retain(|existing| !(existing.org == package.org && existing.name == package.name));
        self.packages.push(package);
        self.packages
            .sort_by(|a, b| (&a.org, &a.name).cmp(&(&b.org, &b.name)));
    }

    /// Insert or replace one completed Nix translation. Identity includes
    /// package/target, direction, system, and selected output, so platform
    /// variants never overwrite each other.
    pub fn upsert_nix_adapter(&mut self, adapter: NixAdapterRecord) -> Result<(), LockfileError> {
        adapter
            .validate()
            .map_err(|error| LockfileError::InvalidNixAdapter(error.to_string()))?;
        let key = nix_adapter_key(&adapter);
        self.nix_adapters
            .retain(|existing| nix_adapter_key(existing) != key);
        self.nix_adapters.push(adapter);
        self.nix_adapters.sort_by_key(nix_adapter_key);
        Ok(())
    }

    fn validate_packages(&self) -> Result<(), LockfileError> {
        let mut seen = BTreeSet::new();
        for package in &self.packages {
            let label = package.full_name();
            if !seen.insert((package.org.clone(), package.name.clone())) {
                return Err(LockfileError::DuplicatePackage(label));
            }
            if package.org.trim().is_empty() {
                return invalid_package(&label, "org must not be empty");
            }
            if package.name.trim().is_empty() {
                return invalid_package(&label, "name must not be empty");
            }
            if package.version.trim().is_empty() {
                return invalid_package(&label, "version must not be empty");
            }
            if !is_canonical_sha256(&package.sha256) {
                return invalid_package(
                    &label,
                    "sha256 must be 64 lowercase hexadecimal characters",
                );
            }
            if package.sha256.bytes().all(|byte| byte == b'0') {
                return invalid_package(&label, "sha256 must not be the all-zero digest");
            }
            if package.size == 0 {
                return invalid_package(&label, "size must be greater than zero");
            }
            if package.vcs_tag.trim().is_empty() {
                return invalid_package(&label, "vcs_tag must not be empty");
            }
            if package
                .vcs_commit
                .as_deref()
                .is_none_or(|commit| commit.trim().is_empty())
            {
                return invalid_package(
                    &label,
                    "vcs_commit must be explicitly present and non-empty",
                );
            }
            if package.source.trim().is_empty() {
                return invalid_package(&label, "source must not be empty");
            }
        }
        Ok(())
    }

    fn validate_nix_adapters(&self) -> Result<(), LockfileError> {
        let mut seen = BTreeSet::new();
        for adapter in &self.nix_adapters {
            adapter
                .validate()
                .map_err(|error| LockfileError::InvalidNixAdapter(error.to_string()))?;
            let key = nix_adapter_key(adapter);
            if !seen.insert(key) {
                return Err(LockfileError::DuplicateNixAdapter(nix_adapter_label(
                    adapter,
                )));
            }
        }
        Ok(())
    }
}

impl LockedPackage {
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.org, self.name)
    }
}

fn invalid_package<T>(package: &str, reason: &str) -> Result<T, LockfileError> {
    Err(LockfileError::InvalidPackageMetadata {
        package: package.to_string(),
        reason: reason.to_string(),
    })
}

fn is_canonical_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn nix_adapter_key(adapter: &NixAdapterRecord) -> NixAdapterKey {
    match adapter {
        NixAdapterRecord::ZedToNix { package, .. } => (
            package.org.clone(),
            package.name.clone(),
            package.version.clone(),
            package.target.clone(),
            0,
            String::new(),
            String::new(),
        ),
        NixAdapterRecord::NixToZed {
            package, source, ..
        } => (
            package.org.clone(),
            package.name.clone(),
            package.version.clone(),
            package.target.clone(),
            1,
            source.realized.system.clone(),
            source.realized.output.clone(),
        ),
    }
}

fn nix_adapter_label(adapter: &NixAdapterRecord) -> String {
    let (direction, package, system, output) = match adapter {
        NixAdapterRecord::ZedToNix { package, .. } => ("zed-to-nix", package, "-", "-"),
        NixAdapterRecord::NixToZed {
            package, source, ..
        } => (
            "nix-to-zed",
            package,
            source.realized.system.as_str(),
            source.realized.output.as_str(),
        ),
    };
    format!(
        "{}/{}@{} target={} direction={direction} system={system} output={output}",
        package.org,
        package.name,
        package.version,
        package.target.as_deref().unwrap_or("-")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_LOCK: &str = r#"version = 1

[[package]]
org = "zed-pkg"
name = "fixture"
version = "1.2.3"
sha256 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
size = 42
format = "tar.gz"
vcs_tag = "v1.2.3"
vcs_commit = "0123456789abcdef0123456789abcdef01234567"
source = "file:///tmp/registry"
"#;

    #[test]
    fn complete_package_metadata_round_trips() {
        let lock = Lockfile::parse(VALID_LOCK).unwrap();
        let serialized = lock.to_toml_string().unwrap();
        assert_eq!(Lockfile::parse(&serialized).unwrap(), lock);
    }

    #[test]
    fn missing_artifact_format_is_not_inferred() {
        let input = VALID_LOCK.replace("format = \"tar.gz\"\n", "");
        let error = Lockfile::parse(&input).unwrap_err().to_string();
        assert!(error.contains("format"), "unexpected error: {error}");
    }

    #[test]
    fn missing_vcs_commit_is_rejected() {
        let input = VALID_LOCK.replace(
            "vcs_commit = \"0123456789abcdef0123456789abcdef01234567\"\n",
            "",
        );
        let error = Lockfile::parse(&input).unwrap_err().to_string();
        assert!(error.contains("vcs_commit"), "unexpected error: {error}");
    }

    #[test]
    fn malformed_zero_and_empty_artifact_metadata_are_rejected() {
        for (needle, replacement, expected) in [
            (
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "ABCDEF",
                "sha256",
            ),
            (
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "0000000000000000000000000000000000000000000000000000000000000000",
                "all-zero",
            ),
            ("size = 42", "size = 0", "size"),
            ("vcs_tag = \"v1.2.3\"", "vcs_tag = \"\"", "vcs_tag"),
            (
                "source = \"file:///tmp/registry\"",
                "source = \"\"",
                "source",
            ),
        ] {
            let input = VALID_LOCK.replacen(needle, replacement, 1);
            let error = Lockfile::parse(&input).unwrap_err().to_string();
            assert!(error.contains(expected), "unexpected error: {error}");
        }
    }

    #[test]
    fn duplicate_package_identities_are_rejected() {
        let package = VALID_LOCK.split_once("[[package]]").unwrap().1;
        let input = format!("{VALID_LOCK}\n[[package]]{package}");
        let error = Lockfile::parse(&input).unwrap_err().to_string();
        assert!(error.contains("duplicate locked package identity"));
    }

    #[test]
    fn writer_refuses_to_emit_incomplete_provenance() {
        let lock = Lockfile {
            version: Lockfile::CURRENT_VERSION,
            packages: vec![LockedPackage {
                org: "zed-pkg".to_string(),
                name: "fixture".to_string(),
                version: "1.2.3".to_string(),
                sha256:
                    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                        .to_string(),
                size: 42,
                format: ArtifactFormat::TarGz,
                vcs_tag: "v1.2.3".to_string(),
                vcs_commit: None,
                source: "file:///tmp/registry".to_string(),
            }],
            nix_adapters: Vec::new(),
        };
        let error = lock.to_toml_string().unwrap_err().to_string();
        assert!(error.contains("vcs_commit"));
    }
}
