//! Shared, service-independent contracts for distributing Zed packages as
//! OCI artifacts.
//!
//! The contract deliberately stops at immutable identity, descriptor, media
//! type, and provenance boundaries. Registry authentication, HTTP transport,
//! retries, and ORAS process execution belong to callers such as `zed-cli`.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Major-versioned identifier for the first finalized Zed ↔ OCI adapter
/// record. Unknown major versions must fail closed.
pub const OCI_ADAPTER_SCHEMA_V1: &str = "zed.oci-adapter/v1";

/// OCI image-manifest media type used by OCI 1.1 artifact manifests.
pub const OCI_IMAGE_MANIFEST_MEDIA_TYPE: &str = "application/vnd.oci.image.manifest.v1+json";
/// Zed package metadata stored as the OCI manifest's config descriptor.
pub const ZED_OCI_CONFIG_MEDIA_TYPE_V1: &str = "application/vnd.zed.package.config.v1+json";
/// A deterministic Zed `tar.gz` package artifact.
pub const ZED_OCI_PACKAGE_TAR_GZ_MEDIA_TYPE_V1: &str = "application/vnd.zed.package.v1.tar+gzip";
/// A deterministic Zed ZIP package artifact.
pub const ZED_OCI_PACKAGE_ZIP_MEDIA_TYPE_V1: &str = "application/vnd.zed.package.v1+zip";
/// The exact `.zpkg.toml` bytes associated with a package artifact.
pub const ZED_OCI_MANIFEST_MEDIA_TYPE_V1: &str = "application/vnd.zed.package.manifest.v1+toml";
/// The exact `.zpkg.lock` bytes associated with a package artifact.
pub const ZED_OCI_LOCK_MEDIA_TYPE_V1: &str = "application/vnd.zed.package.lock.v1+toml";
/// A platform-specific or portable executable/library payload.
pub const ZED_OCI_BINARY_MEDIA_TYPE_V1: &str = "application/vnd.zed.package.binary.v1";
/// SPDX JSON SBOM media type.
pub const SPDX_JSON_MEDIA_TYPE: &str = "application/spdx+json";
/// CycloneDX JSON SBOM media type.
pub const CYCLONEDX_JSON_MEDIA_TYPE: &str = "application/vnd.cyclonedx+json";
/// in-toto statement/provenance media type.
pub const IN_TOTO_JSON_MEDIA_TYPE: &str = "application/vnd.in-toto+json";

/// One canonical OCI content digest.
///
/// Contract v1 intentionally accepts only lowercase SHA-256 digests. OCI can
/// carry other digest algorithms, but Zed's existing artifact and lock model
/// is SHA-256 based; accepting an algorithm the rest of the system cannot
/// verify would create a false portability guarantee.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
pub struct OciDigest(String);

impl OciDigest {
    pub fn parse(value: impl Into<String>) -> Result<Self, OciInteropError> {
        let digest = Self(value.into());
        digest.validate()?;
        Ok(digest)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn encoded(&self) -> Option<&str> {
        self.0.strip_prefix("sha256:")
    }

    pub fn validate(&self) -> Result<(), OciInteropError> {
        let Some(encoded) = self.encoded() else {
            return Err(OciInteropError::InvalidDigest(
                "contract v1 requires a `sha256:<hex>` digest".to_string(),
            ));
        };
        if encoded.len() != 64
            || !encoded
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return Err(OciInteropError::InvalidDigest(
                "SHA-256 digest payload must be 64 lowercase hexadecimal characters".to_string(),
            ));
        }
        Ok(())
    }
}

impl fmt::Display for OciDigest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for OciDigest {
    type Err = OciInteropError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

/// A normalized OCI registry reference.
///
/// The textual form is `oci://registry/repository[:tag][@sha256:digest]`.
/// At least one tag or digest is required. A final adapter record additionally
/// requires the digest because a tag alone is mutable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OciReference {
    pub registry: String,
    pub repository: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<OciDigest>,
}

impl OciReference {
    pub fn parse(value: &str) -> Result<Self, OciInteropError> {
        if value.trim() != value || value.chars().any(char::is_whitespace) {
            return Err(OciInteropError::InvalidReference(
                "OCI reference must not contain leading, trailing, or embedded whitespace"
                    .to_string(),
            ));
        }
        let Some(rest) = value.strip_prefix("oci://") else {
            return Err(OciInteropError::InvalidReference(
                "OCI reference must start with `oci://`".to_string(),
            ));
        };
        if rest.contains(['?', '#']) {
            return Err(OciInteropError::InvalidReference(
                "OCI references must not contain query strings or fragments".to_string(),
            ));
        }

        let (name, digest) = match rest.rsplit_once('@') {
            Some((name, encoded)) => {
                if name.contains('@') {
                    return Err(OciInteropError::InvalidReference(
                        "OCI references must not contain embedded credentials or multiple `@` separators"
                            .to_string(),
                    ));
                }
                (name, Some(OciDigest::parse(encoded)?))
            }
            None => (rest, None),
        };

        let Some((registry, repository_and_tag)) = name.split_once('/') else {
            return Err(OciInteropError::InvalidReference(
                "OCI reference must include both a registry and repository path".to_string(),
            ));
        };
        let (repository, tag) = match repository_and_tag.rsplit_once(':') {
            Some((repository, tag)) => (repository, Some(tag.to_string())),
            None => (repository_and_tag, None),
        };

        let reference = Self {
            registry: registry.to_string(),
            repository: repository.to_string(),
            tag,
            digest,
        };
        reference.validate()?;
        Ok(reference)
    }

    pub fn is_immutable(&self) -> bool {
        self.digest.is_some()
    }

    pub fn require_digest(&self) -> Result<&OciDigest, OciInteropError> {
        self.digest.as_ref().ok_or_else(|| {
            OciInteropError::InvalidReference(
                "a finalized OCI reference requires an immutable digest".to_string(),
            )
        })
    }

    pub fn validate(&self) -> Result<(), OciInteropError> {
        validate_registry(&self.registry)?;
        validate_repository(&self.repository)?;
        if let Some(tag) = &self.tag {
            validate_tag(tag)?;
        }
        if let Some(digest) = &self.digest {
            digest.validate()?;
        }
        if self.tag.is_none() && self.digest.is_none() {
            return Err(OciInteropError::InvalidReference(
                "OCI reference must include a tag, a digest, or both".to_string(),
            ));
        }
        Ok(())
    }
}

impl fmt::Display for OciReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "oci://{}/{}", self.registry, self.repository)?;
        if let Some(tag) = &self.tag {
            write!(f, ":{tag}")?;
        }
        if let Some(digest) = &self.digest {
            write!(f, "@{digest}")?;
        }
        Ok(())
    }
}

impl FromStr for OciReference {
    type Err = OciInteropError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

/// Public Zed identity associated with an OCI artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OciPackageIdentity {
    pub org: String,
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

impl OciPackageIdentity {
    pub fn validate(&self) -> Result<(), OciInteropError> {
        if !is_slug(&self.org) {
            return Err(OciInteropError::InvalidPackageIdentity(format!(
                "invalid org slug `{}`",
                self.org
            )));
        }
        if !is_slug(&self.name) {
            return Err(OciInteropError::InvalidPackageIdentity(format!(
                "invalid package name `{}`",
                self.name
            )));
        }
        if self.version.is_empty()
            || self.version.trim() != self.version
            || self.version.chars().any(char::is_whitespace)
        {
            return Err(OciInteropError::InvalidPackageIdentity(
                "version must be non-empty and contain no whitespace".to_string(),
            ));
        }
        if let Some(target) = &self.target
            && !is_slug(target)
        {
            return Err(OciInteropError::InvalidPackageIdentity(format!(
                "invalid target `{target}`"
            )));
        }
        Ok(())
    }
}

/// OCI platform selector for multi-platform package payloads.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub struct OciPlatform {
    pub os: String,
    pub architecture: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

impl OciPlatform {
    pub fn validate(&self) -> Result<(), OciInteropError> {
        for (field, value) in [
            ("os", self.os.as_str()),
            ("architecture", self.architecture.as_str()),
        ] {
            if !is_platform_token(value) {
                return Err(OciInteropError::InvalidPlatform(format!(
                    "{field} `{value}` must be a lowercase platform token"
                )));
            }
        }
        if let Some(variant) = &self.variant
            && !is_platform_token(variant)
        {
            return Err(OciInteropError::InvalidPlatform(format!(
                "variant `{variant}` must be a lowercase platform token"
            )));
        }
        Ok(())
    }
}

/// One immutable OCI descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OciDescriptor {
    #[serde(rename = "mediaType")]
    pub media_type: String,
    pub digest: OciDigest,
    pub size: u64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, String>,
}

impl OciDescriptor {
    pub fn validate(&self, field: &str) -> Result<(), OciInteropError> {
        if !is_media_type(&self.media_type) {
            return Err(OciInteropError::InvalidDescriptor(format!(
                "{field} media type `{}` is invalid or non-canonical",
                self.media_type
            )));
        }
        self.digest.validate()?;
        if self.size == 0 {
            return Err(OciInteropError::InvalidDescriptor(format!(
                "{field} size must be greater than zero"
            )));
        }
        validate_annotations(&self.annotations, field)
    }
}

/// Semantic role of a layer within a Zed OCI artifact.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum OciLayerKind {
    PackageTarGz,
    PackageZip,
    Manifest,
    Lockfile,
    Binary,
    SpdxSbom,
    CycloneDxSbom,
    Provenance,
}

impl OciLayerKind {
    pub fn media_type(self) -> &'static str {
        match self {
            Self::PackageTarGz => ZED_OCI_PACKAGE_TAR_GZ_MEDIA_TYPE_V1,
            Self::PackageZip => ZED_OCI_PACKAGE_ZIP_MEDIA_TYPE_V1,
            Self::Manifest => ZED_OCI_MANIFEST_MEDIA_TYPE_V1,
            Self::Lockfile => ZED_OCI_LOCK_MEDIA_TYPE_V1,
            Self::Binary => ZED_OCI_BINARY_MEDIA_TYPE_V1,
            Self::SpdxSbom => SPDX_JSON_MEDIA_TYPE,
            Self::CycloneDxSbom => CYCLONEDX_JSON_MEDIA_TYPE,
            Self::Provenance => IN_TOTO_JSON_MEDIA_TYPE,
        }
    }

    pub fn is_primary_package(self) -> bool {
        matches!(self, Self::PackageTarGz | Self::PackageZip | Self::Binary)
    }

    fn allows_platform(self) -> bool {
        self.is_primary_package()
    }
}

/// One typed layer in the finalized artifact record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OciLayer {
    pub kind: OciLayerKind,
    pub descriptor: OciDescriptor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<OciPlatform>,
}

impl OciLayer {
    pub fn validate(&self) -> Result<(), OciInteropError> {
        self.descriptor.validate("OCI layer")?;
        if self.descriptor.media_type != self.kind.media_type() {
            return Err(OciInteropError::InvalidLayer(format!(
                "layer kind `{:?}` requires media type `{}`, found `{}`",
                self.kind,
                self.kind.media_type(),
                self.descriptor.media_type
            )));
        }
        if let Some(platform) = &self.platform {
            if !self.kind.allows_platform() {
                return Err(OciInteropError::InvalidLayer(format!(
                    "layer kind `{:?}` cannot carry a platform selector",
                    self.kind
                )));
            }
            platform.validate()?;
        }
        Ok(())
    }
}

/// Final immutable provenance record connecting a Zed package identity to one
/// OCI manifest digest and all of its typed blobs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OciAdapterRecord {
    pub schema: String,
    pub package: OciPackageIdentity,
    pub reference: OciReference,
    pub manifest: OciDescriptor,
    pub config: OciDescriptor,
    pub layers: Vec<OciLayer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<OciDescriptor>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, String>,
}

impl OciAdapterRecord {
    pub fn new(
        package: OciPackageIdentity,
        reference: OciReference,
        manifest: OciDescriptor,
        config: OciDescriptor,
        layers: Vec<OciLayer>,
    ) -> Self {
        Self {
            schema: OCI_ADAPTER_SCHEMA_V1.to_string(),
            package,
            reference,
            manifest,
            config,
            layers,
            subject: None,
            annotations: BTreeMap::new(),
        }
    }

    pub fn validate(&self) -> Result<(), OciInteropError> {
        if self.schema != OCI_ADAPTER_SCHEMA_V1 {
            return Err(OciInteropError::UnsupportedSchema(self.schema.clone()));
        }
        self.package.validate()?;
        self.reference.validate()?;
        let reference_digest = self.reference.require_digest()?;

        self.manifest.validate("OCI manifest")?;
        if self.manifest.media_type != OCI_IMAGE_MANIFEST_MEDIA_TYPE {
            return Err(OciInteropError::InvalidDescriptor(format!(
                "OCI manifest requires media type `{OCI_IMAGE_MANIFEST_MEDIA_TYPE}`"
            )));
        }
        if reference_digest != &self.manifest.digest {
            return Err(OciInteropError::InvalidAdapter(
                "reference digest must equal the finalized OCI manifest digest".to_string(),
            ));
        }

        self.config.validate("OCI config")?;
        if self.config.media_type != ZED_OCI_CONFIG_MEDIA_TYPE_V1 {
            return Err(OciInteropError::InvalidDescriptor(format!(
                "OCI config requires media type `{ZED_OCI_CONFIG_MEDIA_TYPE_V1}`"
            )));
        }
        if let Some(subject) = &self.subject {
            subject.validate("OCI subject")?;
        }
        validate_annotations(&self.annotations, "OCI adapter record")?;

        if self.layers.is_empty() {
            return Err(OciInteropError::InvalidAdapter(
                "a finalized OCI artifact requires at least one layer".to_string(),
            ));
        }

        let mut digests = BTreeSet::new();
        let mut positions = BTreeSet::new();
        let mut has_primary_package = false;
        for layer in &self.layers {
            layer.validate()?;
            has_primary_package |= layer.kind.is_primary_package();
            if !digests.insert(layer.descriptor.digest.as_str()) {
                return Err(OciInteropError::InvalidAdapter(format!(
                    "layer digest `{}` appears more than once",
                    layer.descriptor.digest
                )));
            }
            if !positions.insert((layer.kind, layer.platform.as_ref())) {
                return Err(OciInteropError::InvalidAdapter(format!(
                    "layer kind `{:?}` has duplicate platform coverage",
                    layer.kind
                )));
            }
        }
        if !has_primary_package {
            return Err(OciInteropError::InvalidAdapter(
                "a finalized OCI artifact requires a package archive or binary layer".to_string(),
            ));
        }
        Ok(())
    }

    /// Deterministic compact JSON bytes for hashing, signing, lock provenance,
    /// and OCI referrer attachment.
    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>, OciInteropError> {
        self.validate()?;
        let mut normalized = self.clone();
        normalized.layers.sort_by(|left, right| {
            (
                left.kind,
                left.platform.as_ref(),
                left.descriptor.digest.as_str(),
            )
                .cmp(&(
                    right.kind,
                    right.platform.as_ref(),
                    right.descriptor.digest.as_str(),
                ))
        });
        let value = serde_json::to_value(normalized)
            .map_err(|error| OciInteropError::Json(error.to_string()))?;
        serde_json::to_vec(&canonicalize_json(value))
            .map_err(|error| OciInteropError::Json(error.to_string()))
    }

    pub fn canonical_json_string(&self) -> Result<String, OciInteropError> {
        String::from_utf8(self.canonical_json_bytes()?)
            .map_err(|error| OciInteropError::Json(error.to_string()))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OciInteropError {
    #[error("unsupported OCI adapter schema `{0}`")]
    UnsupportedSchema(String),
    #[error("invalid OCI reference: {0}")]
    InvalidReference(String),
    #[error("invalid OCI digest: {0}")]
    InvalidDigest(String),
    #[error("invalid OCI package identity: {0}")]
    InvalidPackageIdentity(String),
    #[error("invalid OCI platform: {0}")]
    InvalidPlatform(String),
    #[error("invalid OCI descriptor: {0}")]
    InvalidDescriptor(String),
    #[error("invalid OCI layer: {0}")]
    InvalidLayer(String),
    #[error("invalid OCI adapter record: {0}")]
    InvalidAdapter(String),
    #[error("OCI adapter JSON error: {0}")]
    Json(String),
}

fn validate_registry(registry: &str) -> Result<(), OciInteropError> {
    if registry.is_empty()
        || registry.len() > 253
        || registry != registry.to_ascii_lowercase()
        || registry.contains(['/', '@', '[', ']'])
        || registry.chars().any(char::is_whitespace)
    {
        return Err(OciInteropError::InvalidReference(format!(
            "registry `{registry}` must be a lowercase DNS name or IPv4 address with an optional port"
        )));
    }

    let colon_count = registry.bytes().filter(|byte| *byte == b':').count();
    if colon_count > 1 {
        return Err(OciInteropError::InvalidReference(
            "contract v1 does not accept bracketed or ambiguous IPv6 registry literals".to_string(),
        ));
    }
    let (host, port) = match registry.split_once(':') {
        Some((host, port)) => (host, Some(port)),
        None => (registry, None),
    };
    if host.is_empty()
        || host.split('.').any(|label| {
            label.is_empty()
                || label.len() > 63
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
                || !label
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_alphanumeric)
                || !label
                    .as_bytes()
                    .last()
                    .is_some_and(u8::is_ascii_alphanumeric)
        })
    {
        return Err(OciInteropError::InvalidReference(format!(
            "registry host `{host}` is invalid"
        )));
    }
    if let Some(port) = port
        && (port.is_empty()
            || !port.bytes().all(|byte| byte.is_ascii_digit())
            || port
                .parse::<u16>()
                .ok()
                .filter(|value| *value > 0)
                .is_none())
    {
        return Err(OciInteropError::InvalidReference(format!(
            "registry port `{port}` must be an integer from 1 through 65535"
        )));
    }
    Ok(())
}

fn validate_repository(repository: &str) -> Result<(), OciInteropError> {
    if repository.is_empty()
        || repository.len() > 255
        || repository != repository.to_ascii_lowercase()
        || repository.split('/').any(|component| {
            component.is_empty()
                || !component.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'.' | b'_' | b'-')
                })
                || !component
                    .as_bytes()
                    .first()
                    .is_some_and(u8::is_ascii_alphanumeric)
                || !component
                    .as_bytes()
                    .last()
                    .is_some_and(u8::is_ascii_alphanumeric)
        })
    {
        return Err(OciInteropError::InvalidReference(format!(
            "repository `{repository}` must be a lowercase slash-separated OCI repository name"
        )));
    }
    Ok(())
}

fn validate_tag(tag: &str) -> Result<(), OciInteropError> {
    let Some(first) = tag.as_bytes().first() else {
        return Err(OciInteropError::InvalidReference(
            "OCI tag must not be empty".to_string(),
        ));
    };
    if tag.len() > 128
        || !first.is_ascii_alphanumeric() && *first != b'_'
        || !tag
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
    {
        return Err(OciInteropError::InvalidReference(format!(
            "OCI tag `{tag}` does not match `[A-Za-z0-9_][A-Za-z0-9_.-]{{0,127}}`"
        )));
    }
    Ok(())
}

fn validate_annotations(
    annotations: &BTreeMap<String, String>,
    field: &str,
) -> Result<(), OciInteropError> {
    for (key, value) in annotations {
        if key.is_empty()
            || key.len() > 255
            || !key.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'/' | b'_' | b'-')
            })
        {
            return Err(OciInteropError::InvalidDescriptor(format!(
                "{field} annotation key `{key}` is invalid"
            )));
        }
        if value.len() > 4096 || value.chars().any(char::is_control) {
            return Err(OciInteropError::InvalidDescriptor(format!(
                "{field} annotation `{key}` contains control characters or exceeds 4096 bytes"
            )));
        }
    }
    Ok(())
}

fn is_media_type(value: &str) -> bool {
    if value.is_empty() || value != value.to_ascii_lowercase() || value.contains(';') {
        return false;
    }
    let Some((kind, subtype)) = value.split_once('/') else {
        return false;
    };
    !kind.is_empty()
        && !subtype.is_empty()
        && kind.bytes().chain(subtype.bytes()).all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(
                    byte,
                    b'!' | b'#' | b'$' | b'&' | b'^' | b'_' | b'.' | b'+' | b'-'
                )
        })
}

fn is_slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && value
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn is_platform_token(value: &str) -> bool {
    !value.is_empty()
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && value
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphanumeric)
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'.' | b'-')
        })
}

fn canonicalize_json(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<_> = map.into_iter().collect();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            let mut sorted = serde_json::Map::new();
            for (key, value) in entries {
                sorted.insert(key, canonicalize_json(value));
            }
            Value::Object(sorted)
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize_json).collect()),
        scalar => scalar,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST_A: &str =
        "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const DIGEST_B: &str =
        "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const DIGEST_C: &str =
        "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    const DIGEST_D: &str =
        "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";

    fn descriptor(media_type: &str, digest: &str, size: u64) -> OciDescriptor {
        OciDescriptor {
            media_type: media_type.to_string(),
            digest: OciDigest::parse(digest).unwrap(),
            size,
            annotations: BTreeMap::new(),
        }
    }

    fn package() -> OciPackageIdentity {
        OciPackageIdentity {
            org: "acme".to_string(),
            name: "tool".to_string(),
            version: "1.2.3".to_string(),
            target: Some("rust".to_string()),
        }
    }

    fn record(layers: Vec<OciLayer>) -> OciAdapterRecord {
        OciAdapterRecord::new(
            package(),
            OciReference::parse(&format!("oci://ghcr.io/acme/tool:1.2.3@{DIGEST_A}")).unwrap(),
            descriptor(OCI_IMAGE_MANIFEST_MEDIA_TYPE, DIGEST_A, 512),
            descriptor(ZED_OCI_CONFIG_MEDIA_TYPE_V1, DIGEST_B, 128),
            layers,
        )
    }

    fn package_layer(digest: &str) -> OciLayer {
        OciLayer {
            kind: OciLayerKind::PackageTarGz,
            descriptor: descriptor(ZED_OCI_PACKAGE_TAR_GZ_MEDIA_TYPE_V1, digest, 1024),
            platform: None,
        }
    }

    #[test]
    fn parses_and_roundtrips_tagged_digest_reference() {
        let reference = OciReference::parse(&format!(
            "oci://registry.example:5000/team/tool:1.2.3@{DIGEST_A}"
        ))
        .unwrap();
        assert_eq!(reference.registry, "registry.example:5000");
        assert_eq!(reference.repository, "team/tool");
        assert_eq!(reference.tag.as_deref(), Some("1.2.3"));
        assert!(reference.is_immutable());
        assert_eq!(
            reference.to_string(),
            format!("oci://registry.example:5000/team/tool:1.2.3@{DIGEST_A}")
        );
    }

    #[test]
    fn reference_parser_fails_closed_on_ambiguous_or_mutable_input() {
        assert!(OciReference::parse("https://ghcr.io/acme/tool:1").is_err());
        assert!(OciReference::parse("oci://ghcr.io/acme/tool").is_err());
        assert!(OciReference::parse("oci://ghcr.io/Acme/tool:1").is_err());
        assert!(OciReference::parse("oci://user@example.com/acme/tool:1").is_err());
        assert!(OciReference::parse("oci://ghcr.io/acme/tool:1?x=y").is_err());
        assert!(OciDigest::parse("sha256:ABC").is_err());
    }

    #[test]
    fn finalized_record_requires_matching_manifest_digest() {
        let mut valid = record(vec![package_layer(DIGEST_C)]);
        valid.validate().unwrap();

        valid.manifest.digest = OciDigest::parse(DIGEST_D).unwrap();
        assert!(matches!(
            valid.validate(),
            Err(OciInteropError::InvalidAdapter(_))
        ));

        let mut tag_only = record(vec![package_layer(DIGEST_C)]);
        tag_only.reference.digest = None;
        assert!(tag_only.validate().is_err());
    }

    #[test]
    fn layer_media_type_and_platform_are_typed() {
        let mut manifest = OciLayer {
            kind: OciLayerKind::Manifest,
            descriptor: descriptor(ZED_OCI_MANIFEST_MEDIA_TYPE_V1, DIGEST_C, 64),
            platform: Some(OciPlatform {
                os: "linux".to_string(),
                architecture: "amd64".to_string(),
                variant: None,
            }),
        };
        assert!(manifest.validate().is_err());

        manifest.platform = None;
        manifest.descriptor.media_type = ZED_OCI_LOCK_MEDIA_TYPE_V1.to_string();
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn canonical_json_normalizes_layer_order() {
        let manifest_layer = OciLayer {
            kind: OciLayerKind::Manifest,
            descriptor: descriptor(ZED_OCI_MANIFEST_MEDIA_TYPE_V1, DIGEST_D, 64),
            platform: None,
        };
        let package_layer = package_layer(DIGEST_C);
        let first = record(vec![manifest_layer.clone(), package_layer.clone()]);
        let second = record(vec![package_layer, manifest_layer]);
        assert_eq!(
            first.canonical_json_bytes().unwrap(),
            second.canonical_json_bytes().unwrap()
        );
    }

    #[test]
    fn adapter_rejects_unknown_schema_duplicate_layers_and_missing_payload() {
        let mut unknown = record(vec![package_layer(DIGEST_C)]);
        unknown.schema = "zed.oci-adapter/v2".to_string();
        assert!(matches!(
            unknown.validate(),
            Err(OciInteropError::UnsupportedSchema(_))
        ));

        let duplicate = record(vec![package_layer(DIGEST_C), package_layer(DIGEST_C)]);
        assert!(duplicate.validate().is_err());

        let metadata_only = record(vec![OciLayer {
            kind: OciLayerKind::Manifest,
            descriptor: descriptor(ZED_OCI_MANIFEST_MEDIA_TYPE_V1, DIGEST_C, 64),
            platform: None,
        }]);
        assert!(metadata_only.validate().is_err());
    }
}
