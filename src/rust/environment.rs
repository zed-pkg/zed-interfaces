//! Shared contracts for developer-environment interoperability.
//!
//! Zed remains authoritative for the Zed package graph. These types describe
//! exact toolchains, system packages, manager-native lock provenance, and the
//! fixed activation policy used by Flox, Devbox, mise, asdf, and future Nix
//! development-shell adapters. Arbitrary imported shell hooks and secrets are
//! intentionally not representable.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use semver::Version;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Environment managers with a first-class adapter contract.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum EnvironmentManager {
    Mise,
    Asdf,
    Devbox,
    Flox,
    /// Development-shell provenance only. Nix package/derivation import and
    /// export is a separate interoperability boundary.
    Nix,
}

/// The only activation behavior a Zed environment adapter may add.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ActivationPolicy {
    #[default]
    None,
    FrozenInstall,
}

impl ActivationPolicy {
    /// The exact command emitted by adapters that support activation hooks.
    pub fn command(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::FrozenInstall => Some("zed install --frozen"),
        }
    }
}

/// How strictly an environment plan is validated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvironmentValidationMode {
    /// Requirements may be ranges and resolved identities are optional.
    Authoring,
    /// Every identity must be exact, immutable, and portable.
    FrozenPortable,
    /// Every identity must be exact and immutable; explicit local paths are
    /// allowed and make the plan intentionally non-portable.
    FrozenLocal,
}

impl EnvironmentValidationMode {
    fn is_frozen(self) -> bool {
        !matches!(self, Self::Authoring)
    }

    fn allows_local_paths(self) -> bool {
        matches!(self, Self::FrozenLocal)
    }
}

/// Supported checksum algorithms in environment provenance.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum ChecksumAlgorithm {
    Sha256,
    Sha512,
    Blake3,
}

impl ChecksumAlgorithm {
    fn expected_hex_len(self) -> usize {
        match self {
            Self::Sha256 | Self::Blake3 => 64,
            Self::Sha512 => 128,
        }
    }
}

/// One lowercase hexadecimal content checksum in canonical output.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub struct Checksum {
    pub algorithm: ChecksumAlgorithm,
    pub value: String,
}

impl Checksum {
    fn normalized(&self) -> Self {
        Self {
            algorithm: self.algorithm,
            value: self.value.trim().to_ascii_lowercase(),
        }
    }

    fn validate(&self, field: &str) -> Result<(), EnvironmentPlanError> {
        let expected_hex_len = self.algorithm.expected_hex_len();
        let value = self.value.trim();
        if value.len() != expected_hex_len || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(EnvironmentPlanError::InvalidChecksum {
                field: field.to_string(),
                algorithm: self.algorithm,
                expected_hex_len,
                value: self.value.clone(),
            });
        }
        Ok(())
    }
}

/// A source whose immutable revision is part of environment identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ImmutableSource {
    pub url: String,
    /// A full immutable commit or content digest. Moving tags and branches are
    /// rejected in frozen validation.
    pub revision: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subdir: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checksums: Vec<Checksum>,
}

impl ImmutableSource {
    fn normalized(&self) -> Self {
        let mut source = self.clone();
        source.url = source.url.trim().to_string();
        source.revision = source.revision.trim().to_ascii_lowercase();
        source.subdir = normalize_optional(&source.subdir);
        normalize_checksums(&mut source.checksums);
        source
    }

    fn validate(
        &self,
        field: &str,
        mode: EnvironmentValidationMode,
    ) -> Result<(), EnvironmentPlanError> {
        if self.url.trim().is_empty() {
            return Err(EnvironmentPlanError::EmptyField {
                field: format!("{field}.url"),
            });
        }
        if mode == EnvironmentValidationMode::FrozenPortable && is_local_reference(&self.url) {
            return Err(EnvironmentPlanError::NonPortableLocalReference {
                field: format!("{field}.url"),
                value: self.url.clone(),
            });
        }
        if mode.is_frozen() && !is_immutable_revision(&self.revision) {
            return Err(EnvironmentPlanError::MutableSourceRevision {
                field: format!("{field}.revision"),
                revision: self.revision.clone(),
            });
        }
        if let Some(subdir) = &self.subdir {
            validate_relative_path(&format!("{field}.subdir"), subdir)?;
        }
        validate_checksums(&format!("{field}.checksums"), &self.checksums)
    }
}

/// Desired and resolved identity for one runtime or developer tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ToolRequirement {
    /// Author-authored requirement. It may be a range in authoring mode.
    pub requirement: String,
    /// Exact manager-native result. Required in frozen modes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<ImmutableSource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checksums: Vec<Checksum>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub platforms: Vec<String>,
}

impl ToolRequirement {
    fn normalized(&self) -> Self {
        let mut requirement = self.clone();
        requirement.requirement = requirement.requirement.trim().to_string();
        requirement.resolved = normalize_optional(&requirement.resolved);
        requirement.provider = normalize_optional(&requirement.provider);
        requirement.backend = normalize_optional(&requirement.backend);
        requirement.source = requirement.source.as_ref().map(ImmutableSource::normalized);
        normalize_checksums(&mut requirement.checksums);
        normalize_strings(&mut requirement.platforms);
        requirement
    }

    fn validate(
        &self,
        name: &str,
        mode: EnvironmentValidationMode,
    ) -> Result<(), EnvironmentPlanError> {
        validate_requirement(
            "tool",
            name,
            &self.requirement,
            self.resolved.as_deref(),
            mode,
        )?;
        if let Some(source) = &self.source {
            source.validate(&format!("tools.{name}.source"), mode)?;
        }
        validate_checksums(&format!("tools.{name}.checksums"), &self.checksums)?;
        validate_platforms(&format!("tools.{name}.platforms"), &self.platforms)
    }
}

/// Desired and resolved identity for one system package supplied by an
/// environment manager rather than by the Zed dependency graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SystemPackageRequirement {
    pub requirement: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved: Option<String>,
    /// Manager/catalog provider, such as `nixpkgs`, a Flox catalog, or a flake.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Exact provider-native attribute/reference when it differs from the
    /// normalized package name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<ImmutableSource>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checksums: Vec<Checksum>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub platforms: Vec<String>,
}

impl SystemPackageRequirement {
    fn normalized(&self) -> Self {
        let mut requirement = self.clone();
        requirement.requirement = requirement.requirement.trim().to_string();
        requirement.resolved = normalize_optional(&requirement.resolved);
        requirement.provider = normalize_optional(&requirement.provider);
        requirement.package_ref = normalize_optional(&requirement.package_ref);
        requirement.source = requirement.source.as_ref().map(ImmutableSource::normalized);
        normalize_checksums(&mut requirement.checksums);
        normalize_strings(&mut requirement.platforms);
        requirement
    }

    fn validate(
        &self,
        name: &str,
        mode: EnvironmentValidationMode,
    ) -> Result<(), EnvironmentPlanError> {
        validate_requirement(
            "system package",
            name,
            &self.requirement,
            self.resolved.as_deref(),
            mode,
        )?;
        if let Some(source) = &self.source {
            source.validate(&format!("system-packages.{name}.source"), mode)?;
        }
        validate_checksums(
            &format!("system-packages.{name}.checksums"),
            &self.checksums,
        )?;
        validate_platforms(
            &format!("system-packages.{name}.platforms"),
            &self.platforms,
        )
    }
}

/// Provenance for one manager-native input and optional lock file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EnvironmentSource {
    pub manager: EnvironmentManager,
    /// Project-relative manager input path.
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lock_path: Option<String>,
    /// Digest of the normalized manager-native lock/input state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest: Option<Checksum>,
}

impl EnvironmentSource {
    fn normalized(&self) -> Self {
        Self {
            manager: self.manager,
            path: self.path.trim().to_string(),
            lock_path: normalize_optional(&self.lock_path),
            digest: self.digest.as_ref().map(Checksum::normalized),
        }
    }

    fn validate(&self, index: usize) -> Result<(), EnvironmentPlanError> {
        let field = format!("sources[{index}]");
        validate_relative_path(&format!("{field}.path"), &self.path)?;
        if let Some(lock_path) = &self.lock_path {
            validate_relative_path(&format!("{field}.lock-path"), lock_path)?;
        }
        if let Some(digest) = &self.digest {
            digest.validate(&format!("{field}.digest"))?;
        }
        Ok(())
    }
}

/// Manager-neutral desired and resolved developer-environment state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EnvironmentPlan {
    #[serde(default = "current_environment_schema")]
    pub schema: u32,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tools: BTreeMap<String, ToolRequirement>,
    #[serde(
        default,
        rename = "system-packages",
        alias = "system_packages",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    pub system_packages: BTreeMap<String, SystemPackageRequirement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub platforms: Vec<String>,
    #[serde(default)]
    pub activation: ActivationPolicy,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sources: Vec<EnvironmentSource>,
}

fn current_environment_schema() -> u32 {
    EnvironmentPlan::CURRENT_SCHEMA
}

impl Default for EnvironmentPlan {
    fn default() -> Self {
        Self {
            schema: Self::CURRENT_SCHEMA,
            tools: BTreeMap::new(),
            system_packages: BTreeMap::new(),
            platforms: Vec::new(),
            activation: ActivationPolicy::None,
            sources: Vec::new(),
        }
    }
}

impl EnvironmentPlan {
    pub const CURRENT_SCHEMA: u32 = 1;

    /// Return a presentation-independent form for deterministic generation and
    /// hashing. Invalid map keys remain unchanged so validation cannot be
    /// bypassed by normalization.
    pub fn normalized(&self) -> Self {
        let mut plan = self.clone();
        plan.tools = plan
            .tools
            .iter()
            .map(|(name, requirement)| (name.clone(), requirement.normalized()))
            .collect();
        plan.system_packages = plan
            .system_packages
            .iter()
            .map(|(name, requirement)| (name.clone(), requirement.normalized()))
            .collect();
        normalize_strings(&mut plan.platforms);
        plan.sources = plan
            .sources
            .iter()
            .map(EnvironmentSource::normalized)
            .collect();
        plan.sources.sort_by(|left, right| {
            (left.manager, &left.path, &left.lock_path, &left.digest).cmp(&(
                right.manager,
                &right.path,
                &right.lock_path,
                &right.digest,
            ))
        });
        plan.sources.dedup();
        plan
    }

    /// Canonical compact JSON bytes for the environment-plan digest.
    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>, EnvironmentPlanError> {
        serde_json::to_vec(&self.normalized())
            .map_err(|error| EnvironmentPlanError::Serialization(error.to_string()))
    }

    pub fn validate(&self, mode: EnvironmentValidationMode) -> Result<(), EnvironmentPlanError> {
        if self.schema == 0 || self.schema > Self::CURRENT_SCHEMA {
            return Err(EnvironmentPlanError::UnsupportedSchema {
                found: self.schema,
                supported: Self::CURRENT_SCHEMA,
            });
        }
        validate_platforms("platforms", &self.platforms)?;
        for (name, requirement) in &self.tools {
            validate_name("tool", name)?;
            requirement.validate(name, mode)?;
        }
        for (name, requirement) in &self.system_packages {
            validate_name("system package", name)?;
            requirement.validate(name, mode)?;
        }
        for (index, source) in self.sources.iter().enumerate() {
            source.validate(index)?;
        }
        Ok(())
    }
}

/// Parse strict SemVer 2.0.0 for an adapter or registry that requires it.
pub fn validate_semver_export(version: &str) -> Result<Version, EnvironmentPlanError> {
    Version::parse(version).map_err(|error| EnvironmentPlanError::InvalidSemver {
        version: version.to_string(),
        detail: error.to_string(),
    })
}

/// Return true when valid SemVer strings have identical precedence fields and
/// differ only in build metadata.
pub fn differ_only_in_build_metadata(
    left: &str,
    right: &str,
) -> Result<bool, EnvironmentPlanError> {
    let left = validate_semver_export(left)?;
    let right = validate_semver_export(right)?;
    Ok(left.major == right.major
        && left.minor == right.minor
        && left.patch == right.patch
        && left.pre == right.pre
        && left.build != right.build)
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EnvironmentPlanError {
    #[error("unsupported environment plan schema {found}; this build supports {supported}")]
    UnsupportedSchema { found: u32, supported: u32 },
    #[error("{field} must not be empty")]
    EmptyField { field: String },
    #[error("invalid {kind} name `{name}`; names cannot contain whitespace or controls")]
    InvalidName { kind: &'static str, name: String },
    #[error("{kind} `{name}` has no exact resolved identity for frozen validation")]
    Unresolved { kind: &'static str, name: String },
    #[error("{kind} `{name}` resolves to moving selector `{value}`")]
    MovingSelector {
        kind: &'static str,
        name: String,
        value: String,
    },
    #[error("{field} uses non-portable local reference `{value}`")]
    NonPortableLocalReference { field: String, value: String },
    #[error("{field} has mutable or non-canonical source revision `{revision}`")]
    MutableSourceRevision { field: String, revision: String },
    #[error(
        "{field} has invalid {algorithm:?} checksum `{value}`; expected {expected_hex_len} hexadecimal characters"
    )]
    InvalidChecksum {
        field: String,
        algorithm: ChecksumAlgorithm,
        expected_hex_len: usize,
        value: String,
    },
    #[error("{field} must be a safe project-relative path, got `{value}`")]
    UnsafeRelativePath { field: String, value: String },
    #[error("{field} contains invalid platform `{value}`")]
    InvalidPlatform { field: String, value: String },
    #[error("`{version}` is not strict SemVer 2.0.0: {detail}")]
    InvalidSemver { version: String, detail: String },
    #[error("environment plan serialization failed: {0}")]
    Serialization(String),
}

fn validate_requirement(
    kind: &'static str,
    name: &str,
    requirement: &str,
    resolved: Option<&str>,
    mode: EnvironmentValidationMode,
) -> Result<(), EnvironmentPlanError> {
    if requirement.trim().is_empty() {
        return Err(EnvironmentPlanError::EmptyField {
            field: format!("{kind} `{name}` requirement"),
        });
    }
    if !mode.is_frozen() {
        return Ok(());
    }

    let resolved = resolved
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| EnvironmentPlanError::Unresolved {
            kind,
            name: name.to_string(),
        })?;

    if is_local_reference(resolved) {
        if !mode.allows_local_paths() {
            return Err(EnvironmentPlanError::NonPortableLocalReference {
                field: format!("{kind} `{name}` resolved identity"),
                value: resolved.to_string(),
            });
        }
        return Ok(());
    }
    if is_moving_selector(resolved) {
        return Err(EnvironmentPlanError::MovingSelector {
            kind,
            name: name.to_string(),
            value: resolved.to_string(),
        });
    }
    Ok(())
}

fn validate_name(kind: &'static str, name: &str) -> Result<(), EnvironmentPlanError> {
    if name.is_empty()
        || name.trim() != name
        || name
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
    {
        return Err(EnvironmentPlanError::InvalidName {
            kind,
            name: name.to_string(),
        });
    }
    Ok(())
}

fn validate_checksums(field: &str, checksums: &[Checksum]) -> Result<(), EnvironmentPlanError> {
    for checksum in checksums {
        checksum.validate(field)?;
    }
    Ok(())
}

fn validate_platforms(field: &str, platforms: &[String]) -> Result<(), EnvironmentPlanError> {
    for platform in platforms {
        let value = platform.trim();
        if value.is_empty()
            || value != platform
            || value
                .chars()
                .any(|character| character.is_whitespace() || character.is_control())
        {
            return Err(EnvironmentPlanError::InvalidPlatform {
                field: field.to_string(),
                value: platform.clone(),
            });
        }
    }
    Ok(())
}

fn validate_relative_path(field: &str, value: &str) -> Result<(), EnvironmentPlanError> {
    let trimmed = value.trim();
    let has_drive_prefix = trimmed.as_bytes().get(1) == Some(&b':');
    let has_unsafe_segment = trimmed
        .split(['/', '\\'])
        .any(|segment| segment.is_empty() || segment == "." || segment == "..");
    if trimmed.is_empty()
        || trimmed != value
        || trimmed.starts_with('/')
        || trimmed.starts_with('\\')
        || has_drive_prefix
        || has_unsafe_segment
        || trimmed.chars().any(|character| character.is_control())
    {
        return Err(EnvironmentPlanError::UnsafeRelativePath {
            field: field.to_string(),
            value: value.to_string(),
        });
    }
    Ok(())
}

fn normalize_optional(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn normalize_strings(values: &mut Vec<String>) {
    *values = values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect();
    values.sort();
    values.dedup();
}

fn normalize_checksums(checksums: &mut Vec<Checksum>) {
    *checksums = checksums.iter().map(Checksum::normalized).collect();
    checksums.sort();
    checksums.dedup();
}

fn is_local_reference(value: &str) -> bool {
    let value = value.trim();
    value.starts_with("path:")
        || value.starts_with("file:")
        || value.starts_with("./")
        || value.starts_with("../")
        || value.starts_with('/')
        || value.starts_with('\\')
        || value.as_bytes().get(1) == Some(&b':')
}

fn is_moving_selector(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    if matches!(
        value.as_str(),
        "latest" | "stable" | "lts" | "system" | "head" | "main" | "master"
    ) {
        return true;
    }
    if value.starts_with("prefix:") || value.starts_with("sub-") {
        return true;
    }
    if let Some(revision) = value.strip_prefix("ref:") {
        return !is_full_hex_revision(revision);
    }
    if value.contains('*')
        || value.contains('^')
        || value.contains('~')
        || value.contains('<')
        || value.contains('>')
        || value.contains('|')
        || value.contains(',')
        || value.chars().any(char::is_whitespace)
    {
        return true;
    }

    let precedence_core = value.split(['-', '+']).next().unwrap_or(value.as_str());
    precedence_core.split('.').any(|segment| segment == "x")
}

fn is_immutable_revision(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    if is_full_hex_revision(&value) {
        return true;
    }
    if let Some(digest) = value.strip_prefix("sha256:") {
        return digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit());
    }
    if let Some(digest) = value.strip_prefix("sha512:") {
        return digest.len() == 128 && digest.bytes().all(|byte| byte.is_ascii_hexdigit());
    }
    false
}

fn is_full_hex_revision(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sha256(digit: char) -> Checksum {
        Checksum {
            algorithm: ChecksumAlgorithm::Sha256,
            value: digit.to_string().repeat(64),
        }
    }

    fn exact_tool(resolved: &str) -> ToolRequirement {
        ToolRequirement {
            requirement: "^22".to_string(),
            resolved: Some(resolved.to_string()),
            provider: Some("core".to_string()),
            backend: None,
            source: None,
            checksums: vec![sha256('a')],
            platforms: vec!["x86_64-linux".to_string()],
        }
    }

    #[test]
    fn activation_policy_exposes_only_the_fixed_frozen_command() {
        assert_eq!(ActivationPolicy::None.command(), None);
        assert_eq!(
            ActivationPolicy::FrozenInstall.command(),
            Some("zed install --frozen")
        );
    }

    #[test]
    fn authoring_accepts_ranges_but_frozen_requires_resolution() {
        let mut plan = EnvironmentPlan::default();
        plan.tools.insert(
            "node".to_string(),
            ToolRequirement {
                requirement: "^22".to_string(),
                resolved: None,
                provider: None,
                backend: None,
                source: None,
                checksums: Vec::new(),
                platforms: Vec::new(),
            },
        );

        plan.validate(EnvironmentValidationMode::Authoring).unwrap();
        assert!(matches!(
            plan.validate(EnvironmentValidationMode::FrozenPortable),
            Err(EnvironmentPlanError::Unresolved { .. })
        ));
    }

    #[test]
    fn frozen_validation_rejects_moving_and_nonportable_resolutions() {
        for value in ["latest", "lts", "prefix:22", "ref:master", "^22", "22.x"] {
            let mut plan = EnvironmentPlan::default();
            plan.tools.insert("node".to_string(), exact_tool(value));
            assert!(matches!(
                plan.validate(EnvironmentValidationMode::FrozenPortable),
                Err(EnvironmentPlanError::MovingSelector { .. })
            ));
        }

        let mut plan = EnvironmentPlan::default();
        plan.tools
            .insert("node".to_string(), exact_tool("path:./toolchain"));
        assert!(matches!(
            plan.validate(EnvironmentValidationMode::FrozenPortable),
            Err(EnvironmentPlanError::NonPortableLocalReference { .. })
        ));
        plan.validate(EnvironmentValidationMode::FrozenLocal)
            .unwrap();
    }

    #[test]
    fn prerelease_x_is_not_a_wildcard() {
        let mut plan = EnvironmentPlan::default();
        plan.tools
            .insert("node".to_string(), exact_tool("22.0.0-x.1"));
        plan.validate(EnvironmentValidationMode::FrozenPortable)
            .unwrap();
    }

    #[test]
    fn frozen_sources_require_full_immutable_revisions() {
        let mut plan = EnvironmentPlan::default();
        let mut tool = exact_tool("22.11.0");
        tool.source = Some(ImmutableSource {
            url: "https://github.com/example/tool.git".to_string(),
            revision: "main".to_string(),
            subdir: None,
            checksums: Vec::new(),
        });
        plan.tools.insert("node".to_string(), tool);
        assert!(matches!(
            plan.validate(EnvironmentValidationMode::FrozenPortable),
            Err(EnvironmentPlanError::MutableSourceRevision { .. })
        ));

        plan.tools
            .get_mut("node")
            .unwrap()
            .source
            .as_mut()
            .unwrap()
            .revision = "0123456789abcdef0123456789abcdef01234567".to_string();
        plan.validate(EnvironmentValidationMode::FrozenPortable)
            .unwrap();
    }

    #[test]
    fn canonical_bytes_ignore_set_order_and_duplicates() {
        let mut first = EnvironmentPlan {
            platforms: vec![
                "x86_64-linux".to_string(),
                "aarch64-darwin".to_string(),
                "x86_64-linux".to_string(),
            ],
            activation: ActivationPolicy::FrozenInstall,
            sources: vec![
                EnvironmentSource {
                    manager: EnvironmentManager::Mise,
                    path: "mise.toml".to_string(),
                    lock_path: Some("mise.lock".to_string()),
                    digest: Some(sha256('b')),
                },
                EnvironmentSource {
                    manager: EnvironmentManager::Mise,
                    path: "mise.toml".to_string(),
                    lock_path: Some("mise.lock".to_string()),
                    digest: Some(sha256('b')),
                },
            ],
            ..EnvironmentPlan::default()
        };
        let mut node = exact_tool("22.11.0");
        node.platforms = vec![
            "x86_64-linux".to_string(),
            "aarch64-darwin".to_string(),
            "x86_64-linux".to_string(),
        ];
        first.tools.insert("node".to_string(), node);

        let mut second = first.clone();
        second.platforms.reverse();
        second.sources.reverse();
        second.tools.get_mut("node").unwrap().platforms.reverse();

        assert_eq!(
            first.canonical_json_bytes().unwrap(),
            second.canonical_json_bytes().unwrap()
        );
    }

    #[test]
    fn normalization_does_not_hide_invalid_map_keys() {
        let mut plan = EnvironmentPlan::default();
        plan.tools
            .insert(" node ".to_string(), exact_tool("22.11.0"));
        assert!(matches!(
            plan.normalized()
                .validate(EnvironmentValidationMode::FrozenPortable),
            Err(EnvironmentPlanError::InvalidName { .. })
        ));
    }

    #[test]
    fn canonical_json_roundtrips() {
        let mut plan = EnvironmentPlan {
            activation: ActivationPolicy::FrozenInstall,
            ..EnvironmentPlan::default()
        };
        plan.tools.insert("node".to_string(), exact_tool("22.11.0"));
        let bytes = plan.canonical_json_bytes().unwrap();
        let parsed: EnvironmentPlan = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(parsed, plan.normalized());
    }

    #[test]
    fn checksums_are_length_checked() {
        let mut plan = EnvironmentPlan::default();
        let mut tool = exact_tool("22.11.0");
        tool.checksums = vec![Checksum {
            algorithm: ChecksumAlgorithm::Sha256,
            value: "abc".to_string(),
        }];
        plan.tools.insert("node".to_string(), tool);
        assert!(matches!(
            plan.validate(EnvironmentValidationMode::FrozenPortable),
            Err(EnvironmentPlanError::InvalidChecksum { .. })
        ));
    }

    #[test]
    fn strict_semver_is_an_export_boundary() {
        assert!(validate_semver_export("1.2.3-rc.1+build.7").is_ok());
        assert!(validate_semver_export("v1.2.3").is_err());
        assert!(validate_semver_export("legacy-api").is_err());
        assert!(differ_only_in_build_metadata("1.2.3+arm64", "1.2.3+x86-64").unwrap());
        assert!(!differ_only_in_build_metadata("1.2.3", "1.2.4").unwrap());
    }

    #[test]
    fn manager_paths_cannot_escape_the_project() {
        let plan = EnvironmentPlan {
            sources: vec![EnvironmentSource {
                manager: EnvironmentManager::Devbox,
                path: "../devbox.json".to_string(),
                lock_path: None,
                digest: None,
            }],
            ..EnvironmentPlan::default()
        };
        assert!(matches!(
            plan.validate(EnvironmentValidationMode::Authoring),
            Err(EnvironmentPlanError::UnsafeRelativePath { .. })
        ));
    }
}
