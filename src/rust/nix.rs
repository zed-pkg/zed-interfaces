use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::artifact::ArtifactFormat;
use crate::manifest::{is_sha256_hex, is_slug, is_target_name};

/// Major-versioned identifier for the first immutable Nix ↔ Zed adapter
/// contract. Unknown major versions must fail closed.
pub const NIX_ADAPTER_SCHEMA_V1: &str = "zed.nix-adapter/v1";

/// Store-object JSON versions accepted by the v1 contract. The CLI still
/// records the concrete Nix version because these new-CLI formats are
/// versioned independently of this crate.
pub const SUPPORTED_STORE_INFO_JSON_VERSIONS: &[u32] = &[1, 2, 3];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum NixExportMode {
    /// Export the exact immutable Zed artifact. Source-builder translation is
    /// intentionally not inferred from native manifests in contract v1.
    #[default]
    Artifact,
}

/// Author intent for exporting a package or target to Nix.
///
/// This structure contains no realized store paths, hashes, commands,
/// credentials, cache keys, or service deployment policy. Those belong in a
/// versioned adapter record after planning/realization.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct NixExportSection {
    pub mode: NixExportMode,
    /// Optional Nix package attribute. Omit it to use the Zed package name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attribute: Option<String>,
    /// Explicit Nix systems this package claims to support.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub systems: Vec<String>,
    /// Explicit derivation outputs. Contract v1 never silently selects the
    /// first output of a multi-output derivation.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<String>,
}

impl NixExportSection {
    pub fn resolved_attribute(&self, default_attribute: &str) -> String {
        self.attribute
            .clone()
            .unwrap_or_else(|| default_attribute.to_string())
    }

    pub fn validate(&self, default_attribute: &str) -> Result<(), NixInteropError> {
        let attribute = self.resolved_attribute(default_attribute);
        if !is_nix_identifier(&attribute) {
            return Err(NixInteropError::InvalidExportIntent(format!(
                "attribute `{attribute}` must be a Nix identifier"
            )));
        }
        if attribute == "default" {
            return Err(NixInteropError::InvalidExportIntent(
                "attribute `default` is reserved for the generated flake alias; set an explicit non-default attribute"
                    .to_string(),
            ));
        }
        if self.systems.is_empty() {
            return Err(NixInteropError::InvalidExportIntent(
                "at least one explicit Nix system is required".to_string(),
            ));
        }
        if self.outputs.is_empty() {
            return Err(NixInteropError::InvalidExportIntent(
                "at least one explicit Nix output is required (usually `out`)".to_string(),
            ));
        }
        ensure_unique(&self.systems, "Nix systems")?;
        ensure_unique(&self.outputs, "Nix outputs")?;
        for system in &self.systems {
            if !is_nix_system(system) {
                return Err(NixInteropError::InvalidExportIntent(format!(
                    "system `{system}` must be a lowercase Nix system such as `x86_64-linux`"
                )));
            }
        }
        for output in &self.outputs {
            if !is_nix_identifier(output) {
                return Err(NixInteropError::InvalidExportIntent(format!(
                    "output `{output}` must be a Nix identifier"
                )));
            }
        }
        Ok(())
    }

    fn normalize(&mut self) {
        self.systems.sort();
        self.outputs.sort();
    }
}

/// Public Zed identity chosen for either translation direction. A Nix
/// attribute is a selector and never silently claims a Zed organization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NixPackageIdentity {
    pub org: String,
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}

impl NixPackageIdentity {
    pub fn validate(&self) -> Result<(), NixInteropError> {
        if !is_slug(&self.org) {
            return Err(NixInteropError::InvalidPackageIdentity(format!(
                "invalid org slug `{}`",
                self.org
            )));
        }
        if !is_slug(&self.name) {
            return Err(NixInteropError::InvalidPackageIdentity(format!(
                "invalid package name `{}`",
                self.name
            )));
        }
        if self.version.is_empty()
            || self.version.trim() != self.version
            || self.version.chars().any(char::is_whitespace)
        {
            return Err(NixInteropError::InvalidPackageIdentity(
                "version must be non-empty and contain no whitespace".to_string(),
            ));
        }
        if let Some(target) = &self.target
            && !is_target_name(target)
        {
            return Err(NixInteropError::InvalidPackageIdentity(format!(
                "invalid target `{target}`"
            )));
        }
        Ok(())
    }
}

/// Immutable Zed artifact identity used by both a Zed-origin source and the
/// translated artifact produced by Nix → Zed sealing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NixInteropArtifact {
    #[serde(default)]
    pub format: ArtifactFormat,
    /// Lowercase hexadecimal SHA-256 of the exact archive bytes.
    pub sha256: String,
    pub size: u64,
}

impl NixInteropArtifact {
    pub fn validate(&self, field: &str) -> Result<(), NixInteropError> {
        validate_sha256_hex(field, &self.sha256)?;
        if self.size == 0 {
            return Err(NixInteropError::InvalidArtifact(format!(
                "{field} size must be greater than zero"
            )));
        }
        Ok(())
    }
}

/// Immutable source evidence when Zed is the dependency-resolution authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ZedArtifactOrigin {
    /// Registry base URL used to resolve the exact artifact.
    pub registry: String,
    pub artifact: NixInteropArtifact,
    pub vcs_tag: String,
    pub vcs_commit: String,
    /// Hash of the exact `.zpkg.lock` bytes, omitted only for a dependency-free
    /// package whose export plan proves no lock is required.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock_sha256: Option<String>,
}

impl ZedArtifactOrigin {
    pub fn validate(&self) -> Result<(), NixInteropError> {
        if !is_registry_url(&self.registry) {
            return Err(NixInteropError::InvalidZedOrigin(
                "registry must be an http(s) or file URL without whitespace".to_string(),
            ));
        }
        self.artifact.validate("Zed source artifact")?;
        if !is_ref_token(&self.vcs_tag) {
            return Err(NixInteropError::InvalidZedOrigin(
                "VCS tag must be non-empty and contain no whitespace".to_string(),
            ));
        }
        if self.vcs_commit.len() < 7 || !is_ref_token(&self.vcs_commit) {
            return Err(NixInteropError::InvalidZedOrigin(
                "VCS commit must be an immutable non-whitespace identifier".to_string(),
            ));
        }
        if let Some(lock_sha256) = &self.lock_sha256 {
            validate_sha256_hex("Zed lock", lock_sha256)?;
        }
        Ok(())
    }
}

/// One referenced Nix store object. Portable Nix → Zed imports reject any
/// runtime references in contract v1; Zed → Nix output attestations may still
/// retain them as evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NixStoreReference {
    pub store_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nar_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nar_size: Option<u64>,
}

impl NixStoreReference {
    pub fn validate(&self) -> Result<(), NixInteropError> {
        if !is_nix_store_path(&self.store_path) {
            return Err(NixInteropError::InvalidNixOutput(format!(
                "invalid referenced store path `{}`",
                self.store_path
            )));
        }
        if let Some(nar_hash) = &self.nar_hash
            && !is_sha256_sri(nar_hash)
        {
            return Err(NixInteropError::InvalidNixOutput(format!(
                "reference `{}` has an invalid SHA-256 SRI NAR hash",
                self.store_path
            )));
        }
        if self.nar_size == Some(0) {
            return Err(NixInteropError::InvalidNixOutput(format!(
                "reference `{}` has zero NAR size",
                self.store_path
            )));
        }
        Ok(())
    }
}

/// Realization evidence for exactly one Nix system/output pair.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NixRealizedOutput {
    pub system: String,
    pub output: String,
    pub derivation_json_sha256: String,
    /// Diagnostic path only; `nar_hash` is the portable output identity.
    pub store_path: String,
    pub nar_hash: String,
    pub nar_size: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references: Vec<NixStoreReference>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signatures: Vec<String>,
    pub nix_version: String,
    pub store_info_json_version: u32,
}

impl NixRealizedOutput {
    pub fn validate(&self) -> Result<(), NixInteropError> {
        if !is_nix_system(&self.system) {
            return Err(NixInteropError::InvalidNixOutput(format!(
                "invalid Nix system `{}`",
                self.system
            )));
        }
        if !is_nix_identifier(&self.output) {
            return Err(NixInteropError::InvalidNixOutput(format!(
                "invalid Nix output `{}`",
                self.output
            )));
        }
        validate_sha256_hex("derivation JSON", &self.derivation_json_sha256)?;
        if !is_nix_store_path(&self.store_path) {
            return Err(NixInteropError::InvalidNixOutput(format!(
                "invalid store path `{}`",
                self.store_path
            )));
        }
        if !is_sha256_sri(&self.nar_hash) {
            return Err(NixInteropError::InvalidNixOutput(
                "NAR hash must be a SHA-256 SRI value".to_string(),
            ));
        }
        if self.nar_size == 0 {
            return Err(NixInteropError::InvalidNixOutput(
                "NAR size must be greater than zero".to_string(),
            ));
        }
        let mut reference_paths = BTreeSet::new();
        for reference in &self.references {
            reference.validate()?;
            if !reference_paths.insert(reference.store_path.as_str()) {
                return Err(NixInteropError::InvalidNixOutput(format!(
                    "store reference `{}` appears more than once",
                    reference.store_path
                )));
            }
        }
        ensure_unique(&self.signatures, "Nix signatures")?;
        if self
            .signatures
            .iter()
            .any(|signature| signature.is_empty() || signature.chars().any(char::is_whitespace))
        {
            return Err(NixInteropError::InvalidNixOutput(
                "Nix signatures must be non-empty tokens without whitespace".to_string(),
            ));
        }
        if self.nix_version.trim().is_empty() {
            return Err(NixInteropError::InvalidNixOutput(
                "Nix version must be recorded".to_string(),
            ));
        }
        if !SUPPORTED_STORE_INFO_JSON_VERSIONS.contains(&self.store_info_json_version) {
            return Err(NixInteropError::InvalidNixOutput(format!(
                "unsupported store-info JSON version {}; supported versions are 1, 2, and 3",
                self.store_info_json_version
            )));
        }
        Ok(())
    }

    fn normalize(&mut self) {
        self.references
            .sort_by(|a, b| a.store_path.cmp(&b.store_path));
        self.signatures.sort();
    }
}

/// Immutable Nix source selector plus the exact realized output selected for a
/// Nix → Zed translation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NixOutputOrigin {
    pub locked_ref: String,
    pub flake_lock_sha256: String,
    /// Standard flake attribute path, e.g. `packages.x86_64-linux.tool`.
    pub attribute: String,
    pub realized: NixRealizedOutput,
}

impl NixOutputOrigin {
    pub fn validate(&self) -> Result<(), NixInteropError> {
        if !is_immutable_nix_ref(&self.locked_ref) {
            return Err(NixInteropError::InvalidNixOrigin(
                "locked ref must contain immutable revision or NAR-hash evidence and no whitespace"
                    .to_string(),
            ));
        }
        validate_sha256_hex("flake.lock", &self.flake_lock_sha256)?;
        if !is_nix_attribute_path(&self.attribute) {
            return Err(NixInteropError::InvalidNixOrigin(format!(
                "invalid standard flake attribute path `{}`",
                self.attribute
            )));
        }
        self.realized.validate()
    }

    fn normalize(&mut self) {
        self.realized.normalize();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum NixPolicyProfile {
    StrictV1,
    Development,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum NixBuilderNetwork {
    Disabled,
    PreparationOnly,
    Allowed,
}

/// Evidence that the translation was planned/realized under a named policy.
/// This records policy state; it does not grant credentials or execute Nix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NixPolicyEvidence {
    pub profile: NixPolicyProfile,
    pub pure_evaluation: bool,
    pub import_from_derivation: bool,
    pub sandbox_required: bool,
    pub builder_network: NixBuilderNetwork,
    pub dirty_source: bool,
    pub publishable: bool,
}

impl NixPolicyEvidence {
    pub fn validate(&self) -> Result<(), NixInteropError> {
        match self.profile {
            NixPolicyProfile::StrictV1 => {
                if !self.pure_evaluation
                    || self.import_from_derivation
                    || !self.sandbox_required
                    || self.builder_network != NixBuilderNetwork::Disabled
                    || self.dirty_source
                    || !self.publishable
                {
                    return Err(NixInteropError::InvalidPolicy(
                        "strict-v1 requires pure evaluation, IFD disabled, sandbox required, builder network disabled, clean source, and publishable output"
                            .to_string(),
                    ));
                }
            }
            NixPolicyProfile::Development => {
                if self.publishable {
                    return Err(NixInteropError::InvalidPolicy(
                        "development policy records are never publishable".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Final, immutable provenance record for one completed translation.
///
/// `direction` is internally tagged in JSON so consumers cannot deserialize a
/// direction whose required origin/result fields are absent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "direction", rename_all = "kebab-case")]
pub enum NixAdapterRecord {
    ZedToNix {
        schema: String,
        package: NixPackageIdentity,
        source: ZedArtifactOrigin,
        intent: NixExportSection,
        /// Hash of a canonical inventory of the generated standalone flake
        /// bundle, not a filesystem-dependent archive of the output directory.
        flake_bundle_sha256: String,
        outputs: Vec<NixRealizedOutput>,
        policy: NixPolicyEvidence,
    },
    NixToZed {
        schema: String,
        package: NixPackageIdentity,
        source: NixOutputOrigin,
        artifact: NixInteropArtifact,
        policy: NixPolicyEvidence,
    },
}

impl NixAdapterRecord {
    pub fn zed_to_nix(
        package: NixPackageIdentity,
        source: ZedArtifactOrigin,
        intent: NixExportSection,
        flake_bundle_sha256: String,
        outputs: Vec<NixRealizedOutput>,
        policy: NixPolicyEvidence,
    ) -> Self {
        Self::ZedToNix {
            schema: NIX_ADAPTER_SCHEMA_V1.to_string(),
            package,
            source,
            intent,
            flake_bundle_sha256,
            outputs,
            policy,
        }
    }

    pub fn nix_to_zed(
        package: NixPackageIdentity,
        source: NixOutputOrigin,
        artifact: NixInteropArtifact,
        policy: NixPolicyEvidence,
    ) -> Self {
        Self::NixToZed {
            schema: NIX_ADAPTER_SCHEMA_V1.to_string(),
            package,
            source,
            artifact,
            policy,
        }
    }

    pub fn validate(&self) -> Result<(), NixInteropError> {
        match self {
            Self::ZedToNix {
                schema,
                package,
                source,
                intent,
                flake_bundle_sha256,
                outputs,
                policy,
            } => {
                validate_schema(schema)?;
                package.validate()?;
                source.validate()?;
                intent.validate(&package.name)?;
                validate_sha256_hex("flake bundle", flake_bundle_sha256)?;
                policy.validate()?;
                if outputs.is_empty() {
                    return Err(NixInteropError::InvalidAdapter(
                        "a final Zed → Nix adapter record requires at least one realized output"
                            .to_string(),
                    ));
                }
                let declared_systems: BTreeSet<&str> =
                    intent.systems.iter().map(String::as_str).collect();
                let declared_outputs: BTreeSet<&str> =
                    intent.outputs.iter().map(String::as_str).collect();
                let mut seen = BTreeSet::new();
                let mut realized_systems = BTreeSet::new();
                for output in outputs {
                    output.validate()?;
                    if !declared_systems.contains(output.system.as_str()) {
                        return Err(NixInteropError::InvalidAdapter(format!(
                            "realized system `{}` was not declared by the export intent",
                            output.system
                        )));
                    }
                    if !declared_outputs.contains(output.output.as_str()) {
                        return Err(NixInteropError::InvalidAdapter(format!(
                            "realized output `{}` was not declared by the export intent",
                            output.output
                        )));
                    }
                    if !seen.insert((output.system.as_str(), output.output.as_str())) {
                        return Err(NixInteropError::InvalidAdapter(format!(
                            "system/output pair `{}/{}` appears more than once",
                            output.system, output.output
                        )));
                    }
                    realized_systems.insert(output.system.as_str());
                }
                for system in declared_systems {
                    if !realized_systems.contains(system) {
                        return Err(NixInteropError::InvalidAdapter(format!(
                            "declared system `{system}` has no realized output evidence"
                        )));
                    }
                }
                Ok(())
            }
            Self::NixToZed {
                schema,
                package,
                source,
                artifact,
                policy,
            } => {
                validate_schema(schema)?;
                package.validate()?;
                source.validate()?;
                artifact.validate("translated Zed artifact")?;
                policy.validate()?;
                if !source.realized.references.is_empty() {
                    return Err(NixInteropError::InvalidAdapter(
                        "contract v1 Nix → Zed imports must be closure-free; runtime store references are not portable"
                            .to_string(),
                    ));
                }
                Ok(())
            }
        }
    }

    /// Deterministic compact JSON bytes for hashing/signing. Validation occurs
    /// before normalization, so duplicate unordered values cannot be silently
    /// collapsed. Arrays whose order is not semantic are then sorted, and all
    /// object keys are emitted lexicographically.
    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>, NixInteropError> {
        self.validate()?;
        let mut normalized = self.clone();
        normalized.normalize();
        let value = serde_json::to_value(normalized)
            .map_err(|error| NixInteropError::Json(error.to_string()))?;
        serde_json::to_vec(&canonicalize_json(value))
            .map_err(|error| NixInteropError::Json(error.to_string()))
    }

    pub fn canonical_json_string(&self) -> Result<String, NixInteropError> {
        String::from_utf8(self.canonical_json_bytes()?)
            .map_err(|error| NixInteropError::Json(error.to_string()))
    }

    fn normalize(&mut self) {
        match self {
            Self::ZedToNix {
                intent, outputs, ..
            } => {
                intent.normalize();
                for output in outputs.iter_mut() {
                    output.normalize();
                }
                outputs.sort_by(|a, b| {
                    (&a.system, &a.output, &a.store_path).cmp(&(
                        &b.system,
                        &b.output,
                        &b.store_path,
                    ))
                });
            }
            Self::NixToZed { source, .. } => source.normalize(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum NixInteropError {
    #[error("unsupported Nix adapter schema `{0}`")]
    UnsupportedSchema(String),
    #[error("invalid Nix export intent: {0}")]
    InvalidExportIntent(String),
    #[error("invalid Nix interop package identity: {0}")]
    InvalidPackageIdentity(String),
    #[error("invalid Nix interop artifact: {0}")]
    InvalidArtifact(String),
    #[error("invalid Zed origin: {0}")]
    InvalidZedOrigin(String),
    #[error("invalid Nix origin: {0}")]
    InvalidNixOrigin(String),
    #[error("invalid Nix output evidence: {0}")]
    InvalidNixOutput(String),
    #[error("invalid Nix interop policy: {0}")]
    InvalidPolicy(String),
    #[error("invalid Nix adapter record: {0}")]
    InvalidAdapter(String),
    #[error("Nix adapter JSON error: {0}")]
    Json(String),
}

pub fn is_nix_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '\''))
}

pub fn is_nix_attribute_path(value: &str) -> bool {
    !value.is_empty() && value.split('.').all(is_nix_identifier)
}

pub fn is_nix_system(value: &str) -> bool {
    !value.is_empty()
        && value.contains('-')
        && !value.starts_with('-')
        && !value.ends_with('-')
        && value
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-'))
}

pub fn is_nix_store_path(value: &str) -> bool {
    let Some(rest) = value.strip_prefix("/nix/store/") else {
        return false;
    };
    if rest.len() < 34 || rest.as_bytes().get(32) != Some(&b'-') {
        return false;
    }
    const NIX_BASE32: &str = "0123456789abcdfghijklmnpqrsvwxyz";
    rest[..32].chars().all(|c| NIX_BASE32.contains(c))
        && rest[33..]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.' | '_' | '?'))
}

pub fn is_sha256_sri(value: &str) -> bool {
    let Some(payload) = value.strip_prefix("sha256-") else {
        return false;
    };
    payload.len() == 44
        && payload.ends_with('=')
        && payload[..43]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/'))
}

pub fn is_immutable_nix_ref(value: &str) -> bool {
    if value.is_empty()
        || value.trim() != value
        || value.chars().any(char::is_whitespace)
        || value.contains("<")
        || value.contains('>')
    {
        return false;
    }
    if value.starts_with("/nix/store/") || value.starts_with("path:/nix/store/") {
        return true;
    }
    if value.contains("narHash=sha256-") {
        return true;
    }
    if let Some(rev) = query_value(value, "rev")
        && is_hex_revision(rev)
    {
        return true;
    }
    value
        .split(|c: char| !c.is_ascii_hexdigit())
        .any(is_hex_revision)
}

fn is_hex_revision(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.chars().all(|c| c.is_ascii_hexdigit())
}

fn query_value<'a>(value: &'a str, name: &str) -> Option<&'a str> {
    value
        .split(['?', '&'])
        .find_map(|part| part.strip_prefix(&format!("{name}=")))
}

fn is_registry_url(value: &str) -> bool {
    value.trim() == value
        && !value.chars().any(char::is_whitespace)
        && ["https://", "http://", "file://"]
            .iter()
            .any(|prefix| value.starts_with(prefix))
}

fn is_ref_token(value: &str) -> bool {
    !value.is_empty() && value.trim() == value && !value.chars().any(char::is_whitespace)
}

fn validate_schema(schema: &str) -> Result<(), NixInteropError> {
    if schema != NIX_ADAPTER_SCHEMA_V1 {
        return Err(NixInteropError::UnsupportedSchema(schema.to_string()));
    }
    Ok(())
}

fn validate_sha256_hex(field: &str, value: &str) -> Result<(), NixInteropError> {
    if !is_sha256_hex(value) {
        return Err(NixInteropError::InvalidArtifact(format!(
            "{field} SHA-256 must be 64 lowercase hexadecimal characters"
        )));
    }
    Ok(())
}

fn ensure_unique(values: &[String], field: &str) -> Result<(), NixInteropError> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value.as_str()) {
            return Err(NixInteropError::InvalidAdapter(format!(
                "{field} contains duplicate `{value}`"
            )));
        }
    }
    Ok(())
}

fn canonicalize_json(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<_> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
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

    const HEX_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const HEX_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const NAR_A: &str = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
    const STORE_A: &str = "/nix/store/00000000000000000000000000000000-tool-1.2.3";

    fn strict_policy() -> NixPolicyEvidence {
        NixPolicyEvidence {
            profile: NixPolicyProfile::StrictV1,
            pure_evaluation: true,
            import_from_derivation: false,
            sandbox_required: true,
            builder_network: NixBuilderNetwork::Disabled,
            dirty_source: false,
            publishable: true,
        }
    }

    fn output(system: &str, output: &str) -> NixRealizedOutput {
        NixRealizedOutput {
            system: system.to_string(),
            output: output.to_string(),
            derivation_json_sha256: HEX_B.to_string(),
            store_path: STORE_A.to_string(),
            nar_hash: NAR_A.to_string(),
            nar_size: 128,
            references: Vec::new(),
            signatures: vec!["cache.example-1:signature".to_string()],
            nix_version: "2.35.2".to_string(),
            store_info_json_version: 3,
        }
    }

    fn package() -> NixPackageIdentity {
        NixPackageIdentity {
            org: "acme".to_string(),
            name: "tool".to_string(),
            version: "1.2.3".to_string(),
            target: None,
        }
    }

    #[test]
    fn export_intent_requires_explicit_systems_and_outputs() {
        let empty = NixExportSection::default();
        assert!(empty.validate("tool").is_err());

        let valid = NixExportSection {
            mode: NixExportMode::Artifact,
            attribute: None,
            systems: vec!["x86_64-linux".to_string()],
            outputs: vec!["out".to_string()],
        };
        valid.validate("tool").unwrap();

        let mut reserved = valid.clone();
        reserved.attribute = Some("default".to_string());
        assert!(reserved.validate("tool").is_err());
    }

    #[test]
    fn strict_policy_cannot_silently_downgrade() {
        strict_policy().validate().unwrap();
        let mut impure = strict_policy();
        impure.pure_evaluation = false;
        assert!(impure.validate().is_err());

        let development = NixPolicyEvidence {
            profile: NixPolicyProfile::Development,
            publishable: false,
            ..strict_policy()
        };
        development.validate().unwrap();
    }

    #[test]
    fn nix_to_zed_v1_rejects_runtime_store_references() {
        let mut realized = output("x86_64-linux", "out");
        realized.references.push(NixStoreReference {
            store_path: "/nix/store/11111111111111111111111111111111-glibc".to_string(),
            nar_hash: Some(NAR_A.to_string()),
            nar_size: Some(256),
        });
        let record = NixAdapterRecord::nix_to_zed(
            package(),
            NixOutputOrigin {
                locked_ref: format!("github:acme/tool/{HEX_A}"),
                flake_lock_sha256: HEX_A.to_string(),
                attribute: "packages.x86_64-linux.tool".to_string(),
                realized,
            },
            NixInteropArtifact {
                format: ArtifactFormat::TarGz,
                sha256: HEX_B.to_string(),
                size: 512,
            },
            strict_policy(),
        );
        assert!(matches!(
            record.validate(),
            Err(NixInteropError::InvalidAdapter(_))
        ));
    }

    #[test]
    fn canonical_json_normalizes_non_semantic_array_order() {
        let intent_a = NixExportSection {
            mode: NixExportMode::Artifact,
            attribute: Some("tool".to_string()),
            systems: vec!["x86_64-linux".to_string(), "aarch64-linux".to_string()],
            outputs: vec!["out".to_string()],
        };
        let intent_b = NixExportSection {
            systems: intent_a.systems.iter().cloned().rev().collect(),
            ..intent_a.clone()
        };
        let source = ZedArtifactOrigin {
            registry: "https://zpkg.example".to_string(),
            artifact: NixInteropArtifact {
                format: ArtifactFormat::TarGz,
                sha256: HEX_A.to_string(),
                size: 256,
            },
            vcs_tag: "v1.2.3".to_string(),
            vcs_commit: HEX_B[..40].to_string(),
            lock_sha256: Some(HEX_B.to_string()),
        };
        let outputs_a = vec![
            output("x86_64-linux", "out"),
            output("aarch64-linux", "out"),
        ];
        let outputs_b = outputs_a.iter().cloned().rev().collect();
        let first = NixAdapterRecord::zed_to_nix(
            package(),
            source.clone(),
            intent_a,
            HEX_A.to_string(),
            outputs_a,
            strict_policy(),
        );
        let second = NixAdapterRecord::zed_to_nix(
            package(),
            source,
            intent_b,
            HEX_A.to_string(),
            outputs_b,
            strict_policy(),
        );
        assert_eq!(
            first.canonical_json_bytes().unwrap(),
            second.canonical_json_bytes().unwrap()
        );
    }

    #[test]
    fn canonical_json_has_a_stable_compact_golden_vector() {
        let record = NixAdapterRecord::nix_to_zed(
            package(),
            NixOutputOrigin {
                locked_ref: format!("github:acme/tool/{HEX_A}"),
                flake_lock_sha256: HEX_A.to_string(),
                attribute: "packages.x86_64-linux.tool".to_string(),
                realized: output("x86_64-linux", "out"),
            },
            NixInteropArtifact {
                format: ArtifactFormat::TarGz,
                sha256: HEX_B.to_string(),
                size: 512,
            },
            strict_policy(),
        );
        let canonical = record.canonical_json_string().unwrap();
        assert!(!canonical.contains('\n'));
        assert_eq!(
            serde_json::from_str::<Value>(&canonical).unwrap(),
            serde_json::to_value(record).unwrap()
        );
        assert!(canonical.starts_with("{\"artifact\":"));
        assert!(
            canonical.ends_with("}") && canonical.contains("\"schema\":\"zed.nix-adapter/v1\"")
        );
    }

    #[test]
    fn unknown_schema_and_unknown_store_info_version_fail_closed() {
        let mut record = NixAdapterRecord::nix_to_zed(
            package(),
            NixOutputOrigin {
                locked_ref: format!("github:acme/tool/{HEX_A}"),
                flake_lock_sha256: HEX_A.to_string(),
                attribute: "packages.x86_64-linux.tool".to_string(),
                realized: output("x86_64-linux", "out"),
            },
            NixInteropArtifact {
                format: ArtifactFormat::TarGz,
                sha256: HEX_B.to_string(),
                size: 512,
            },
            strict_policy(),
        );
        if let NixAdapterRecord::NixToZed { schema, source, .. } = &mut record {
            *schema = "zed.nix-adapter/v2".to_string();
            source.realized.store_info_json_version = 99;
        }
        assert!(matches!(
            record.validate(),
            Err(NixInteropError::UnsupportedSchema(_))
        ));
    }

    #[test]
    fn immutable_ref_and_hash_helpers_are_strict() {
        assert!(is_immutable_nix_ref(&format!("github:acme/tool/{HEX_A}")));
        assert!(is_immutable_nix_ref(
            "git+https://example.invalid/repo?rev=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        ));
        assert!(!is_immutable_nix_ref("github:acme/tool/main"));
        assert!(is_sha256_sri(NAR_A));
        assert!(!is_sha256_sri("sha256-not-a-hash"));
        assert!(is_nix_store_path(STORE_A));
        assert!(!is_nix_store_path("/tmp/tool"));
    }
}
