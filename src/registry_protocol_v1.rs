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
use serde_json::Value;
use thiserror::Error;

/// Discovery and protocol identifier understood by v1 clients.
pub const REGISTRY_PROTOCOL_V1: &str = "zpkg.registry/v1";
/// Discovery document schema.
pub const REGISTRY_DISCOVERY_SCHEMA_V1: &str = "zpkg.registry-discovery/v1";
/// Sparse index-record schema.
pub const REGISTRY_INDEX_RECORD_SCHEMA_V1: &str = "zpkg.registry-index-record/v1";
/// Immutable sparse-index snapshot-manifest schema.
pub const REGISTRY_INDEX_SNAPSHOT_SCHEMA_V1: &str = "zpkg.registry-index-snapshot/v1";
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
    pub snapshot_manifest_template: String,
    pub package_template: String,
    pub archive_manifest_template: String,
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
            &["{snapshot}", "{org}", "{name}"],
        )?;
        validate_endpoint_template(
            "endpoints.snapshot_manifest_template",
            &self.snapshot_manifest_template,
            &["{snapshot}"],
        )?;
        validate_endpoint_template(
            "endpoints.package_template",
            &self.package_template,
            &["{org}", "{name}", "{version}"],
        )?;
        validate_endpoint_template(
            "endpoints.archive_manifest_template",
            &self.archive_manifest_template,
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

/// Signature made by a locally enrolled recovery/root key.
///
/// Root public keys are deliberately not self-asserted by discovery. Their
/// fingerprints and threshold policy are enrolled out of band. These
/// signatures delegate the online checkpoint-signing keys carried by the
/// discovery payload.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistryRootSignatureV1 {
    pub key_id: String,
    pub algorithm: String,
    pub signature: String,
}

impl RegistryRootSignatureV1 {
    fn validate(&self, field: &str) -> Result<(), RegistryProtocolV1Error> {
        validate_lower_token(&format!("{field}.key_id"), &self.key_id)?;
        if self.algorithm != "ed25519" {
            return Err(RegistryProtocolV1Error::UnsupportedValue {
                field: format!("{field}.algorithm"),
                value: self.algorithm.clone(),
            });
        }
        validate_signature(&format!("{field}.signature"), &self.signature)
    }
}

/// Root-signed, versioned registry discovery document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistryDiscoveryV1 {
    pub schema: String,
    pub version: u64,
    pub generated_at: String,
    pub expires_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_discovery_sha256: Option<String>,
    pub registry_id: String,
    pub canonical_url: String,
    pub protocol_versions: Vec<String>,
    pub endpoints: RegistryEndpointsV1,
    pub capabilities: RegistryCapabilitiesV1,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auth: Vec<RegistryAuthDescriptorV1>,
    pub signing_keys: Vec<RegistrySigningKeyV1>,
    pub accepted_digest_algorithms: Vec<String>,
    pub accepted_archive_formats: Vec<RegistryArchiveFormatV1>,
    pub limits: RegistryLimitsV1,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub root_signatures: Vec<RegistryRootSignatureV1>,
}

impl RegistryDiscoveryV1 {
    pub const SCHEMA_V1: &'static str = REGISTRY_DISCOVERY_SCHEMA_V1;

    fn validate_payload_fields(&self) -> Result<(), RegistryProtocolV1Error> {
        validate_schema("schema", &self.schema, Self::SCHEMA_V1)?;
        if self.version == 0 {
            return Err(RegistryProtocolV1Error::ZeroValue {
                field: "version".to_owned(),
            });
        }
        validate_utc_timestamp("generated_at", &self.generated_at)?;
        validate_utc_timestamp("expires_at", &self.expires_at)?;
        if self.generated_at >= self.expires_at {
            return Err(RegistryProtocolV1Error::InvalidRelationship {
                message: "generated_at must precede expires_at".to_owned(),
            });
        }
        match (self.version, self.previous_discovery_sha256.as_deref()) {
            (1, None) => {}
            (1, Some(_)) => {
                return Err(RegistryProtocolV1Error::UnexpectedField {
                    field: "previous_discovery_sha256".to_owned(),
                });
            }
            (_, Some(previous)) => {
                validate_sha256("previous_discovery_sha256", previous)?;
            }
            (_, None) => {
                return Err(RegistryProtocolV1Error::MissingField {
                    field: "previous_discovery_sha256".to_owned(),
                });
            }
        }

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
        let has_anonymous = self
            .auth
            .iter()
            .any(|descriptor| descriptor.mode == RegistryAuthModeV1::AnonymousRead);
        let has_authenticated = self
            .auth
            .iter()
            .any(|descriptor| descriptor.mode != RegistryAuthModeV1::AnonymousRead);
        if self.capabilities.public_read != has_anonymous {
            return Err(RegistryProtocolV1Error::InvalidRelationship {
                message: "public_read must match the anonymous-read auth descriptor".to_owned(),
            });
        }
        if self.capabilities.publish != self.endpoints.publish.is_some() {
            return Err(RegistryProtocolV1Error::InvalidRelationship {
                message: "publish capability and endpoint must agree".to_owned(),
            });
        }
        if self.capabilities.yank != self.endpoints.yank.is_some() {
            return Err(RegistryProtocolV1Error::InvalidRelationship {
                message: "yank capability and endpoint must agree".to_owned(),
            });
        }
        if self.capabilities.static_export && !self.capabilities.public_read {
            return Err(RegistryProtocolV1Error::InvalidRelationship {
                message: "static_export requires public_read".to_owned(),
            });
        }
        if (self.capabilities.publish
            || self.capabilities.yank
            || self.capabilities.private_packages)
            && !has_authenticated
        {
            return Err(RegistryProtocolV1Error::InvalidRelationship {
                message: "write/private capabilities require an authenticated mode".to_owned(),
            });
        }

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

        if self.accepted_digest_algorithms.is_empty()
            || !self
                .accepted_digest_algorithms
                .iter()
                .any(|algorithm| algorithm == "sha256")
        {
            return Err(RegistryProtocolV1Error::MissingField {
                field: "accepted_digest_algorithms:sha256".to_owned(),
            });
        }
        for (index, algorithm) in self.accepted_digest_algorithms.iter().enumerate() {
            validate_lower_token(&format!("accepted_digest_algorithms[{index}]"), algorithm)?;
        }
        ensure_unique(
            "accepted_digest_algorithms",
            &self.accepted_digest_algorithms,
        )?;

        if self.accepted_archive_formats.is_empty()
            || !self
                .accepted_archive_formats
                .contains(&RegistryArchiveFormatV1::TarZstd)
        {
            return Err(RegistryProtocolV1Error::MissingField {
                field: "accepted_archive_formats:tar-zstd".to_owned(),
            });
        }
        ensure_unique_by(
            "accepted_archive_formats",
            self.accepted_archive_formats
                .iter()
                .map(|format| format!("{format:?}")),
        )?;
        self.limits.validate()
    }

    pub fn validate(&self) -> Result<(), RegistryProtocolV1Error> {
        self.validate_payload_fields()?;
        if self.root_signatures.is_empty() {
            return Err(RegistryProtocolV1Error::MissingField {
                field: "root_signatures".to_owned(),
            });
        }
        let mut root_key_ids = BTreeSet::new();
        for (index, signature) in self.root_signatures.iter().enumerate() {
            signature.validate(&format!("root_signatures[{index}]"))?;
            if !root_key_ids.insert(signature.key_id.clone()) {
                return Err(RegistryProtocolV1Error::DuplicateValue {
                    field: "root_signatures.key_id".to_owned(),
                    value: signature.key_id.clone(),
                });
            }
        }
        Ok(())
    }

    fn normalize_in_place(&mut self) {
        self.protocol_versions.sort();
        self.auth.sort_by_key(|descriptor| descriptor.mode);
        self.signing_keys
            .sort_by(|left, right| left.key_id.cmp(&right.key_id));
        self.accepted_digest_algorithms.sort();
        self.accepted_archive_formats.sort();
        self.root_signatures.sort();
    }

    /// Canonical payload verified against locally enrolled recovery/root keys.
    /// Root signatures themselves are excluded from these bytes.
    pub fn signing_payload_bytes(&self) -> Result<Vec<u8>, RegistryProtocolV1Error> {
        self.validate_payload_fields()?;
        let mut payload = self.clone();
        payload.root_signatures.clear();
        payload.normalize_in_place();
        canonical_json_bytes(&serde_json::to_value(payload)?)
    }

    /// Canonical signed discovery bytes. Their SHA-256 forms the discovery
    /// predecessor link for the next version.
    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>, RegistryProtocolV1Error> {
        self.validate()?;
        let mut normalized = self.clone();
        normalized.normalize_in_place();
        canonical_json_bytes(&serde_json::to_value(normalized)?)
    }

    /// Validates the metadata relationship before cryptographic signature
    /// verification by the client implementation.
    pub fn authorize_current_checkpoint_metadata(
        &self,
        checkpoint: &RegistryCheckpointV1,
    ) -> Result<(), RegistryProtocolV1Error> {
        self.validate()?;
        checkpoint.validate()?;
        if self.registry_id != checkpoint.registry_id {
            return Err(RegistryProtocolV1Error::InvalidRelationship {
                message: "discovery and checkpoint registry_id must match".to_owned(),
            });
        }
        let authorized = self.signing_keys.iter().any(|key| {
            key.state == RegistrySigningKeyStateV1::Active
                && key.key_id == checkpoint.signing_key_id
        });
        if !authorized {
            return Err(RegistryProtocolV1Error::InvalidRelationship {
                message: "checkpoint signing key is not active in accepted discovery".to_owned(),
            });
        }
        Ok(())
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
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
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
        canonical_json_bytes(&serde_json::to_value(&normalized)?)
    }
}

/// One immutable sparse-index object authenticated by a snapshot manifest.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistryIndexSnapshotEntryV1 {
    pub path: String,
    pub sha256: String,
    pub size: u64,
}

impl RegistryIndexSnapshotEntryV1 {
    fn validate(&self, field: &str) -> Result<(), RegistryProtocolV1Error> {
        validate_safe_relative_path(&format!("{field}.path"), &self.path)?;
        let mut parts = self.path.split('/');
        match (parts.next(), parts.next(), parts.next(), parts.next()) {
            (Some("index"), Some(org), Some(name), None) => {
                validate_coordinate_component(&format!("{field}.path.org"), org)?;
                validate_coordinate_component(&format!("{field}.path.name"), name)?;
            }
            _ => {
                return Err(RegistryProtocolV1Error::InvalidValue {
                    field: format!("{field}.path"),
                    value: self.path.clone(),
                });
            }
        }
        validate_sha256(&format!("{field}.sha256"), &self.sha256)?;
        if self.size == 0 {
            return Err(RegistryProtocolV1Error::ZeroValue {
                field: format!("{field}.size"),
            });
        }
        Ok(())
    }
}

/// Canonical manifest for every per-package index in one immutable snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistryIndexSnapshotV1 {
    pub schema: String,
    pub registry_id: String,
    pub sequence: u64,
    pub entries: Vec<RegistryIndexSnapshotEntryV1>,
}

impl RegistryIndexSnapshotV1 {
    pub const SCHEMA_V1: &'static str = REGISTRY_INDEX_SNAPSHOT_SCHEMA_V1;

    pub fn validate(&self) -> Result<(), RegistryProtocolV1Error> {
        validate_schema("schema", &self.schema, Self::SCHEMA_V1)?;
        validate_registry_id(&self.registry_id)?;
        if self.sequence == 0 {
            return Err(RegistryProtocolV1Error::ZeroValue {
                field: "sequence".to_owned(),
            });
        }
        if self.entries.is_empty() {
            return Err(RegistryProtocolV1Error::MissingField {
                field: "entries".to_owned(),
            });
        }

        let mut previous: Option<&str> = None;
        for (index, entry) in self.entries.iter().enumerate() {
            entry.validate(&format!("entries[{index}]"))?;
            if previous.is_some_and(|path| path >= entry.path.as_str()) {
                return Err(RegistryProtocolV1Error::NonCanonicalOrder {
                    field: "entries.path".to_owned(),
                });
            }
            previous = Some(&entry.path);
        }
        Ok(())
    }

    /// Canonical bytes whose SHA-256 is selected by a signed checkpoint.
    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>, RegistryProtocolV1Error> {
        self.validate()?;
        canonical_json_bytes(&serde_json::to_value(self)?)
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
    /// SHA-256 of canonical `RegistryIndexSnapshotV1` bytes. The same
    /// lowercase digest is substituted into the `{snapshot}` endpoint token.
    pub index_root_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_checkpoint_sha256: Option<String>,
    pub signing_key_id: String,
    pub signature: String,
}

impl RegistryCheckpointV1 {
    pub const SCHEMA_V1: &'static str = REGISTRY_CHECKPOINT_SCHEMA_V1;

    fn validate_payload_fields(&self) -> Result<(), RegistryProtocolV1Error> {
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
        match (self.sequence, self.previous_checkpoint_sha256.as_deref()) {
            (1, None) => {}
            (1, Some(_)) => {
                return Err(RegistryProtocolV1Error::UnexpectedField {
                    field: "previous_checkpoint_sha256".to_owned(),
                });
            }
            (_, Some(previous)) => {
                validate_sha256("previous_checkpoint_sha256", previous)?;
            }
            (_, None) => {
                return Err(RegistryProtocolV1Error::MissingField {
                    field: "previous_checkpoint_sha256".to_owned(),
                });
            }
        }
        validate_lower_token("signing_key_id", &self.signing_key_id)
    }

    pub fn validate(&self) -> Result<(), RegistryProtocolV1Error> {
        self.validate_payload_fields()?;
        validate_signature("signature", &self.signature)
    }

    /// Immutable snapshot token selected by this checkpoint.
    #[must_use]
    pub fn snapshot_id(&self) -> &str {
        &self.index_root_sha256
    }

    /// Canonical bytes covered by `signature`. The signature itself is
    /// excluded, and a publisher can obtain these bytes before a signature
    /// exists.
    pub fn signing_payload_bytes(&self) -> Result<Vec<u8>, RegistryProtocolV1Error> {
        self.validate_payload_fields()?;
        #[derive(Serialize)]
        struct Payload<'a> {
            schema: &'a str,
            registry_id: &'a str,
            sequence: u64,
            generated_at: &'a str,
            expires_at: &'a str,
            index_root_sha256: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            previous_checkpoint_sha256: &'a Option<String>,
            signing_key_id: &'a str,
        }
        canonical_json_bytes(&serde_json::to_value(Payload {
            schema: &self.schema,
            registry_id: &self.registry_id,
            sequence: self.sequence,
            generated_at: &self.generated_at,
            expires_at: &self.expires_at,
            index_root_sha256: &self.index_root_sha256,
            previous_checkpoint_sha256: &self.previous_checkpoint_sha256,
            signing_key_id: &self.signing_key_id,
        })?)
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
        canonical_json_bytes(&serde_json::to_value(self)?)
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
    #[error("canonical registry JSON forbids non-integer number: {0}")]
    UnsupportedJsonNumber(String),
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
        || value.contains('?')
        || value.contains('#')
        || value.contains('%')
        || value
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
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
    let mut remainder = value.to_owned();
    for token in required_tokens {
        match value.matches(token).count() {
            0 => {
                return Err(RegistryProtocolV1Error::MissingField {
                    field: format!("{field}:{token}"),
                });
            }
            1 => {
                remainder = remainder.replacen(token, "", 1);
            }
            _ => {
                return Err(RegistryProtocolV1Error::InvalidValue {
                    field: field.to_owned(),
                    value: value.to_owned(),
                });
            }
        }
    }
    if remainder.contains('{') || remainder.contains('}') {
        return Err(RegistryProtocolV1Error::InvalidValue {
            field: field.to_owned(),
            value: value.to_owned(),
        });
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
        return Err(RegistryProtocolV1Error::InvalidValue {
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
        return Err(RegistryProtocolV1Error::InvalidValue {
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

fn canonical_json_bytes(value: &Value) -> Result<Vec<u8>, RegistryProtocolV1Error> {
    let mut bytes = Vec::new();
    write_canonical_json(value, &mut bytes)?;
    Ok(bytes)
}

fn write_canonical_json(value: &Value, bytes: &mut Vec<u8>) -> Result<(), RegistryProtocolV1Error> {
    match value {
        Value::Null => bytes.extend_from_slice(b"null"),
        Value::Bool(value) => bytes.extend_from_slice(if *value { b"true" } else { b"false" }),
        Value::Number(number) => {
            if !number.is_i64() && !number.is_u64() {
                return Err(RegistryProtocolV1Error::UnsupportedJsonNumber(
                    number.to_string(),
                ));
            }
            bytes.extend_from_slice(number.to_string().as_bytes());
        }
        Value::String(value) => bytes.extend_from_slice(serde_json::to_string(value)?.as_bytes()),
        Value::Array(values) => {
            bytes.push(b'[');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    bytes.push(b',');
                }
                write_canonical_json(value, bytes)?;
            }
            bytes.push(b']');
        }
        Value::Object(values) => {
            bytes.push(b'{');
            let mut keys: Vec<_> = values.keys().collect();
            keys.sort_unstable();
            for (index, key) in keys.into_iter().enumerate() {
                if index != 0 {
                    bytes.push(b',');
                }
                bytes.extend_from_slice(serde_json::to_string(key)?.as_bytes());
                bytes.push(b':');
                write_canonical_json(&values[key], bytes)?;
            }
            bytes.push(b'}');
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
            version: 1,
            generated_at: "2026-08-07T00:00:00Z".to_owned(),
            expires_at: "2026-08-08T00:00:00Z".to_owned(),
            previous_discovery_sha256: None,
            registry_id: REGISTRY_ID.to_owned(),
            canonical_url: "https://registry.example.test".to_owned(),
            protocol_versions: vec![REGISTRY_PROTOCOL_V1.to_owned()],
            endpoints: RegistryEndpointsV1 {
                sparse_index_template: "/snapshots/{snapshot}/index/{org}/{name}".to_owned(),
                snapshot_manifest_template: "/snapshots/{snapshot}/manifest.json".to_owned(),
                package_template: "/pkgs/{org}/{name}/{version}.tar.zst".to_owned(),
                archive_manifest_template: "/pkgs/{org}/{name}/{version}.manifest.json".to_owned(),
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
            auth: vec![
                RegistryAuthDescriptorV1 {
                    mode: RegistryAuthModeV1::AnonymousRead,
                    issuer: None,
                    audience: None,
                },
                RegistryAuthDescriptorV1 {
                    mode: RegistryAuthModeV1::OidcPkce,
                    issuer: Some("https://auth.example.test".to_owned()),
                    audience: Some("registry.example.test".to_owned()),
                },
            ],
            signing_keys: vec![RegistrySigningKeyV1 {
                key_id: "metadata-2026-01".to_owned(),
                algorithm: "ed25519".to_owned(),
                public_key_multibase: "z6MkrJVnaZkeFzdQ".to_owned(),
                state: RegistrySigningKeyStateV1::Active,
            }],
            accepted_digest_algorithms: vec!["sha256".to_owned()],
            accepted_archive_formats: vec![RegistryArchiveFormatV1::TarZstd],
            limits: RegistryLimitsV1 {
                max_archive_bytes: 100 * 1024 * 1024,
                max_expanded_bytes: 500 * 1024 * 1024,
                max_files: 100_000,
                max_path_bytes: 1024,
                max_compression_ratio: 100,
            },
            root_signatures: vec![RegistryRootSignatureV1 {
                key_id: "root-2026-01".to_owned(),
                algorithm: "ed25519".to_owned(),
                signature: "AbCdEfGhIjKlMnOpQrStUvWxYz_01234".to_owned(),
            }],
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

    fn index_snapshot() -> RegistryIndexSnapshotV1 {
        RegistryIndexSnapshotV1 {
            schema: REGISTRY_INDEX_SNAPSHOT_SCHEMA_V1.to_owned(),
            registry_id: REGISTRY_ID.to_owned(),
            sequence: 7,
            entries: vec![
                RegistryIndexSnapshotEntryV1 {
                    path: "index/acme/shared".to_owned(),
                    sha256: SHA_A.to_owned(),
                    size: 21,
                },
                RegistryIndexSnapshotEntryV1 {
                    path: "index/acme/widget".to_owned(),
                    sha256: SHA_B.to_owned(),
                    size: 42,
                },
            ],
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
        index_snapshot().validate().expect("snapshot validates");
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
    fn static_read_templates_require_the_immutable_snapshot_token() {
        let mut sparse_discovery = discovery();
        sparse_discovery.endpoints.sparse_index_template = "/index/{org}/{name}".to_owned();
        assert!(sparse_discovery.validate().is_err());

        let mut snapshot_discovery = discovery();
        snapshot_discovery.endpoints.snapshot_manifest_template = "/manifest.json".to_owned();
        assert!(snapshot_discovery.validate().is_err());

        let mut archive_discovery = discovery();
        archive_discovery.endpoints.archive_manifest_template = "/manifest.json".to_owned();
        assert!(archive_discovery.validate().is_err());
    }

    #[test]
    fn snapshot_manifest_rejects_non_index_paths_and_noncanonical_order() {
        let mut snapshot = index_snapshot();
        snapshot.entries[0].path = "packages/acme/shared".to_owned();
        assert!(snapshot.validate().is_err());

        let mut snapshot = index_snapshot();
        snapshot.entries.reverse();
        assert!(snapshot.validate().is_err());

        let mut snapshot = index_snapshot();
        snapshot.entries[1].path = snapshot.entries[0].path.clone();
        assert!(snapshot.validate().is_err());
    }

    #[test]
    fn discovery_is_root_signed_versioned_and_capability_consistent() {
        let discovery = discovery();
        let payload = discovery
            .signing_payload_bytes()
            .expect("unsigned discovery payload canonicalizes");
        let payload_text = String::from_utf8(payload).expect("canonical JSON is UTF-8");
        assert!(!payload_text.contains("root_signatures"));
        discovery.validate().expect("signed discovery validates");

        let mut unsigned = discovery.clone();
        unsigned.root_signatures.clear();
        assert!(unsigned.validate().is_err());
        unsigned
            .signing_payload_bytes()
            .expect("publisher can canonicalize before signing");

        let mut unchained = discovery.clone();
        unchained.version = 2;
        assert!(unchained.validate().is_err());

        let mut inconsistent = discovery.clone();
        inconsistent.capabilities.publish = false;
        assert!(inconsistent.validate().is_err());

        let mut no_anonymous = discovery.clone();
        no_anonymous
            .auth
            .retain(|descriptor| descriptor.mode != RegistryAuthModeV1::AnonymousRead);
        assert!(no_anonymous.validate().is_err());
    }

    #[test]
    fn checkpoint_genesis_timestamp_and_signing_payload_are_strict() {
        let mut checkpoint: RegistryCheckpointV1 =
            serde_json::from_str(include_str!("../fixtures/registry-v1/checkpoint.json"))
                .expect("checkpoint fixture parses");
        checkpoint.validate().expect("checkpoint validates");

        checkpoint.signature.clear();
        checkpoint
            .signing_payload_bytes()
            .expect("publisher can canonicalize before signing");
        assert!(checkpoint.validate().is_err());

        checkpoint.signature = "AbCdEfGhIjKlMnOpQrStUvWxYz_01234".to_owned();
        checkpoint.previous_checkpoint_sha256 = Some(SHA_A.to_owned());
        assert!(checkpoint.validate().is_err());

        checkpoint.previous_checkpoint_sha256 = None;
        checkpoint.generated_at = "2026-8-07T00:00:00Z".to_owned();
        assert!(checkpoint.validate().is_err());
    }

    #[test]
    fn golden_fixture_chain_is_digest_consistent() {
        use sha2::{Digest as _, Sha256};

        let snapshot: RegistryIndexSnapshotV1 =
            serde_json::from_str(include_str!("../fixtures/registry-v1/index-snapshot.json"))
                .expect("snapshot fixture parses");
        let checkpoint: RegistryCheckpointV1 =
            serde_json::from_str(include_str!("../fixtures/registry-v1/checkpoint.json"))
                .expect("checkpoint fixture parses");
        let snapshot_digest = hex::encode(Sha256::digest(
            snapshot
                .canonical_json_bytes()
                .expect("snapshot canonicalizes"),
        ));
        assert_eq!(checkpoint.index_root_sha256, snapshot_digest);

        let widget_bytes = include_bytes!("../fixtures/registry-v1/snapshot/index/acme/widget");
        let widget_entry = snapshot
            .entries
            .iter()
            .find(|entry| entry.path == "index/acme/widget")
            .expect("widget index entry exists");
        assert_eq!(widget_entry.size, widget_bytes.len() as u64);
        assert_eq!(
            widget_entry.sha256,
            hex::encode(Sha256::digest(widget_bytes))
        );

        let archive: RegistryArchiveManifestV1 = serde_json::from_str(include_str!(
            "../fixtures/registry-v1/archive-manifest.json"
        ))
        .expect("archive fixture parses");
        let archive_manifest_digest = hex::encode(Sha256::digest(
            archive
                .canonical_json_bytes()
                .expect("archive manifest canonicalizes"),
        ));
        let first_record: RegistryIndexRecordV1 = serde_json::from_str(
            include_str!("../fixtures/registry-v1/index.ndjson")
                .lines()
                .next()
                .expect("index fixture has a record"),
        )
        .expect("first index record parses");
        assert_eq!(
            first_record.archive.manifest_sha256,
            archive_manifest_digest
        );
        assert_eq!(first_record.archive.sha256, archive.archive_sha256);
        assert_eq!(first_record.checkpoint_sequence, checkpoint.sequence);
        discovery()
            .authorize_current_checkpoint_metadata(&checkpoint)
            .expect("accepted discovery authorizes current checkpoint metadata");
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
        assert_eq!(checkpoint.snapshot_id(), SHA_A);
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

        let snapshot: RegistryIndexSnapshotV1 =
            serde_json::from_str(include_str!("../fixtures/registry-v1/index-snapshot.json"))
                .expect("snapshot fixture parses");
        snapshot.validate().expect("snapshot fixture validates");

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
