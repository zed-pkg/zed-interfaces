//! Versioned, transport-neutral contracts for Zed registry protocol v1.
//!
//! The public read path is intentionally representable as immutable files. This
//! module defines registry identity, sparse index records, signed freshness
//! checkpoints, canonical archive manifests, publication requests, and explicit
//! lifecycle/error states. It performs no network, credential, storage, or
//! deployment operation.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Discovery and protocol identifier understood by v1 clients.
pub const REGISTRY_PROTOCOL_V1: &str = "zpkg.registry/v1";
/// Discovery document schema.
pub const REGISTRY_DISCOVERY_SCHEMA_V1: &str = "zpkg.registry-discovery/v1";
/// Sparse index-record schema.
pub const REGISTRY_INDEX_RECORD_SCHEMA_V1: &str = "zpkg.registry-index-record/v1";
/// Signed checkpoint schema.
pub const REGISTRY_CHECKPOINT_SCHEMA_V1: &str = "zpkg.registry-checkpoint/v1";
/// Canonical archive-manifest schema.
pub const REGISTRY_ARCHIVE_MANIFEST_SCHEMA_V1: &str = "zpkg.registry-archive-manifest/v1";
/// Immutable publication request schema.
pub const REGISTRY_PUBLISH_REQUEST_SCHEMA_V1: &str = "zpkg.registry-publish-request/v1";
/// Versioned protocol error schema.
pub const REGISTRY_PROTOCOL_ERROR_SCHEMA_V1: &str = "zpkg.registry-error/v1";

/// Stable endpoints advertised by a registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistryEndpointsV1 {
    pub sparse_index_template: String,
    pub package_template: String,
    pub checkpoint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publish: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub yank: Option<String>,
}

impl RegistryEndpointsV1 {
    fn validate(&self) -> Result<(), RegistryProtocolV1Error> {
        validate_endpoint_template(
            "endpoints.sparse_index_template",
            &self.sparse_index_template,
            &["{org}", "{name}"],
        )?;
        validate_endpoint_template(
            "endpoints.package_template",
            &self.package_template,
            &["{org}", "{name}", "{version}"],
        )?;
        validate_endpoint("endpoints.checkpoint", &self.checkpoint)?;
        if let Some(publish) = &self.publish {
            validate_endpoint_template(
                "endpoints.publish",
                publish,
                &["{org}", "{name}", "{version}"],
            )?;
        }
        if let Some(yank) = &self.yank {
            validate_endpoint_template("endpoints.yank", yank, &["{org}", "{name}", "{version}"])?;
        }
        Ok(())
    }
}

/// Additive feature flags used for client feature detection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistryCapabilitiesV1 {
    pub public_read: bool,
    pub publish: bool,
    pub yank: bool,
    pub private_packages: bool,
    pub static_export: bool,
    pub mirrors: bool,
}

/// Authentication mechanisms advertised by a registry.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum RegistryAuthModeV1 {
    AnonymousRead,
    StaticToken,
    OidcPkce,
    DeviceAuthorization,
    WorkloadIdentity,
}

/// Authentication descriptor. Identity assertions are exchanged for
/// registry-audience credentials; general identity-provider tokens are not
/// package credentials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistryAuthDescriptorV1 {
    pub mode: RegistryAuthModeV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audience: Option<String>,
}

impl RegistryAuthDescriptorV1 {
    fn validate(&self) -> Result<(), RegistryProtocolV1Error> {
        match self.mode {
            RegistryAuthModeV1::OidcPkce
            | RegistryAuthModeV1::DeviceAuthorization
            | RegistryAuthModeV1::WorkloadIdentity => {
                let issuer = self.issuer.as_deref().ok_or_else(|| {
                    RegistryProtocolV1Error::MissingField {
                        field: "auth.issuer".to_owned(),
                    }
                })?;
                validate_https_url("auth.issuer", issuer)?;
                let audience = self.audience.as_deref().ok_or_else(|| {
                    RegistryProtocolV1Error::MissingField {
                        field: "auth.audience".to_owned(),
                    }
                })?;
                validate_nonempty("auth.audience", audience)?;
            }
            RegistryAuthModeV1::AnonymousRead | RegistryAuthModeV1::StaticToken => {
                if self.issuer.is_some() {
                    return Err(RegistryProtocolV1Error::UnexpectedField {
                        field: "auth.issuer".to_owned(),
                    });
                }
            }
        }
        Ok(())
    }
}

/// Registry metadata-signing key state.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum RegistrySigningKeyStateV1 {
    Active,
    Retired,
}

/// Public metadata-signing key descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistrySigningKeyV1 {
    pub key_id: String,
    pub algorithm: String,
    pub public_key_multibase: String,
    pub state: RegistrySigningKeyStateV1,
}

impl RegistrySigningKeyV1 {
    fn validate(&self, field: &str) -> Result<(), RegistryProtocolV1Error> {
        validate_lower_token(&format!("{field}.key_id"), &self.key_id)?;
        if self.algorithm != "ed25519" {
            return Err(RegistryProtocolV1Error::UnsupportedValue {
                field: format!("{field}.algorithm"),
                value: self.algorithm.clone(),
            });
        }
        if !self.public_key_multibase.starts_with('z')
            || self.public_key_multibase.len() < 8
            || !self
                .public_key_multibase
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric())
        {
            return Err(RegistryProtocolV1Error::InvalidValue {
                field: format!("{field}.public_key_multibase"),
                value: self.public_key_multibase.clone(),
            });
        }
        Ok(())
    }
}

/// Server-enforced package/archive limits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistryLimitsV1 {
    pub max_archive_bytes: u64,
    pub max_expanded_bytes: u64,
    pub max_files: u64,
    pub max_path_bytes: u64,
    pub max_compression_ratio: u64,
}

impl RegistryLimitsV1 {
    fn validate(&self) -> Result<(), RegistryProtocolV1Error> {
        for (field, value) in [
            ("limits.max_archive_bytes", self.max_archive_bytes),
            ("limits.max_expanded_bytes", self.max_expanded_bytes),
            ("limits.max_files", self.max_files),
            ("limits.max_path_bytes", self.max_path_bytes),
            ("limits.max_compression_ratio", self.max_compression_ratio),
        ] {
            if value == 0 {
                return Err(RegistryProtocolV1Error::ZeroValue {
                    field: field.to_owned(),
                });
            }
        }
        if self.max_expanded_bytes < self.max_archive_bytes {
            return Err(RegistryProtocolV1Error::InvalidRelationship {
                message: "max_expanded_bytes must be at least max_archive_bytes".to_owned(),
            });
        }
        Ok(())
    }
}

/// Signed/versioned registry discovery document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistryDiscoveryV1 {
    pub schema: String,
    pub registry_id: String,
    pub canonical_url: String,
    pub protocol_versions: Vec<String>,
    pub endpoints: RegistryEndpointsV1,
    pub capabilities: RegistryCapabilitiesV1,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auth: Vec<RegistryAuthDescriptorV1>,
    pub signing_keys: Vec<RegistrySigningKeyV1>,
    pub limits: RegistryLimitsV1,
}

impl RegistryDiscoveryV1 {
    pub const SCHEMA_V1: &'static str = REGISTRY_DISCOVERY_SCHEMA_V1;

    pub fn validate(&self) -> Result<(), RegistryProtocolV1Error> {
        validate_schema("schema", &self.schema, Self::SCHEMA_V1)?;
        validate_registry_id(&self.registry_id)?;
        validate_canonical_url(&self.canonical_url)?;
        if self.protocol_versions.is_empty()
            || !self
                .protocol_versions
                .iter()
                .any(|version| version == REGISTRY_PROTOCOL_V1)
        {
            return Err(RegistryProtocolV1Error::MissingProtocolVersion {
                version: REGISTRY_PROTOCOL_V1.to_owned(),
            });
        }
        ensure_unique("protocol_versions", &self.protocol_versions)?;
        self.endpoints.validate()?;
        for auth in &self.auth {
            auth.validate()?;
        }
        ensure_unique_by(
            "auth",
            self.auth
                .iter()
                .map(|descriptor| format!("{:?}", descriptor.mode)),
        )?;

        if self.signing_keys.is_empty() {
            return Err(RegistryProtocolV1Error::MissingActiveSigningKey);
        }
        let mut active = 0_u64;
        let mut key_ids = BTreeSet::new();
        for (index, key) in self.signing_keys.iter().enumerate() {
            key.validate(&format!("signing_keys[{index}]"))?;
            if !key_ids.insert(key.key_id.clone()) {
                return Err(RegistryProtocolV1Error::DuplicateValue {
                    field: "signing_keys.key_id".to_owned(),
                    value: key.key_id.clone(),
                });
            }
            if key.state == RegistrySigningKeyStateV1::Active {
                active += 1;
            }
        }
        if active == 0 {
            return Err(RegistryProtocolV1Error::MissingActiveSigningKey);
        }
        self.limits.validate()
    }

    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>, RegistryProtocolV1Error> {
        self.validate()?;
        let mut normalized = self.clone();
        normalized.protocol_versions.sort();
        normalized.auth.sort_by_key(|descriptor| descriptor.mode);
        normalized
            .signing_keys
            .sort_by(|left, right| left.key_id.cmp(&right.key_id));
        serde_json::to_vec(&normalized).map_err(RegistryProtocolV1Error::Serialization)
    }
}

/// Package version lifecycle. Version identities remain permanently burned.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum RegistryLifecycleStateV1 {
    Active,
    Yanked,
    SecurityRevoked,
    LegalTombstoned,
}

/// Immutable archive reference bound by signed metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistryArchiveReferenceV1 {
    pub sha256: String,
    pub manifest_sha256: String,
    pub size: u64,
    pub format: RegistryArchiveFormatV1,
}

impl RegistryArchiveReferenceV1 {
    fn validate(&self, field: &str) -> Result<(), RegistryProtocolV1Error> {
        validate_sha256(&format!("{field}.sha256"), &self.sha256)?;
        validate_sha256(&format!("{field}.manifest_sha256"), &self.manifest_sha256)?;
        if self.size == 0 {
            return Err(RegistryProtocolV1Error::ZeroValue {
                field: format!("{field}.size"),
            });
        }
        Ok(())
    }
}

/// Archive formats accepted by protocol v1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum RegistryArchiveFormatV1 {
    TarZstd,
}

/// One dependency edge with an explicit registry trust identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistryDependencyV1 {
    pub registry_id: String,
    pub org: String,
    pub name: String,
    pub requirement: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
}

impl RegistryDependencyV1 {
    fn validate(&self, field: &str) -> Result<(), RegistryProtocolV1Error> {
        validate_registry_id(&self.registry_id)?;
        validate_coordinate_component(&format!("{field}.org"), &self.org)?;
        validate_coordinate_component(&format!("{field}.name"), &self.name)?;
        validate_nonempty(&format!("{field}.requirement"), &self.requirement)?;
        for (index, feature) in self.features.iter().enumerate() {
            validate_lower_token(&format!("{field}.features[{index}]"), feature)?;
        }
        ensure_unique(&format!("{field}.features"), &self.features)
    }
}

/// One authoritative NDJSON sparse-index record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistryIndexRecordV1 {
    pub schema: String,
    pub registry_id: String,
    pub org: String,
    pub name: String,
    pub version: String,
    pub archive: RegistryArchiveReferenceV1,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<RegistryDependencyV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
    pub lifecycle: RegistryLifecycleStateV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lifecycle_reason: Option<String>,
    pub checkpoint_sequence: u64,
}

impl RegistryIndexRecordV1 {
    pub const SCHEMA_V1: &'static str = REGISTRY_INDEX_RECORD_SCHEMA_V1;

    pub fn validate(&self) -> Result<(), RegistryProtocolV1Error> {
        validate_schema("schema", &self.schema, Self::SCHEMA_V1)?;
        validate_registry_id(&self.registry_id)?;
        validate_coordinate_component("org", &self.org)?;
        validate_coordinate_component("name", &self.name)?;
        validate_version("version", &self.version)?;
        self.archive.validate("archive")?;
        if self.checkpoint_sequence == 0 {
            return Err(RegistryProtocolV1Error::ZeroValue {
                field: "checkpoint_sequence".to_owned(),
            });
        }
        for (index, dependency) in self.dependencies.iter().enumerate() {
            dependency.validate(&format!("dependencies[{index}]"))?;
        }
        ensure_unique_by(
            "dependencies",
            self.dependencies.iter().map(|dependency| {
                format!(
                    "{}/{}/{}",
                    dependency.registry_id, dependency.org, dependency.name
                )
            }),
        )?;
        for (field, values) in [("targets", &self.targets), ("features", &self.features)] {
            for (index, value) in values.iter().enumerate() {
                validate_lower_token(&format!("{field}[{index}]"), value)?;
            }
            ensure_unique(field, values)?;
        }
        match self.lifecycle {
            RegistryLifecycleStateV1::Active if self.lifecycle_reason.is_some() => {
                Err(RegistryProtocolV1Error::UnexpectedField {
                    field: "lifecycle_reason".to_owned(),
                })
            }
            RegistryLifecycleStateV1::Active => Ok(()),
            _ => {
                let reason = self.lifecycle_reason.as_deref().ok_or_else(|| {
                    RegistryProtocolV1Error::MissingField {
                        field: "lifecycle_reason".to_owned(),
                    }
                })?;
                validate_nonempty("lifecycle_reason", reason)
            }
        }
    }

    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>, RegistryProtocolV1Error> {
        self.validate()?;
        let mut normalized = self.clone();
        normalized.dependencies.sort();
        for dependency in &mut normalized.dependencies {
            dependency.features.sort();
        }
        normalized.targets.sort();
        normalized.features.sort();
        serde_json::to_vec(&normalized).map_err(RegistryProtocolV1Error::Serialization)
    }
}

/// Signed, monotonically advancing freshness checkpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistryCheckpointV1 {
    pub schema: String,
    pub registry_id: String,
    pub sequence: u64,
    pub generated_at: String,
    pub expires_at: String,
    pub index_root_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_checkpoint_sha256: Option<String>,
    pub signing_key_id: String,
    pub signature: String,
}

impl RegistryCheckpointV1 {
    pub const SCHEMA_V1: &'static str = REGISTRY_CHECKPOINT_SCHEMA_V1;

    pub fn validate(&self) -> Result<(), RegistryProtocolV1Error> {
        validate_schema("schema", &self.schema, Self::SCHEMA_V1)?;
        validate_registry_id(&self.registry_id)?;
        if self.sequence == 0 {
            return Err(RegistryProtocolV1Error::ZeroValue {
                field: "sequence".to_owned(),
            });
        }
        validate_utc_timestamp("generated_at", &self.generated_at)?;
        validate_utc_timestamp("expires_at", &self.expires_at)?;
        if self.generated_at >= self.expires_at {
            return Err(RegistryProtocolV1Error::InvalidRelationship {
                message: "generated_at must precede expires_at".to_owned(),
            });
        }
        validate_sha256("index_root_sha256", &self.index_root_sha256)?;
        if let Some(previous) = &self.previous_checkpoint_sha256 {
            validate_sha256("previous_checkpoint_sha256", previous)?;
        } else if self.sequence != 1 {
            return Err(RegistryProtocolV1Error::MissingField {
                field: "previous_checkpoint_sha256".to_owned(),
            });
        }
        validate_lower_token("signing_key_id", &self.signing_key_id)?;
        validate_signature("signature", &self.signature)
    }

    /// Bytes covered by `signature`. The signature itself is excluded.
    pub fn signing_payload_bytes(&self) -> Result<Vec<u8>, RegistryProtocolV1Error> {
        self.validate()?;
        #[derive(Serialize)]
        struct Payload<'a> {
            schema: &'a str,
            registry_id: &'a str,
            sequence: u64,
            generated_at: &'a str,
            expires_at: &'a str,
            index_root_sha256: &'a str,
            previous_checkpoint_sha256: &'a Option<String>,
            signing_key_id: &'a str,
        }
        serde_json::to_vec(&Payload {
            schema: &self.schema,
            registry_id: &self.registry_id,
            sequence: self.sequence,
            generated_at: &self.generated_at,
            expires_at: &self.expires_at,
            index_root_sha256: &self.index_root_sha256,
            previous_checkpoint_sha256: &self.previous_checkpoint_sha256,
            signing_key_id: &self.signing_key_id,
        })
        .map_err(RegistryProtocolV1Error::Serialization)
    }
}

/// Entry kinds supported by a canonical package archive.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum RegistryArchiveEntryKindV1 {
    Directory,
    File,
    Symlink,
}

/// One normalized package archive entry.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistryArchiveEntryV1 {
    pub path: String,
    pub kind: RegistryArchiveEntryKindV1,
    pub mode: u32,
    pub size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link_target: Option<String>,
}

impl RegistryArchiveEntryV1 {
    fn validate(&self, field: &str) -> Result<(), RegistryProtocolV1Error> {
        validate_safe_relative_path(&format!("{field}.path"), &self.path)?;
        if self.mode > 0o777 {
            return Err(RegistryProtocolV1Error::InvalidValue {
                field: format!("{field}.mode"),
                value: format!("{:o}", self.mode),
            });
        }
        match self.kind {
            RegistryArchiveEntryKindV1::File => {
                let digest = self.sha256.as_deref().ok_or_else(|| {
                    RegistryProtocolV1Error::MissingField {
                        field: format!("{field}.sha256"),
                    }
                })?;
                validate_sha256(&format!("{field}.sha256"), digest)?;
                if self.link_target.is_some() {
                    return Err(RegistryProtocolV1Error::UnexpectedField {
                        field: format!("{field}.link_target"),
                    });
                }
            }
            RegistryArchiveEntryKindV1::Directory => {
                if self.size != 0 {
                    return Err(RegistryProtocolV1Error::InvalidValue {
                        field: format!("{field}.size"),
                        value: self.size.to_string(),
                    });
                }
                if self.sha256.is_some() || self.link_target.is_some() {
                    return Err(RegistryProtocolV1Error::UnexpectedField {
                        field: format!("{field}.sha256/link_target"),
                    });
                }
            }
            RegistryArchiveEntryKindV1::Symlink => {
                if self.size != 0 || self.sha256.is_some() {
                    return Err(RegistryProtocolV1Error::UnexpectedField {
                        field: format!("{field}.size/sha256"),
                    });
                }
                let target = self.link_target.as_deref().ok_or_else(|| {
                    RegistryProtocolV1Error::MissingField {
                        field: format!("{field}.link_target"),
                    }
                })?;
                validate_safe_relative_path(&format!("{field}.link_target"), target)?;
            }
        }
        Ok(())
    }
}

/// Canonical archive contents bound to an immutable package version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistryArchiveManifestV1 {
    pub schema: String,
    pub registry_id: String,
    pub org: String,
    pub name: String,
    pub version: String,
    pub archive_sha256: String,
    pub expanded_size: u64,
    pub entries: Vec<RegistryArchiveEntryV1>,
}

impl RegistryArchiveManifestV1 {
    pub const SCHEMA_V1: &'static str = REGISTRY_ARCHIVE_MANIFEST_SCHEMA_V1;

    pub fn validate(&self) -> Result<(), RegistryProtocolV1Error> {
        validate_schema("schema", &self.schema, Self::SCHEMA_V1)?;
        validate_registry_id(&self.registry_id)?;
        validate_coordinate_component("org", &self.org)?;
        validate_coordinate_component("name", &self.name)?;
        validate_version("version", &self.version)?;
        validate_sha256("archive_sha256", &self.archive_sha256)?;
        if self.entries.is_empty() {
            return Err(RegistryProtocolV1Error::MissingField {
                field: "entries".to_owned(),
            });
        }
        let mut previous: Option<&str> = None;
        let mut total_file_size = 0_u64;
        for (index, entry) in self.entries.iter().enumerate() {
            entry.validate(&format!("entries[{index}]"))?;
            if previous.is_some_and(|value| value >= entry.path.as_str()) {
                return Err(RegistryProtocolV1Error::NonCanonicalOrder {
                    field: "entries.path".to_owned(),
                });
            }
            previous = Some(&entry.path);
            if entry.kind == RegistryArchiveEntryKindV1::File {
                total_file_size = total_file_size.checked_add(entry.size).ok_or_else(|| {
                    RegistryProtocolV1Error::InvalidRelationship {
                        message: "archive entry sizes overflow u64".to_owned(),
                    }
                })?;
            }
        }
        if self.expanded_size != total_file_size {
            return Err(RegistryProtocolV1Error::InvalidRelationship {
                message: format!(
                    "expanded_size {} does not equal file-entry total {total_file_size}",
                    self.expanded_size
                ),
            });
        }
        Ok(())
    }

    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>, RegistryProtocolV1Error> {
        self.validate()?;
        serde_json::to_vec(self).map_err(RegistryProtocolV1Error::Serialization)
    }
}

/// Visibility of an immutable publication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RegistryVisibilityV1 {
    Public,
    Private,
}

/// Idempotent publication request. Reusing a version succeeds only when both
/// immutable digests match the previously accepted bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistryPublishRequestV1 {
    pub schema: String,
    pub registry_id: String,
    pub org: String,
    pub name: String,
    pub version: String,
    pub archive: RegistryArchiveReferenceV1,
    pub visibility: RegistryVisibilityV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_precondition: Option<u64>,
}

impl RegistryPublishRequestV1 {
    pub const SCHEMA_V1: &'static str = REGISTRY_PUBLISH_REQUEST_SCHEMA_V1;

    pub fn validate(&self) -> Result<(), RegistryProtocolV1Error> {
        validate_schema("schema", &self.schema, Self::SCHEMA_V1)?;
        validate_registry_id(&self.registry_id)?;
        validate_coordinate_component("org", &self.org)?;
        validate_coordinate_component("name", &self.name)?;
        validate_version("version", &self.version)?;
        self.archive.validate("archive")?;
        if self.checkpoint_precondition == Some(0) {
            return Err(RegistryProtocolV1Error::ZeroValue {
                field: "checkpoint_precondition".to_owned(),
            });
        }
        Ok(())
    }
}

/// Stable protocol-level error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum RegistryProtocolErrorCodeV1 {
    InvalidRequest,
    UnsupportedProtocol,
    RegistryIdentityMismatch,
    Unauthorized,
    Forbidden,
    PackageNotFound,
    VersionNotFound,
    VersionYanked,
    VersionSecurityRevoked,
    VersionLegalTombstoned,
    ImmutableVersionConflict,
    CheckpointPreconditionFailed,
    MetadataExpired,
    MetadataRollback,
    ArchiveRejected,
    RateLimited,
    Internal,
}

/// Machine-readable registry error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistryProtocolErrorV1 {
    pub schema: String,
    pub code: RegistryProtocolErrorCodeV1,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_after_seconds: Option<u64>,
}

impl RegistryProtocolErrorV1 {
    pub const SCHEMA_V1: &'static str = REGISTRY_PROTOCOL_ERROR_SCHEMA_V1;

    pub fn validate(&self) -> Result<(), RegistryProtocolV1Error> {
        validate_schema("schema", &self.schema, Self::SCHEMA_V1)?;
        validate_nonempty("message", &self.message)?;
        if let Some(registry_id) = &self.registry_id {
            validate_registry_id(registry_id)?;
        }
        if self.retry_after_seconds == Some(0) {
            return Err(RegistryProtocolV1Error::ZeroValue {
                field: "retry_after_seconds".to_owned(),
            });
        }
        Ok(())
    }
}

/// Validation/serialization failures for protocol v1 DTOs.
#[derive(Debug, Error)]
pub enum RegistryProtocolV1Error {
    #[error("field `{field}` must not be empty")]
    MissingField { field: String },
    #[error("field `{field}` is not allowed in this state")]
    UnexpectedField { field: String },
    #[error("field `{field}` must be non-zero")]
    ZeroValue { field: String },
    #[error("field `{field}` has invalid value `{value}`")]
    InvalidValue { field: String, value: String },
    #[error("field `{field}` has unsupported value `{value}`")]
    UnsupportedValue { field: String, value: String },
    #[error("field `{field}` expected schema `{expected}`, got `{actual}`")]
    InvalidSchema {
        field: String,
        expected: String,
        actual: String,
    },
    #[error("required protocol version `{version}` is absent")]
    MissingProtocolVersion { version: String },
    #[error("no active metadata-signing key is present")]
    MissingActiveSigningKey,
    #[error("duplicate value `{value}` in `{field}`")]
    DuplicateValue { field: String, value: String },
    #[error("field `{field}` is not in canonical order")]
    NonCanonicalOrder { field: String },
    #[error("invalid relationship: {message}")]
    InvalidRelationship { message: String },
    #[error("JSON serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
}

fn validate_schema(
    field: &str,
    actual: &str,
    expected: &str,
) -> Result<(), RegistryProtocolV1Error> {
    if actual == expected {
        Ok(())
    } else {
        Err(RegistryProtocolV1Error::InvalidSchema {
            field: field.to_owned(),
            expected: expected.to_owned(),
            actual: actual.to_owned(),
        })
    }
}

fn validate_registry_id(value: &str) -> Result<(), RegistryProtocolV1Error> {
    const PREFIX: &str = "zpkg-registry:";
    let suffix =
        value
            .strip_prefix(PREFIX)
            .ok_or_else(|| RegistryProtocolV1Error::InvalidValue {
                field: "registry_id".to_owned(),
                value: value.to_owned(),
            })?;
    if suffix.len() == 32
        && suffix
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(RegistryProtocolV1Error::InvalidValue {
            field: "registry_id".to_owned(),
            value: value.to_owned(),
        })
    }
}

fn validate_canonical_url(value: &str) -> Result<(), RegistryProtocolV1Error> {
    validate_https_url("canonical_url", value)?;
    if value.ends_with('/') || value.contains('#') || value.contains('?') || value.contains('@') {
        return Err(RegistryProtocolV1Error::InvalidValue {
            field: "canonical_url".to_owned(),
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn validate_https_url(field: &str, value: &str) -> Result<(), RegistryProtocolV1Error> {
    if !value.starts_with("https://")
        || value.len() <= "https://".len()
        || value.bytes().any(|byte| byte.is_ascii_whitespace())
    {
        return Err(RegistryProtocolV1Error::InvalidValue {
            field: field.to_owned(),
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn validate_endpoint(field: &str, value: &str) -> Result<(), RegistryProtocolV1Error> {
    if !value.starts_with('/')
        || value.starts_with("//")
        || value.contains("..")
        || value.contains('\\')
        || value.bytes().any(|byte| byte.is_ascii_whitespace())
    {
        return Err(RegistryProtocolV1Error::InvalidValue {
            field: field.to_owned(),
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn validate_endpoint_template(
    field: &str,
    value: &str,
    required_tokens: &[&str],
) -> Result<(), RegistryProtocolV1Error> {
    validate_endpoint(field, value)?;
    for token in required_tokens {
        if !value.contains(token) {
            return Err(RegistryProtocolV1Error::MissingField {
                field: format!("{field}:{token}"),
            });
        }
    }
    Ok(())
}

fn validate_coordinate_component(field: &str, value: &str) -> Result<(), RegistryProtocolV1Error> {
    validate_lower_token(field, value)
}

fn validate_lower_token(field: &str, value: &str) -> Result<(), RegistryProtocolV1Error> {
    let bytes = value.as_bytes();
    if bytes.is_empty()
        || bytes.len() > 128
        || !bytes[0].is_ascii_lowercase()
        || !bytes[bytes.len() - 1].is_ascii_alphanumeric()
        || !bytes.iter().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(*byte, b'-' | b'_' | b'.')
        })
    {
        return Err(RegistryProtocolV1Error::InvalidValue {
            field: field.to_owned(),
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn validate_version(field: &str, value: &str) -> Result<(), RegistryProtocolV1Error> {
    let parsed =
        semver::Version::parse(value).map_err(|_| RegistryProtocolV1Error::InvalidValue {
            field: field.to_owned(),
            value: value.to_owned(),
        })?;
    if parsed.build.is_empty() {
        Ok(())
    } else {
        Err(RegistryProtocolV1Error::InvalidValue {
            field: field.to_owned(),
            value: value.to_owned(),
        })
    }
}

fn validate_sha256(field: &str, value: &str) -> Result<(), RegistryProtocolV1Error> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(RegistryProtocolV1Error::InvalidValue {
            field: field.to_owned(),
            value: value.to_owned(),
        })
    }
}

fn validate_signature(field: &str, value: &str) -> Result<(), RegistryProtocolV1Error> {
    if value.len() >= 16
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        Ok(())
    } else {
        Err(RegistryProtocolV1Error::InvalidValue {
            field: field.to_owned(),
            value: value.to_owned(),
        })
    }
}

fn validate_utc_timestamp(field: &str, value: &str) -> Result<(), RegistryProtocolV1Error> {
    if value.len() >= 20 && value.contains('T') && value.ends_with('Z') {
        Ok(())
    } else {
        Err(RegistryProtocolV1Error::InvalidValue {
            field: field.to_owned(),
            value: value.to_owned(),
        })
    }
}

fn validate_safe_relative_path(field: &str, value: &str) -> Result<(), RegistryProtocolV1Error> {
    if value.is_empty()
        || value.len() > 1024
        || value.starts_with('/')
        || value.ends_with('/')
        || value.contains('\\')
        || value.contains('\0')
        || value
            .split('/')
            .any(|component| component.is_empty() || matches!(component, "." | ".."))
    {
        return Err(RegistryProtocolV1Error::InvalidValue {
            field: field.to_owned(),
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn validate_nonempty(field: &str, value: &str) -> Result<(), RegistryProtocolV1Error> {
    if value.trim().is_empty() {
        Err(RegistryProtocolV1Error::MissingField {
            field: field.to_owned(),
        })
    } else {
        Ok(())
    }
}

fn ensure_unique(field: &str, values: &[String]) -> Result<(), RegistryProtocolV1Error> {
    ensure_unique_by(field, values.iter().cloned())
}

fn ensure_unique_by<I>(field: &str, values: I) -> Result<(), RegistryProtocolV1Error>
where
    I: IntoIterator<Item = String>,
{
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value.clone()) {
            return Err(RegistryProtocolV1Error::DuplicateValue {
                field: field.to_owned(),
                value,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const REGISTRY_ID: &str = "zpkg-registry:0123456789abcdef0123456789abcdef";
    const SHA_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const SHA_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn discovery() -> RegistryDiscoveryV1 {
        RegistryDiscoveryV1 {
            schema: REGISTRY_DISCOVERY_SCHEMA_V1.to_owned(),
            registry_id: REGISTRY_ID.to_owned(),
            canonical_url: "https://registry.example.test".to_owned(),
            protocol_versions: vec![REGISTRY_PROTOCOL_V1.to_owned()],
            endpoints: RegistryEndpointsV1 {
                sparse_index_template: "/index/{org}/{name}".to_owned(),
                package_template: "/pkgs/{org}/{name}/{version}.tar.zst".to_owned(),
                checkpoint: "/checkpoint.json".to_owned(),
                publish: Some("/api/v1/packages/{org}/{name}/{version}".to_owned()),
                yank: Some("/api/v1/packages/{org}/{name}/{version}/yank".to_owned()),
            },
            capabilities: RegistryCapabilitiesV1 {
                public_read: true,
                publish: true,
                yank: true,
                private_packages: true,
                static_export: true,
                mirrors: true,
            },
            auth: vec![RegistryAuthDescriptorV1 {
                mode: RegistryAuthModeV1::OidcPkce,
                issuer: Some("https://auth.example.test".to_owned()),
                audience: Some("registry.example.test".to_owned()),
            }],
            signing_keys: vec![RegistrySigningKeyV1 {
                key_id: "metadata-2026-01".to_owned(),
                algorithm: "ed25519".to_owned(),
                public_key_multibase: "z6MkrJVnaZkeFzdQ".to_owned(),
                state: RegistrySigningKeyStateV1::Active,
            }],
            limits: RegistryLimitsV1 {
                max_archive_bytes: 100 * 1024 * 1024,
                max_expanded_bytes: 500 * 1024 * 1024,
                max_files: 100_000,
                max_path_bytes: 1024,
                max_compression_ratio: 100,
            },
        }
    }

    fn index_record() -> RegistryIndexRecordV1 {
        RegistryIndexRecordV1 {
            schema: REGISTRY_INDEX_RECORD_SCHEMA_V1.to_owned(),
            registry_id: REGISTRY_ID.to_owned(),
            org: "acme".to_owned(),
            name: "widget".to_owned(),
            version: "1.2.3".to_owned(),
            archive: RegistryArchiveReferenceV1 {
                sha256: SHA_A.to_owned(),
                manifest_sha256: SHA_B.to_owned(),
                size: 42,
                format: RegistryArchiveFormatV1::TarZstd,
            },
            dependencies: vec![RegistryDependencyV1 {
                registry_id: REGISTRY_ID.to_owned(),
                org: "acme".to_owned(),
                name: "shared".to_owned(),
                requirement: "^1.0.0".to_owned(),
                features: vec!["serde".to_owned()],
            }],
            targets: vec!["rust".to_owned()],
            features: vec!["default".to_owned()],
            lifecycle: RegistryLifecycleStateV1::Active,
            lifecycle_reason: None,
            checkpoint_sequence: 7,
        }
    }

    fn archive_manifest() -> RegistryArchiveManifestV1 {
        RegistryArchiveManifestV1 {
            schema: REGISTRY_ARCHIVE_MANIFEST_SCHEMA_V1.to_owned(),
            registry_id: REGISTRY_ID.to_owned(),
            org: "acme".to_owned(),
            name: "widget".to_owned(),
            version: "1.2.3".to_owned(),
            archive_sha256: SHA_A.to_owned(),
            expanded_size: 3,
            entries: vec![
                RegistryArchiveEntryV1 {
                    path: "README.md".to_owned(),
                    kind: RegistryArchiveEntryKindV1::File,
                    mode: 0o644,
                    size: 3,
                    sha256: Some(SHA_B.to_owned()),
                    link_target: None,
                },
                RegistryArchiveEntryV1 {
                    path: "src".to_owned(),
                    kind: RegistryArchiveEntryKindV1::Directory,
                    mode: 0o755,
                    size: 0,
                    sha256: None,
                    link_target: None,
                },
            ],
        }
    }

    #[test]
    fn valid_contracts_round_trip_and_validate() {
        let discovery = discovery();
        discovery.validate().expect("discovery validates");
        let encoded = serde_json::to_vec(&discovery).expect("discovery serializes");
        let decoded: RegistryDiscoveryV1 =
            serde_json::from_slice(&encoded).expect("discovery deserializes");
        assert_eq!(decoded, discovery);

        index_record().validate().expect("index validates");
        archive_manifest().validate().expect("archive validates");
    }

    #[test]
    fn local_alias_or_url_cannot_replace_registry_identity() {
        let mut record = index_record();
        record.registry_id = "corp".to_owned();
        assert!(record.validate().is_err());

        let mut discovery = discovery();
        discovery.canonical_url = "https://user:token@registry.example.test".to_owned();
        assert!(discovery.validate().is_err());
    }

    #[test]
    fn lifecycle_reason_is_explicit_and_version_identity_stays_burned() {
        let mut record = index_record();
        record.lifecycle = RegistryLifecycleStateV1::LegalTombstoned;
        assert!(record.validate().is_err());
        record.lifecycle_reason = Some("legal-order".to_owned());
        record.validate().expect("tombstone reason is explicit");
    }

    #[test]
    fn metadata_sequence_and_signature_are_fail_closed() {
        let checkpoint = RegistryCheckpointV1 {
            schema: REGISTRY_CHECKPOINT_SCHEMA_V1.to_owned(),
            registry_id: REGISTRY_ID.to_owned(),
            sequence: 2,
            generated_at: "2026-08-07T00:00:00Z".to_owned(),
            expires_at: "2026-08-08T00:00:00Z".to_owned(),
            index_root_sha256: SHA_A.to_owned(),
            previous_checkpoint_sha256: None,
            signing_key_id: "metadata-2026-01".to_owned(),
            signature: "AbCdEfGhIjKlMnOpQrStUvWxYz_01234".to_owned(),
        };
        assert!(checkpoint.validate().is_err());
    }

    #[test]
    fn archive_rejects_traversal_links_and_noncanonical_order() {
        let mut manifest = archive_manifest();
        manifest.entries[0].path = "../secret".to_owned();
        assert!(manifest.validate().is_err());

        let mut manifest = archive_manifest();
        manifest.entries.reverse();
        assert!(manifest.validate().is_err());

        let mut manifest = archive_manifest();
        manifest.entries.push(RegistryArchiveEntryV1 {
            path: "unsafe-link".to_owned(),
            kind: RegistryArchiveEntryKindV1::Symlink,
            mode: 0o777,
            size: 0,
            sha256: None,
            link_target: Some("../../outside".to_owned()),
        });
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn changed_bytes_for_same_version_are_detectable() {
        let first = index_record();
        let mut second = first.clone();
        second.archive.sha256 = SHA_B.to_owned();
        assert_ne!(
            first.canonical_json_bytes().expect("first canonicalizes"),
            second.canonical_json_bytes().expect("second canonicalizes")
        );
    }

    #[test]
    fn golden_fixtures_deserialize_and_validate() {
        let discovery: RegistryDiscoveryV1 =
            serde_json::from_str(include_str!("../fixtures/registry-v1/discovery.json"))
                .expect("discovery fixture parses");
        discovery.validate().expect("discovery fixture validates");

        for line in include_str!("../fixtures/registry-v1/index.ndjson").lines() {
            let record: RegistryIndexRecordV1 =
                serde_json::from_str(line).expect("index fixture line parses");
            record.validate().expect("index fixture line validates");
        }

        let checkpoint: RegistryCheckpointV1 =
            serde_json::from_str(include_str!("../fixtures/registry-v1/checkpoint.json"))
                .expect("checkpoint fixture parses");
        checkpoint.validate().expect("checkpoint fixture validates");

        let archive: RegistryArchiveManifestV1 = serde_json::from_str(include_str!(
            "../fixtures/registry-v1/archive-manifest.json"
        ))
        .expect("archive fixture parses");
        archive.validate().expect("archive fixture validates");
    }
}
