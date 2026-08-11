//! Secure, self-describing binary artifact profile for Zed package archives.
//!
//! A binary artifact is always a ZIP rooted at `pkg/`. The package's ordinary
//! `.zpkg.toml` remains authoritative for package identity and `[bin]`
//! entrypoints, while `.zpkg-binary.json` binds the selected platform and the
//! digest, size, and executable intent of every payload file. The descriptor
//! deliberately excludes itself from `files` so it has no circular digest.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::manifest::{Manifest, is_sha256_hex, is_slug};

pub const BINARY_ARTIFACT_SCHEMA_V1: &str = "zpkg.binary-artifact/v1";
pub const BINARY_ARTIFACT_METADATA_SCHEMA_V1: &str = "zpkg.binary-artifact-metadata/v1";
pub const BINARY_ARTIFACT_LIST_SCHEMA_V1: &str = "zpkg.binary-artifact-list/v1";
pub const BINARY_ARTIFACT_PUBLISH_META_SCHEMA_V1: &str = "zpkg.binary-artifact-publish-meta/v1";
pub const BINARY_ARTIFACT_LOCK_SCHEMA_V1: &str = "zpkg.binary-artifact-lock/v1";
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

impl BinaryArchiveFormatV1 {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Zip => "zip",
        }
    }
}

impl fmt::Display for BinaryArchiveFormatV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
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
            validate_immutable_source_token("source.vcs_commit", commit)?;
        }
        Ok(())
    }
}

/// Digest-addressed evidence associated with an immutable binary archive.
///
/// The attachment digest identifies the evidence bytes. `subject_sha256`
/// prevents a valid signature, SBOM, or provenance statement for one archive
/// from being replayed as evidence for another archive. Cryptographic
/// verification of the attachment contents remains implementation behavior.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum BinaryArtifactAttachmentKindV1 {
    Signature,
    Attestation,
    Provenance,
    Sbom,
}

/// One immutable signature, attestation, provenance, or SBOM object.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BinaryArtifactAttachmentV1 {
    pub kind: BinaryArtifactAttachmentKindV1,
    pub media_type: String,
    /// SHA-256 of the exact attachment bytes.
    pub sha256: String,
    pub size: u64,
    /// SHA-256 of the exact binary ZIP bytes described by this attachment.
    pub subject_sha256: String,
    /// Absolute or registry-relative immutable download URL.
    pub download_url: String,
}

impl BinaryArtifactAttachmentV1 {
    fn validate(
        &self,
        field: &str,
        expected_subject_sha256: &str,
    ) -> Result<(), BinaryArtifactError> {
        validate_media_type(&format!("{field}.media_type"), &self.media_type)?;
        validate_digest(&format!("{field}.sha256"), &self.sha256)?;
        if self.size == 0 {
            return Err(BinaryArtifactError::InvalidRelationship {
                message: format!("{field}.size must be greater than zero"),
            });
        }
        validate_digest(&format!("{field}.subject_sha256"), &self.subject_sha256)?;
        if self.subject_sha256 != expected_subject_sha256 {
            return Err(BinaryArtifactError::InvalidRelationship {
                message: format!("{field}.subject_sha256 must equal the binary archive sha256"),
            });
        }
        validate_url_reference(&format!("{field}.download_url"), &self.download_url)
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

    /// Validate the descriptor together with its sibling `pkg/.zpkg.toml`.
    ///
    /// Descriptor-only validation cannot prove that the package identity and
    /// command map are authoritative: those values deliberately appear in
    /// both files so that a verifier can reject substitution of either one.
    /// Packers and download verifiers must call this after parsing the exact
    /// manifest bytes whose digest appears in `files`.
    pub fn validate_against_manifest(
        &self,
        manifest: &Manifest,
    ) -> Result<(), BinaryArtifactError> {
        self.validate()?;
        manifest
            .validate()
            .map_err(|error| BinaryArtifactError::InvalidRelationship {
                message: format!("sibling .zpkg.toml is invalid: {error}"),
            })?;

        if self.package.org != manifest.package.org
            || self.package.name != manifest.package.name
            || self.package.version != manifest.package.version
        {
            return Err(BinaryArtifactError::InvalidRelationship {
                message: "descriptor package identity must exactly match sibling .zpkg.toml"
                    .to_owned(),
            });
        }
        if self.entrypoints != manifest.bin {
            return Err(BinaryArtifactError::InvalidRelationship {
                message: "descriptor entrypoints must exactly match sibling .zpkg.toml [bin]"
                    .to_owned(),
            });
        }
        Ok(())
    }
}

/// Immutable registry metadata for one release + target + format artifact.
///
/// `(org, name, version)` is release identity. `platform.target` and `format`
/// complete artifact identity; the remaining platform fields are resolved
/// attributes and must not be encoded in SemVer build metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BinaryArtifactMetadataV1 {
    pub schema: String,
    pub org: String,
    pub name: String,
    pub version: String,
    pub platform: BinaryPlatformV1,
    #[serde(default)]
    pub format: BinaryArchiveFormatV1,
    /// SHA-256 and size of the exact deterministic ZIP bytes.
    pub sha256: String,
    pub size: u64,
    /// SHA-256 of the exact canonical `pkg/.zpkg-binary.json` bytes.
    pub descriptor_sha256: String,
    /// Absolute or registry-relative immutable download URL.
    pub download_url: String,
    /// Exact UTC publication second (`YYYY-MM-DDTHH:MM:SSZ`).
    pub published_at: String,
    #[serde(default)]
    pub yanked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<BinarySourceProvenanceV1>,
    /// Strictly sorted, digest-addressed evidence. Optional evidence is omitted,
    /// never serialized as JSON null in the canonical form.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<BinaryArtifactAttachmentV1>,
}

impl BinaryArtifactMetadataV1 {
    pub const SCHEMA_V1: &'static str = BINARY_ARTIFACT_METADATA_SCHEMA_V1;

    pub fn validate(&self) -> Result<(), BinaryArtifactError> {
        validate_schema(&self.schema, Self::SCHEMA_V1)?;
        self.package_identity().validate()?;
        self.platform.validate()?;
        validate_digest("sha256", &self.sha256)?;
        if self.size == 0 {
            return Err(BinaryArtifactError::InvalidRelationship {
                message: "binary archive size must be greater than zero".to_owned(),
            });
        }
        validate_digest("descriptor_sha256", &self.descriptor_sha256)?;
        validate_url_reference("download_url", &self.download_url)?;
        validate_utc_timestamp("published_at", &self.published_at)?;
        if let Some(source) = &self.source {
            source.validate()?;
        }
        validate_attachments(&self.attachments, &self.sha256)
    }

    #[must_use]
    pub fn package_identity(&self) -> BinaryPackageIdentityV1 {
        BinaryPackageIdentityV1 {
            org: self.org.clone(),
            name: self.name.clone(),
            version: self.version.clone(),
        }
    }

    /// Stable artifact identity used by uniqueness constraints and conditional
    /// object creation. It intentionally excludes mutable lifecycle fields.
    #[must_use]
    pub fn artifact_identity(&self) -> (&str, &str, &str, &str, BinaryArchiveFormatV1) {
        (
            &self.org,
            &self.name,
            &self.version,
            &self.platform.target,
            self.format,
        )
    }
}

/// Collection response for every target/format artifact of one release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BinaryArtifactListResponseV1 {
    pub schema: String,
    pub org: String,
    pub name: String,
    pub version: String,
    /// Strictly sorted by `(platform.target, format)` with no duplicates.
    pub artifacts: Vec<BinaryArtifactMetadataV1>,
}

impl BinaryArtifactListResponseV1 {
    pub const SCHEMA_V1: &'static str = BINARY_ARTIFACT_LIST_SCHEMA_V1;

    pub fn validate(&self) -> Result<(), BinaryArtifactError> {
        validate_schema(&self.schema, Self::SCHEMA_V1)?;
        BinaryPackageIdentityV1 {
            org: self.org.clone(),
            name: self.name.clone(),
            version: self.version.clone(),
        }
        .validate()?;

        let mut previous: Option<(&str, BinaryArchiveFormatV1)> = None;
        for (index, artifact) in self.artifacts.iter().enumerate() {
            artifact.validate()?;
            if artifact.org != self.org
                || artifact.name != self.name
                || artifact.version != self.version
            {
                return Err(BinaryArtifactError::InvalidRelationship {
                    message: format!(
                        "artifacts[{index}] package identity must match the collection release"
                    ),
                });
            }
            let key = (artifact.platform.target.as_str(), artifact.format);
            if previous.is_some_and(|prior| prior >= key) {
                return Err(BinaryArtifactError::NonCanonicalOrder {
                    field: "artifacts platform.target/format".to_owned(),
                });
            }
            previous = Some(key);
        }
        Ok(())
    }
}

/// JSON metadata submitted beside one binary ZIP multipart body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BinaryArtifactPublishMetaV1 {
    pub schema: String,
    /// The exact ordinary manifest shipped as `pkg/.zpkg.toml`.
    pub manifest: Manifest,
    pub platform: BinaryPlatformV1,
    #[serde(default)]
    pub format: BinaryArchiveFormatV1,
    pub sha256: String,
    pub size: u64,
    pub descriptor_sha256: String,
    pub vcs_tag: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vcs_commit: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<BinaryArtifactAttachmentV1>,
}

impl BinaryArtifactPublishMetaV1 {
    pub const SCHEMA_V1: &'static str = BINARY_ARTIFACT_PUBLISH_META_SCHEMA_V1;

    pub fn validate(&self) -> Result<(), BinaryArtifactError> {
        validate_schema(&self.schema, Self::SCHEMA_V1)?;
        self.manifest
            .validate()
            .map_err(|error| BinaryArtifactError::InvalidRelationship {
                message: format!("publish manifest is invalid: {error}"),
            })?;
        if self.manifest.bin.is_empty() {
            return Err(BinaryArtifactError::InvalidRelationship {
                message: "binary publication manifest requires at least one [bin] entry".to_owned(),
            });
        }
        self.platform.validate()?;
        validate_digest("sha256", &self.sha256)?;
        if self.size == 0 {
            return Err(BinaryArtifactError::InvalidRelationship {
                message: "binary archive size must be greater than zero".to_owned(),
            });
        }
        validate_digest("descriptor_sha256", &self.descriptor_sha256)?;
        validate_nonempty("vcs_tag", &self.vcs_tag)?;
        if let Some(commit) = &self.vcs_commit {
            validate_immutable_source_token("vcs_commit", commit)?;
        }
        validate_attachments(&self.attachments, &self.sha256)
    }

    /// True only when a repeated PUT selects the same immutable identity and
    /// all byte/evidence bindings already stored by the registry.
    pub fn is_idempotent_with(
        &self,
        existing: &BinaryArtifactMetadataV1,
    ) -> Result<bool, BinaryArtifactError> {
        self.validate()?;
        existing.validate()?;
        let published_source = BinarySourceProvenanceV1 {
            repository: self.manifest.package.repository.url.clone(),
            vcs_tag: self.vcs_tag.clone(),
            vcs_commit: self.vcs_commit.clone(),
        };
        Ok(self.manifest.package.org == existing.org
            && self.manifest.package.name == existing.name
            && self.manifest.package.version == existing.version
            && self.platform == existing.platform
            && self.format == existing.format
            && self.sha256 == existing.sha256
            && self.size == existing.size
            && self.descriptor_sha256 == existing.descriptor_sha256
            && self.attachments == existing.attachments
            && existing.source.as_ref() == Some(&published_source))
    }
}

/// Frozen provenance for one artifact-qualified binary resolution.
///
/// This is intentionally a standalone versioned record rather than an
/// unversioned optional field on `LockedPackage`: old lockfile-v1 readers
/// would otherwise deserialize and silently discard target/evidence fields on
/// rewrite. A future lockfile envelope may embed this record only with an
/// explicit version bump and fail-closed older-client behavior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BinaryArtifactLockV1 {
    pub schema: String,
    pub package: BinaryPackageIdentityV1,
    pub platform: BinaryPlatformV1,
    #[serde(default)]
    pub format: BinaryArchiveFormatV1,
    pub sha256: String,
    pub size: u64,
    pub descriptor_sha256: String,
    /// Canonical registry origin, not an expiring object-store URL.
    pub registry: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<BinarySourceProvenanceV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<BinaryArtifactAttachmentV1>,
}

impl BinaryArtifactLockV1 {
    pub const SCHEMA_V1: &'static str = BINARY_ARTIFACT_LOCK_SCHEMA_V1;

    pub fn validate(&self) -> Result<(), BinaryArtifactError> {
        validate_schema(&self.schema, Self::SCHEMA_V1)?;
        self.package.validate()?;
        self.platform.validate()?;
        validate_digest("sha256", &self.sha256)?;
        if self.size == 0 {
            return Err(BinaryArtifactError::InvalidRelationship {
                message: "locked binary archive size must be greater than zero".to_owned(),
            });
        }
        validate_digest("descriptor_sha256", &self.descriptor_sha256)?;
        validate_registry_origin("registry", &self.registry)?;
        if let Some(download_url) = &self.download_url {
            validate_url_reference("download_url", download_url)?;
        }
        if let Some(source) = &self.source {
            source.validate()?;
        }
        validate_attachments(&self.attachments, &self.sha256)
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

fn validate_schema(actual: &str, expected: &str) -> Result<(), BinaryArtifactError> {
    if actual == expected {
        Ok(())
    } else {
        Err(BinaryArtifactError::InvalidSchema {
            expected: expected.to_owned(),
            actual: actual.to_owned(),
        })
    }
}

fn validate_digest(field: &str, value: &str) -> Result<(), BinaryArtifactError> {
    if is_sha256_hex(value) {
        Ok(())
    } else {
        Err(BinaryArtifactError::InvalidDigest {
            field: field.to_owned(),
            value: value.to_owned(),
        })
    }
}

fn validate_immutable_source_token(field: &str, value: &str) -> Result<(), BinaryArtifactError> {
    let lower = value.to_ascii_lowercase();
    if !(7..=128).contains(&value.len())
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'+' | b':' | b'/')
        })
        || value.bytes().all(|byte| byte == b'0')
        || matches!(
            lower.as_str(),
            "head" | "main" | "master" | "trunk" | "latest"
        )
        || lower.starts_with("refs/heads/")
        || lower.starts_with("heads/")
    {
        return Err(BinaryArtifactError::InvalidValue {
            field: field.to_owned(),
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn validate_media_type(field: &str, value: &str) -> Result<(), BinaryArtifactError> {
    let mut parts = value.split('/');
    let valid_token = |token: &str| {
        !token.is_empty()
            && token.len() <= 127
            && token.bytes().all(|byte| {
                byte.is_ascii_alphanumeric()
                    || matches!(
                        byte,
                        b'!' | b'#' | b'$' | b'&' | b'^' | b'_' | b'.' | b'+' | b'-'
                    )
            })
    };
    if !matches!((parts.next(), parts.next(), parts.next()), (Some(left), Some(right), None) if valid_token(left) && valid_token(right))
    {
        return Err(BinaryArtifactError::InvalidValue {
            field: field.to_owned(),
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn validate_url_reference(field: &str, value: &str) -> Result<(), BinaryArtifactError> {
    let valid_prefix = value.starts_with("https://")
        || value.starts_with("http://")
        || (value.starts_with('/') && !value.starts_with("//"));
    if !valid_prefix
        || value.len() > 4096
        || value
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
    {
        return Err(BinaryArtifactError::InvalidValue {
            field: field.to_owned(),
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn validate_registry_origin(field: &str, value: &str) -> Result<(), BinaryArtifactError> {
    if !(value.starts_with("https://") || value.starts_with("http://"))
        || value.ends_with('/')
        || value.contains('?')
        || value.contains('#')
        || value
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
    {
        return Err(BinaryArtifactError::InvalidValue {
            field: field.to_owned(),
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn validate_attachments(
    attachments: &[BinaryArtifactAttachmentV1],
    expected_subject_sha256: &str,
) -> Result<(), BinaryArtifactError> {
    let mut previous: Option<(BinaryArtifactAttachmentKindV1, &str, &str)> = None;
    for (index, attachment) in attachments.iter().enumerate() {
        attachment.validate(&format!("attachments[{index}]"), expected_subject_sha256)?;
        let key = (
            attachment.kind,
            attachment.media_type.as_str(),
            attachment.sha256.as_str(),
        );
        if previous.is_some_and(|prior| prior >= key) {
            return Err(BinaryArtifactError::NonCanonicalOrder {
                field: "attachments kind/media_type/sha256".to_owned(),
            });
        }
        previous = Some(key);
    }
    Ok(())
}

fn validate_utc_timestamp(field: &str, value: &str) -> Result<(), BinaryArtifactError> {
    let bytes = value.as_bytes();
    let valid_shape = bytes.len() == 20
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'T'
        && bytes[13] == b':'
        && bytes[16] == b':'
        && bytes[19] == b'Z'
        && [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18]
            .into_iter()
            .all(|index| bytes[index].is_ascii_digit());
    if !valid_shape {
        return Err(BinaryArtifactError::InvalidValue {
            field: field.to_owned(),
            value: value.to_owned(),
        });
    }

    let year = parse_decimal(bytes, 0, 4);
    let month = parse_decimal(bytes, 5, 2);
    let day = parse_decimal(bytes, 8, 2);
    let hour = parse_decimal(bytes, 11, 2);
    let minute = parse_decimal(bytes, 14, 2);
    let second = parse_decimal(bytes, 17, 2);
    let valid_calendar = (1..=9999).contains(&year)
        && (1..=12).contains(&month)
        && day >= 1
        && day <= days_in_month(year, month)
        && hour <= 23
        && minute <= 59
        && second <= 59;
    if !valid_calendar {
        return Err(BinaryArtifactError::InvalidValue {
            field: field.to_owned(),
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn parse_decimal(bytes: &[u8], start: usize, length: usize) -> u32 {
    bytes[start..start + length]
        .iter()
        .fold(0_u32, |value, byte| value * 10 + u32::from(byte - b'0'))
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year.is_multiple_of(400) || (year.is_multiple_of(4) && !year.is_multiple_of(100)) => {
            29
        }
        2 => 28,
        _ => 0,
    }
}

fn validate_release_version(field: &str, value: &str) -> Result<(), BinaryArtifactError> {
    validate_nonempty(field, value)?;
    if value
        .bytes()
        .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
    {
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
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
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
        || path
            .bytes()
            .any(|byte| byte == 0 || byte.is_ascii_control())
        || path
            .split('/')
            .any(|part| part.is_empty() || matches!(part, "." | ".."))
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
        || !command
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'+'))
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
            entrypoints: BTreeMap::from([("zed-tool".to_owned(), "bin/zed-tool".to_owned())]),
            source: Some(BinarySourceProvenanceV1 {
                repository: "https://github.com/acme/zed-tool".to_owned(),
                vcs_tag: "v1.2.3".to_owned(),
                vcs_commit: Some(digest('c')),
            }),
        }
    }

    fn sibling_package_manifest() -> Manifest {
        Manifest::parse(
            r#"
[package]
org = "acme"
name = "zed-tool"
version = "1.2.3"

[package.repository]
vcs = "git"
url = "https://github.com/acme/zed-tool"

[bin]
zed-tool = "bin/zed-tool"
"#,
        )
        .expect("test package manifest is valid")
    }

    fn attachment(subject: &str) -> BinaryArtifactAttachmentV1 {
        BinaryArtifactAttachmentV1 {
            kind: BinaryArtifactAttachmentKindV1::Sbom,
            media_type: "application/spdx+json".to_owned(),
            sha256: digest('d'),
            size: 42,
            subject_sha256: subject.to_owned(),
            download_url: format!("/v1/artifacts/{}", digest('d')),
        }
    }

    fn metadata() -> BinaryArtifactMetadataV1 {
        BinaryArtifactMetadataV1 {
            schema: BINARY_ARTIFACT_METADATA_SCHEMA_V1.to_owned(),
            org: "acme".to_owned(),
            name: "zed-tool".to_owned(),
            version: "1.2.3".to_owned(),
            platform: BinaryPlatformV1 {
                target: "x86_64-unknown-linux-gnu".to_owned(),
                os: "linux".to_owned(),
                arch: "x86_64".to_owned(),
                libc: Some("gnu".to_owned()),
                abi: None,
            },
            format: BinaryArchiveFormatV1::Zip,
            sha256: digest('a'),
            size: 1024,
            descriptor_sha256: digest('b'),
            download_url: format!("/v1/artifacts/{}", digest('a')),
            published_at: "2026-08-11T16:00:00Z".to_owned(),
            yanked: false,
            source: Some(BinarySourceProvenanceV1 {
                repository: "https://github.com/acme/zed-tool".to_owned(),
                vcs_tag: "v1.2.3".to_owned(),
                vcs_commit: Some(digest('c')),
            }),
            attachments: vec![attachment(&digest('a'))],
        }
    }

    #[test]
    fn valid_binary_descriptor_passes() {
        valid_manifest().validate().unwrap();
    }

    #[test]
    fn descriptor_is_bound_to_sibling_package_manifest() {
        let descriptor = valid_manifest();
        let manifest = sibling_package_manifest();
        descriptor.validate_against_manifest(&manifest).unwrap();

        let mut wrong_identity = manifest.clone();
        wrong_identity.package.version = "1.2.4".to_owned();
        assert!(matches!(
            descriptor.validate_against_manifest(&wrong_identity),
            Err(BinaryArtifactError::InvalidRelationship { .. })
        ));

        let mut wrong_bins = manifest;
        wrong_bins
            .bin
            .insert("helper".to_owned(), "bin/helper".to_owned());
        assert!(matches!(
            descriptor.validate_against_manifest(&wrong_bins),
            Err(BinaryArtifactError::InvalidRelationship { .. })
        ));
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
        traversal
            .entrypoints
            .insert("zed-tool".to_owned(), "../zed-tool".to_owned());
        assert!(matches!(
            traversal.validate(),
            Err(BinaryArtifactError::InvalidValue { .. })
        ));

        let mut reserved = valid_manifest();
        reserved.files[1].path = BINARY_DESCRIPTOR_PATH.to_owned();
        reserved
            .entrypoints
            .insert("zed-tool".to_owned(), BINARY_DESCRIPTOR_PATH.to_owned());
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

    #[test]
    fn artifact_metadata_binds_evidence_to_exact_archive() {
        metadata().validate().unwrap();

        let mut replayed = metadata();
        replayed.attachments[0].subject_sha256 = digest('f');
        assert!(matches!(
            replayed.validate(),
            Err(BinaryArtifactError::InvalidRelationship { .. })
        ));

        let mut malformed_time = metadata();
        malformed_time.published_at = "2026-02-30T00:00:00Z".to_owned();
        assert!(matches!(
            malformed_time.validate(),
            Err(BinaryArtifactError::InvalidValue { .. })
        ));
    }

    #[test]
    fn collection_and_publication_enforce_immutable_artifact_identity() {
        let existing = metadata();
        let list = BinaryArtifactListResponseV1 {
            schema: BINARY_ARTIFACT_LIST_SCHEMA_V1.to_owned(),
            org: "acme".to_owned(),
            name: "zed-tool".to_owned(),
            version: "1.2.3".to_owned(),
            artifacts: vec![existing.clone()],
        };
        list.validate().unwrap();

        let mut duplicated = list;
        duplicated.artifacts.push(existing.clone());
        assert!(matches!(
            duplicated.validate(),
            Err(BinaryArtifactError::NonCanonicalOrder { .. })
        ));

        let publish = BinaryArtifactPublishMetaV1 {
            schema: BINARY_ARTIFACT_PUBLISH_META_SCHEMA_V1.to_owned(),
            manifest: sibling_package_manifest(),
            platform: existing.platform.clone(),
            format: existing.format,
            sha256: existing.sha256.clone(),
            size: existing.size,
            descriptor_sha256: existing.descriptor_sha256.clone(),
            vcs_tag: "v1.2.3".to_owned(),
            vcs_commit: Some(digest('c')),
            attachments: existing.attachments.clone(),
        };
        assert!(publish.is_idempotent_with(&existing).unwrap());

        let mut changed = publish;
        changed.descriptor_sha256 = digest('e');
        assert!(!changed.is_idempotent_with(&existing).unwrap());

        let locked = BinaryArtifactLockV1 {
            schema: BINARY_ARTIFACT_LOCK_SCHEMA_V1.to_owned(),
            package: existing.package_identity(),
            platform: existing.platform.clone(),
            format: existing.format,
            sha256: existing.sha256.clone(),
            size: existing.size,
            descriptor_sha256: existing.descriptor_sha256.clone(),
            registry: "https://registry.zpkg.net".to_owned(),
            download_url: Some(existing.download_url.clone()),
            source: existing.source.clone(),
            attachments: existing.attachments.clone(),
        };
        locked.validate().unwrap();
    }

    #[test]
    fn qualified_routes_include_target_and_format_as_path_segments() {
        assert_eq!(
            crate::registry::binary_artifacts_path("acme", "zed-tool", "1.2.3"),
            "/v1/packages/acme/zed-tool/versions/1.2.3/artifacts"
        );
        assert_eq!(
            crate::registry::binary_artifact_path(
                "acme",
                "zed-tool",
                "1.2.3",
                "x86_64-unknown-linux-gnu",
                BinaryArchiveFormatV1::Zip,
            ),
            "/v1/packages/acme/zed-tool/versions/1.2.3/artifacts/x86_64-unknown-linux-gnu/zip"
        );
    }
}
