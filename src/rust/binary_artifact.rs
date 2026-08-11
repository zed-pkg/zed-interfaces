//! Secure, self-describing binary artifact profile for Zed package archives.
//!
//! A binary artifact is always a ZIP rooted at `pkg/`. The package's ordinary
//! `.zpkg.toml` remains authoritative for package identity and `[bin]`
//! entrypoints, while `.zpkg-binary.json` binds the selected platform and the
//! digest, size, and executable intent of every payload file. The descriptor
//! deliberately excludes itself from `files` so it has no circular digest.

use std::collections::{BTreeMap, BTreeSet};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::manifest::{is_sha256_hex, is_slug};

pub const BINARY_ARTIFACT_SCHEMA_V1: &str = "zpkg.binary-artifact/v1";
pub const BINARY_ARCHIVE_ROOT: &str = "pkg";
pub const BINARY_PACKAGE_MANIFEST_PATH: &str = ".zpkg.toml";
pub const BINARY_DESCRIPTOR_PATH: &str = ".zpkg-binary.json";
pub const BINARY_PACKAGE_MANIFEST_ARCHIVE_PATH: &str = "pkg/.zpkg.toml";
pub const BINARY_DESCRIPTOR_ARCHIVE_PATH: &str = "pkg/.zpkg-binary.json";

/// Binary artifacts use ZIP exclusively in v1. ZIP is portable to Windows,
/// preserves POSIX executable bits when present, and supports safe central-
/// directory inspection before extraction.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum BinaryArchiveFormatV1 {
    #[default]
    Zip,
}

/// Release identity stays independent from artifact/platform identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BinaryPackageIdentityV1 {
    pub org: String,
    pub name: String,
    pub version: String,
}

impl BinaryPackageIdentityV1 {
    fn validate(&self) -> Result<(), BinaryArtifactError> {
        if !is_slug(&self.org) {
            return Err(BinaryArtifactError::InvalidValue {
                field: "package.org".to_owned(),
                value: self.org.clone(),
            });
        }
        if !is_slug(&self.name) {
            return Err(BinaryArtifactError::InvalidValue {
                field: "package.name".to_owned(),
                value: self.name.clone(),
            });
        }
        validate_release_version("package.version", &self.version)
    }
}

/// Normalized native platform selected for this one artifact.
///
/// `target` is normally a Rust-style target triple such as
/// `x86_64-unknown-linux-gnu`, but Zed treats it as an opaque normalized token
/// so non-Rust toolchains can use their canonical spelling. `os`, `arch`,
/// `libc`, and `abi` remain structured for resolver filtering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BinaryPlatformV1 {
    pub target: String,
    pub os: String,
    pub arch: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub libc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abi: Option<String>,
}

impl BinaryPlatformV1 {
    pub fn validate(&self) -> Result<(), BinaryArtifactError> {
        validate_platform_token("platform.target", &self.target)?;
        validate_platform_token("platform.os", &self.os)?;
        validate_platform_token("platform.arch", &self.arch)?;
        if let Some(libc) = &self.libc {
            validate_platform_token("platform.libc", libc)?;
        }
        if let Some(abi) = &self.abi {
            validate_platform_token("platform.abi", abi)?;
        }
        Ok(())
    }
}

/// One regular payload file, addressed relative to `pkg/`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BinaryFileV1 {
    pub path: String,
    pub sha256: String,
    pub size: u64,
    /// Portable executable intent. Installers use this in addition to any ZIP
    /// Unix mode because Windows ZIP producers may not emit POSIX metadata.
    pub executable: bool,
}

impl BinaryFileV1 {
    fn validate(&self, field: &str) -> Result<(), BinaryArtifactError> {
        validate_safe_relative_path(&format!("{field}.path"), &self.path)?;
        if self.path == BINARY_DESCRIPTOR_PATH {
            return Err(BinaryArtifactError::ReservedPath {
                path: self.path.clone(),
            });
        }
        if !is_sha256_hex(&self.sha256) {
            return Err(BinaryArtifactError::InvalidDigest {
                field: format!("{field}.sha256"),
                value: self.sha256.clone(),
            });
        }
        Ok(())
    }
}

/// Optional source/VCS provenance carried inside the archive. The registry's
/// signed publication metadata remains the trust anchor; this is an inspectable
/// copy bound by the archive digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BinarySourceProvenanceV1 {
    pub repository: String,
    pub vcs_tag: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vcs_commit: Option<String>,
}

impl BinarySourceProvenanceV1 {
    fn validate(&self) -> Result<(), BinaryArtifactError> {
        validate_nonempty("source.repository", &self.repository)?;
        validate_nonempty("source.vcs_tag", &self.vcs_tag)?;
        if let Some(commit) = &self.vcs_commit {
            validate_nonempty("source.vcs_commit", commit)?;
            if commit.bytes().any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control()) {
                return Err(BinaryArtifactError::InvalidValue {
                    field: "source.vcs_commit".to_owned(),
                    value: commit.clone(),
                });
            }
        }
        Ok(())
    }
}

/// Canonical integrity descriptor stored at `pkg/.zpkg-binary.json`.
///
/// `files` contains every regular file except this descriptor itself. It must
/// include `.zpkg.toml`. `entrypoints` must exactly mirror the package
/// manifest's `[bin]` table and each referenced file must be marked executable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BinaryArtifactManifestV1 {
    pub schema: String,
    pub package: BinaryPackageIdentityV1,
    pub platform: BinaryPlatformV1,
    #[serde(default)]
    pub format: BinaryArchiveFormatV1,
    pub package_manifest: String,
    pub expanded_size: u64,
    pub files: Vec<BinaryFileV1>,
    pub entrypoints: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<BinarySourceProvenanceV1>,
}

impl BinaryArtifactManifestV1 {
    pub const SCHEMA_V1: &'static str = BINARY_ARTIFACT_SCHEMA_V1;

    pub fn validate(&self) -> Result<(), BinaryArtifactError> {
        if self.schema != Self::SCHEMA_V1 {
            return Err(BinaryArtifactError::InvalidSchema {
                expected: Self::SCHEMA_V1.to_owned(),
                actual: self.schema.clone(),
            });
        }
        self.package.validate()?;
        self.platform.validate()?;
        if self.package_manifest != BINARY_PACKAGE_MANIFEST_PATH {
            return Err(BinaryArtifactError::InvalidValue {
                field: "package_manifest".to_owned(),
                value: self.package_manifest.clone(),
            });
        }
        if self.files.is_empty() {
            return Err(BinaryArtifactError::MissingField {
                field: "files".to_owned(),
            });
        }
        if self.entrypoints.is_empty() {
            return Err(BinaryArtifactError::MissingField {
                field: "entrypoints".to_owned(),
            });
        }

        let mut previous: Option<&str> = None;
        let mut exact_paths = BTreeSet::new();
        let mut portable_paths = BTreeSet::new();
        let mut total_size = 0_u64;
        let mut manifest_seen = false;

        for (index, file) in self.files.iter().enumerate() {
            let field = format!("files[{index}]");
            file.validate(&field)?;
            if previous.is_some_and(|path| path >= file.path.as_str()) {
                return Err(BinaryArtifactError::NonCanonicalOrder {
                    field: "files.path".to_owned(),
                });
            }
            previous = Some(&file.path);
            if !exact_paths.insert(file.path.clone()) {
                return Err(BinaryArtifactError::DuplicatePath {
                    path: file.path.clone(),
                });
            }
            let portable = portable_path_key(&file.path);
            if !portable_paths.insert(portable) {
                return Err(BinaryArtifactError::PortablePathCollision {
                    path: file.path.clone(),
                });
            }
            if file.path == self.package_manifest {
                manifest_seen = true;
                if file.executable {
                    return Err(BinaryArtifactError::InvalidRelationship {
                        message: ".zpkg.toml must not be executable".to_owned(),
                    });
                }
            }
            total_size = total_size.checked_add(file.size).ok_or_else(|| {
                BinaryArtifactError::InvalidRelationship {
                    message: "binary artifact file sizes overflow u64".to_owned(),
                }
            })?;
        }

        if !manifest_seen {
            return Err(BinaryArtifactError::MissingPackageManifest);
        }
        if self.expanded_size != total_size {
            return Err(BinaryArtifactError::ExpandedSizeMismatch {
                declared: self.expanded_size,
                actual: total_size,
            });
        }

        let mut portable_commands = BTreeSet::new();
        for (command, path) in &self.entrypoints {
            validate_command(command)?;
            if !portable_commands.insert(command.to_ascii_lowercase()) {
                return Err(BinaryArtifactError::PortableCommandCollision {
                    command: command.clone(),
                });
            }
            validate_safe_relative_path("entrypoints path", path)?;
            let Some(file) = self.files.iter().find(|file| file.path == *path) else {
                return Err(BinaryArtifactError::MissingEntrypointFile {
                    command: command.clone(),
                    path: path.clone(),
                });
            };
            if !file.executable {
                return Err(BinaryArtifactError::EntrypointNotExecutable {
                    command: command.clone(),
                    path: path.clone(),
                });
            }
        }

        if let Some(source) = &self.source {
            source.validate()?;
        }
        Ok(())
    }

    /// Stable JSON bytes used for the in-archive descriptor and its external
    /// manifest digest. A trailing newline is deliberately excluded.
    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>, BinaryArtifactError> {
        self.validate()?;
        Ok(serde_json::to_vec(self)?)
    }
}

#[derive(Debug, Error)]
pub enum BinaryArtifactError {
    #[error("binary artifact schema must be `{expected}`, got `{actual}`")]
    InvalidSchema { expected: String, actual: String },
    #[error("field `{field}` is required")]
    MissingField { field: String },
    #[error("field `{field}` has invalid value `{value}`")]
    InvalidValue { field: String, value: String },
    #[error("field `{field}` has invalid sha256 `{value}`")]
    InvalidDigest { field: String, value: String },
    #[error("path `{path}` is reserved for generated binary metadata")]
    ReservedPath { path: String },
    #[error("duplicate archive path `{path}`")]
    DuplicatePath { path: String },
    #[error("archive path `{path}` collides under portable case/separator rules")]
    PortablePathCollision { path: String },
    #[error("entrypoint command `{command}` collides under portable case rules")]
    PortableCommandCollision { command: String },
    #[error("field `{field}` is not in canonical order")]
    NonCanonicalOrder { field: String },
    #[error("binary artifact is missing `.zpkg.toml`")]
    MissingPackageManifest,
    #[error("entrypoint `{command}` references missing file `{path}`")]
    MissingEntrypointFile { command: String, path: String },
    #[error("entrypoint `{command}` references non-executable file `{path}`")]
    EntrypointNotExecutable { command: String, path: String },
    #[error("expanded_size is {declared}, but payload files total {actual}")]
    ExpandedSizeMismatch { declared: u64, actual: u64 },
    #[error("invalid binary artifact relationship: {message}")]
    InvalidRelationship { message: String },
    #[error("JSON serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
}

fn validate_nonempty(field: &str, value: &str) -> Result<(), BinaryArtifactError> {
    if value.trim().is_empty() {
        Err(BinaryArtifactError::MissingField {
            field: field.to_owned(),
        })
    } else {
        Ok(())
    }
}

fn validate_release_version(field: &str, value: &str) -> Result<(), BinaryArtifactError> {
    validate_nonempty(field, value)?;
    if value.bytes().any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control()) {
        return Err(BinaryArtifactError::InvalidValue {
            field: field.to_owned(),
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn validate_platform_token(field: &str, value: &str) -> Result<(), BinaryArtifactError> {
    if value.is_empty()
        || value.len() > 128
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'-' | b'_' | b'.')
        })
        || !value
            .as_bytes()
            .first()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        || !value
            .as_bytes()
            .last()
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    {
        return Err(BinaryArtifactError::InvalidValue {
            field: field.to_owned(),
            value: value.to_owned(),
        });
    }
    Ok(())
}

pub fn validate_safe_relative_path(field: &str, path: &str) -> Result<(), BinaryArtifactError> {
    if path.is_empty()
        || path.len() > 4096
        || path.starts_with('/')
        || path.ends_with('/')
        || path.contains('\\')
        || path.contains(':')
        || path.bytes().any(|byte| byte == 0 || byte.is_ascii_control())
        || path.split('/').any(|part| part.is_empty() || matches!(part, "." | ".."))
    {
        return Err(BinaryArtifactError::InvalidValue {
            field: field.to_owned(),
            value: path.to_owned(),
        });
    }
    Ok(())
}

fn validate_command(command: &str) -> Result<(), BinaryArtifactError> {
    if command.is_empty()
        || command.len() > 128
        || command.starts_with('.')
        || command.contains('/')
        || command.contains('\\')
        || command.contains(':')
        || !command.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'+')
        })
    {
        return Err(BinaryArtifactError::InvalidValue {
            field: "entrypoints command".to_owned(),
            value: command.to_owned(),
        });
    }
    Ok(())
}

fn portable_path_key(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: char) -> String {
        std::iter::repeat_n(byte, 64).collect()
    }

    fn valid_manifest() -> BinaryArtifactManifestV1 {
        BinaryArtifactManifestV1 {
            schema: BINARY_ARTIFACT_SCHEMA_V1.to_owned(),
            package: BinaryPackageIdentityV1 {
                org: "acme".to_owned(),
                name: "zed-tool".to_owned(),
                version: "1.2.3".to_owned(),
            },
            platform: BinaryPlatformV1 {
                target: "x86_64-unknown-linux-gnu".to_owned(),
                os: "linux".to_owned(),
                arch: "x86_64".to_owned(),
                libc: Some("gnu".to_owned()),
                abi: None,
            },
            format: BinaryArchiveFormatV1::Zip,
            package_manifest: BINARY_PACKAGE_MANIFEST_PATH.to_owned(),
            expanded_size: 109,
            files: vec![
                BinaryFileV1 {
                    path: ".zpkg.toml".to_owned(),
                    sha256: digest('a'),
                    size: 9,
                    executable: false,
                },
                BinaryFileV1 {
                    path: "bin/zed-tool".to_owned(),
                    sha256: digest('b'),
                    size: 100,
                    executable: true,
                },
            ],
            entrypoints: BTreeMap::from([(
                "zed-tool".to_owned(),
                "bin/zed-tool".to_owned(),
            )]),
            source: Some(BinarySourceProvenanceV1 {
                repository: "https://github.com/acme/zed-tool".to_owned(),
                vcs_tag: "v1.2.3".to_owned(),
                vcs_commit: Some(digest('c')),
            }),
        }
    }

    #[test]
    fn valid_binary_descriptor_passes() {
        valid_manifest().validate().unwrap();
    }

    #[test]
    fn descriptor_requires_manifest_and_executable_entrypoint() {
        let mut manifest = valid_manifest();
        manifest.files.remove(0);
        manifest.expanded_size = 100;
        assert!(matches!(
            manifest.validate(),
            Err(BinaryArtifactError::MissingPackageManifest)
        ));

        let mut manifest = valid_manifest();
        manifest.files[1].executable = false;
        assert!(matches!(
            manifest.validate(),
            Err(BinaryArtifactError::EntrypointNotExecutable { .. })
        ));
    }

    #[test]
    fn descriptor_rejects_traversal_reserved_and_portable_collisions() {
        let mut traversal = valid_manifest();
        traversal.files[1].path = "../zed-tool".to_owned();
        traversal.entrypoints.insert("zed-tool".to_owned(), "../zed-tool".to_owned());
        assert!(matches!(
            traversal.validate(),
            Err(BinaryArtifactError::InvalidValue { .. })
        ));

        let mut reserved = valid_manifest();
        reserved.files[1].path = BINARY_DESCRIPTOR_PATH.to_owned();
        reserved.entrypoints.insert(
            "zed-tool".to_owned(),
            BINARY_DESCRIPTOR_PATH.to_owned(),
        );
        assert!(matches!(
            reserved.validate(),
            Err(BinaryArtifactError::ReservedPath { .. })
        ));

        let mut collision = valid_manifest();
        collision.files.push(BinaryFileV1 {
            path: "BIN/ZED-TOOL".to_owned(),
            sha256: digest('d'),
            size: 1,
            executable: true,
        });
        collision.expanded_size += 1;
        assert!(matches!(
            collision.validate(),
            Err(BinaryArtifactError::PortablePathCollision { .. })
                | Err(BinaryArtifactError::NonCanonicalOrder { .. })
        ));
    }

    #[test]
    fn canonical_json_is_stable_and_contains_platform_outside_version() {
        let manifest = valid_manifest();
        let first = manifest.canonical_json_bytes().unwrap();
        let second = manifest.canonical_json_bytes().unwrap();
        assert_eq!(first, second);
        let text = String::from_utf8(first).unwrap();
        assert!(text.contains("x86_64-unknown-linux-gnu"));
        assert!(text.contains("\"version\":\"1.2.3\""));
        assert!(!text.contains("1.2.3+linux"));
    }
}
