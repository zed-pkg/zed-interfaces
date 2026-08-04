//! Exact, manager-neutral locks for native Zed development environments.
//!
//! Human-authored requirements belong in an environment plan. This module
//! records the immutable backend, source, platform, artifact, and install
//! identities selected from that plan so installs can be frozen, replayed
//! offline, verified for tampering, and garbage-collected without requiring the
//! source environment manager.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Current on-disk/wire schema for [`EnvironmentLock`].
pub const ENVIRONMENT_LOCK_SCHEMA_VERSION: u32 = 1;

fn default_schema_version() -> u32 {
    ENVIRONMENT_LOCK_SCHEMA_VERSION
}

/// Portability boundary used while validating an environment lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EnvironmentLockValidationMode {
    /// Reject local-only source identities and machine-specific state.
    Portable,
    /// Permit explicitly local source identities, while retaining exact tree
    /// digests and project-relative paths.
    Local,
}

/// Exact resolved state for all tools in one development environment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EnvironmentLock {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,

    /// SHA-256 of the normalized environment plan that produced this lock.
    pub plan_digest_sha256: String,

    /// Logical tool name to one or more exact version/platform variants.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tools: BTreeMap<String, Vec<LockedTool>>,

    /// Lossless future/backend fields. Unknown state must never disappear
    /// merely because an older client rewrote a lock.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

impl Default for EnvironmentLock {
    fn default() -> Self {
        Self {
            schema_version: ENVIRONMENT_LOCK_SCHEMA_VERSION,
            plan_digest_sha256: String::new(),
            tools: BTreeMap::new(),
            extensions: BTreeMap::new(),
        }
    }
}

/// One exact backend/version/platform selection for a logical tool.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LockedTool {
    /// Original human-authored requirement retained for diagnostics.
    pub requirement: String,

    /// Exact, non-moving version selected by the backend.
    pub resolved: String,

    /// Backend/provider identity (`core`, `aqua`, `github`, `npm`, `cargo`,
    /// `ubi`, `asdf`, `vfox`, `http`, `zed`, ...).
    pub backend: String,

    /// Exact backend/plugin implementation version when backend behavior can
    /// affect resolution or installation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend_version: Option<String>,

    /// Digest of normalized backend options, excluding credentials/secrets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend_options_digest_sha256: Option<String>,

    pub source: LockedSource,
    pub artifact: LockedArtifact,
    pub platform: LockedPlatform,
    pub install: LockedInstall,

    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

/// Source category for an exact tool artifact.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum LockedSourceKind {
    Registry,
    Vcs,
    Http,
    Oci,
    Path,
    Other,
}

/// Exact source identity used before artifact verification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LockedSource {
    pub kind: LockedSourceKind,

    /// Registry coordinates, repository URL, HTTP URL, OCI reference, or a
    /// project-relative path.
    pub locator: String,

    /// Exact package revision, VCS object, OCI digest, or backend identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,

    /// SHA-256 of a local directory tree for `path` sources.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tree_sha256: Option<String>,

    /// True when backend semantics guarantee that `revision` cannot move.
    #[serde(default)]
    pub immutable: bool,

    /// A relative path source may be marked portable only when its complete
    /// tree identity is locked and intended to travel with the project.
    #[serde(default)]
    pub portable: bool,

    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

/// Archive/blob identity downloaded into the content-addressed store.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LockedArtifact {
    /// Exact lowercase hexadecimal SHA-256, without a `sha256:` prefix.
    pub sha256: String,
    pub size: u64,
    pub format: LockedArtifactFormat,

    /// Alternate immutable download locations. Order is not semantic.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mirrors: Vec<String>,

    /// Signature, transparency-log, or attestation identities. Verification
    /// policy remains a caller concern; the lock records what was verified.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signatures: Vec<LockedSignature>,

    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum LockedArtifactFormat {
    Tar,
    TarGz,
    TarXz,
    TarZstd,
    Zip,
    Raw,
    Directory,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub struct LockedSignature {
    /// Signature system (`cosign`, `minisign`, `gpg`, `sigstore-bundle`, ...).
    pub kind: String,
    /// Key, certificate, transparency-log, or issuer/subject identity.
    pub identity: String,
    /// Optional SHA-256 of detached signature or attestation bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

/// Exact target identity for one locked variant.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub struct LockedPlatform {
    /// Canonical target triple or backend target identifier.
    pub target: String,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub os: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub libc: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abi: Option<String>,
}

/// Verified install layout within an extracted artifact/store entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LockedInstall {
    /// Relative root selected from the extracted artifact. `.` means the
    /// artifact root.
    pub root: String,

    /// Relative PATH directories below `root`. Order is not semantic here;
    /// activation precedence is defined by the environment plan/runtime.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bin_dirs: Vec<String>,

    /// Executables and aliases exposed by this variant.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub executables: Vec<LockedExecutable>,

    /// Digest of normalized layout metadata when a backend has additional
    /// deterministic install-layout decisions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout_digest_sha256: Option<String>,

    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extensions: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LockedExecutable {
    pub name: String,
    /// Relative to [`LockedInstall::root`].
    pub path: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
}

/// Validation, parsing, serialization, and identity failures.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EnvironmentLockError {
    #[error("unsupported environment lock schema version {found}; this build supports {supported}")]
    UnsupportedSchemaVersion { found: u32, supported: u32 },

    #[error("{field} cannot be empty")]
    EmptyField { field: String },

    #[error("{field} contains a control character")]
    ControlCharacter { field: String },

    #[error("{field} must be a 64-character hexadecimal SHA-256 digest")]
    InvalidSha256 { field: String },

    #[error("{field} must not contain credentials, query parameters, or fragments: `{value}`")]
    UnsafeLocator { field: String, value: String },

    #[error("tool `{tool}` source {kind:?} is incompatible with artifact format {format:?}")]
    SourceArtifactMismatch {
        tool: String,
        kind: LockedSourceKind,
        format: LockedArtifactFormat,
    },

    #[error("extension value `{path}` cannot be null")]
    NullExtension { path: String },

    #[error("tool `{tool}` has no locked variants")]
    ToolWithoutVariants { tool: String },

    #[error("tool `{tool}` resolved to moving selector `{value}`")]
    FloatingResolvedVersion { tool: String, value: String },

    #[error("tool `{tool}` has mutable or incomplete {kind:?} source provenance")]
    MutableSource {
        tool: String,
        kind: LockedSourceKind,
    },

    #[error("tool `{tool}` uses local-only source `{locator}` in a portable lock")]
    LocalSourceNotPortable { tool: String, locator: String },

    #[error("{field} must be a portable relative path: `{path}`")]
    UnsafeRelativePath { field: String, path: String },

    #[error("tool `{tool}` contains duplicate locked variant `{identity}`")]
    DuplicateVariant { tool: String, identity: String },

    #[error("tool `{tool}` variant `{variant}` exposes executable name `{name}` more than once")]
    ExecutableCollision {
        tool: String,
        variant: String,
        name: String,
    },

    #[error("tool `{tool}` variant `{variant}` has invalid executable name `{name}`")]
    InvalidExecutableName {
        tool: String,
        variant: String,
        name: String,
    },

    #[error("expected environment plan digest {expected}, but lock records {actual}")]
    PlanDigestMismatch { expected: String, actual: String },

    #[error("invalid environment lock TOML: {message}")]
    TomlParse { message: String },

    #[error("could not serialize environment lock as TOML: {message}")]
    TomlSerialize { message: String },

    #[error("invalid environment lock JSON: {message}")]
    JsonParse { message: String },

    #[error("could not serialize canonical environment lock: {message}")]
    JsonSerialize { message: String },
}

impl EnvironmentLock {
    /// Parse TOML and apply local frozen validation.
    pub fn parse_toml(input: &str) -> Result<Self, EnvironmentLockError> {
        let lock: Self =
            toml::from_str(input).map_err(|error| EnvironmentLockError::TomlParse {
                message: error.to_string(),
            })?;
        lock.validate(EnvironmentLockValidationMode::Local)?;
        Ok(lock)
    }

    /// Parse JSON and apply local frozen validation.
    pub fn parse_json(input: &str) -> Result<Self, EnvironmentLockError> {
        let lock: Self =
            serde_json::from_str(input).map_err(|error| EnvironmentLockError::JsonParse {
                message: error.to_string(),
            })?;
        lock.validate(EnvironmentLockValidationMode::Local)?;
        Ok(lock)
    }

    /// Validate exact frozen identities and the selected portability boundary.
    pub fn validate(
        &self,
        mode: EnvironmentLockValidationMode,
    ) -> Result<(), EnvironmentLockError> {
        if self.schema_version != ENVIRONMENT_LOCK_SCHEMA_VERSION {
            return Err(EnvironmentLockError::UnsupportedSchemaVersion {
                found: self.schema_version,
                supported: ENVIRONMENT_LOCK_SCHEMA_VERSION,
            });
        }
        validate_sha256("plan_digest_sha256", &self.plan_digest_sha256)?;
        validate_extensions("extensions", &self.extensions)?;

        for (tool_name, variants) in &self.tools {
            validate_text(&format!("tool name `{tool_name}`"), tool_name)?;
            if variants.is_empty() {
                return Err(EnvironmentLockError::ToolWithoutVariants {
                    tool: tool_name.clone(),
                });
            }

            let mut identities = BTreeSet::new();
            for variant in variants {
                variant.validate(tool_name, mode)?;
                let identity = variant.variant_identity();
                if !identities.insert(identity.clone()) {
                    return Err(EnvironmentLockError::DuplicateVariant {
                        tool: tool_name.clone(),
                        identity,
                    });
                }
            }
        }
        Ok(())
    }

    /// Canonical clone used for generation, drift comparison, and hashing.
    pub fn normalized(&self) -> Self {
        let mut lock = self.clone();
        lock.plan_digest_sha256.make_ascii_lowercase();

        for variants in lock.tools.values_mut() {
            for variant in variants.iter_mut() {
                variant.normalize();
            }
            variants.sort_by_cached_key(LockedTool::stable_key);
        }
        lock
    }

    pub fn to_toml_string(&self) -> Result<String, EnvironmentLockError> {
        self.validate(EnvironmentLockValidationMode::Local)?;
        toml::to_string_pretty(&self.normalized()).map_err(|error| {
            EnvironmentLockError::TomlSerialize {
                message: error.to_string(),
            }
        })
    }

    pub fn canonical_json_string(&self) -> Result<String, EnvironmentLockError> {
        self.validate(EnvironmentLockValidationMode::Local)?;
        serde_json::to_string_pretty(&self.normalized()).map_err(|error| {
            EnvironmentLockError::JsonSerialize {
                message: error.to_string(),
            }
        })
    }

    /// SHA-256 over compact canonical JSON.
    pub fn normalized_digest_sha256(&self) -> Result<String, EnvironmentLockError> {
        self.validate(EnvironmentLockValidationMode::Local)?;
        let bytes = serde_json::to_vec(&self.normalized()).map_err(|error| {
            EnvironmentLockError::JsonSerialize {
                message: error.to_string(),
            }
        })?;
        Ok(hex::encode(Sha256::digest(bytes)))
    }

    /// Verify that this lock belongs to one normalized environment plan.
    pub fn verify_plan_digest(&self, expected: &str) -> Result<(), EnvironmentLockError> {
        validate_sha256("expected plan digest", expected)?;
        if !self.plan_digest_sha256.eq_ignore_ascii_case(expected) {
            return Err(EnvironmentLockError::PlanDigestMismatch {
                expected: expected.to_ascii_lowercase(),
                actual: self.plan_digest_sha256.to_ascii_lowercase(),
            });
        }
        Ok(())
    }

    /// Exact variants for one target, retaining multi-version declaration
    /// order only where it remains encoded in distinct locked records.
    pub fn variants_for_target<'a>(
        &'a self,
        tool: &'a str,
        target: &'a str,
    ) -> impl Iterator<Item = &'a LockedTool> + 'a {
        self.tools
            .get(tool)
            .into_iter()
            .flatten()
            .filter(move |variant| variant.platform.target == target)
    }
}

impl LockedTool {
    fn validate(
        &self,
        tool: &str,
        mode: EnvironmentLockValidationMode,
    ) -> Result<(), EnvironmentLockError> {
        validate_text(&format!("tool `{tool}` requirement"), &self.requirement)?;
        validate_text(&format!("tool `{tool}` resolved version"), &self.resolved)?;
        validate_text(&format!("tool `{tool}` backend"), &self.backend)?;
        if looks_floating(&self.resolved) {
            return Err(EnvironmentLockError::FloatingResolvedVersion {
                tool: tool.to_string(),
                value: self.resolved.clone(),
            });
        }
        if let Some(version) = &self.backend_version {
            validate_text(&format!("tool `{tool}` backend version"), version)?;
            if looks_floating(version) {
                return Err(EnvironmentLockError::FloatingResolvedVersion {
                    tool: format!("{tool} backend"),
                    value: version.clone(),
                });
            }
        }
        if let Some(digest) = &self.backend_options_digest_sha256 {
            validate_sha256(&format!("tool `{tool}` backend options digest"), digest)?;
        }
        validate_extensions(&format!("tool `{tool}` extensions"), &self.extensions)?;
        self.artifact.validate(tool)?;
        self.source.validate(tool, &self.artifact, mode)?;
        self.platform.validate(tool)?;
        self.install.validate(tool, &self.variant_identity())?;
        Ok(())
    }

    fn variant_identity(&self) -> String {
        format!(
            "{}:{}@{}:{}",
            self.backend, self.resolved, self.platform.target, self.source.locator
        )
    }

    fn stable_key(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| self.variant_identity())
    }

    fn normalize(&mut self) {
        if let Some(digest) = &mut self.backend_options_digest_sha256 {
            digest.make_ascii_lowercase();
        }
        self.source.normalize();
        self.artifact.normalize();
        self.install.normalize();
    }
}

impl LockedSource {
    fn validate(
        &self,
        tool: &str,
        artifact: &LockedArtifact,
        mode: EnvironmentLockValidationMode,
    ) -> Result<(), EnvironmentLockError> {
        let locator_field = format!("tool `{tool}` source locator");
        validate_text(&locator_field, &self.locator)?;
        validate_source_locator(&locator_field, &self.locator, self.kind)?;
        if let Some(revision) = &self.revision {
            validate_text(&format!("tool `{tool}` source revision"), revision)?;
        }
        if let Some(digest) = &self.tree_sha256 {
            validate_sha256(&format!("tool `{tool}` source tree digest"), digest)?;
        }
        validate_extensions(
            &format!("tool `{tool}` source extensions"),
            &self.extensions,
        )?;

        let revision_is_exact = self
            .revision
            .as_deref()
            .is_some_and(|revision| !looks_floating(revision));

        let exact = match self.kind {
            LockedSourceKind::Registry => revision_is_exact && self.immutable,
            LockedSourceKind::Vcs | LockedSourceKind::Other => revision_is_exact && self.immutable,
            LockedSourceKind::Http => valid_sha256(&artifact.sha256),
            LockedSourceKind::Oci => self.revision.as_deref().is_some_and(valid_prefixed_sha256),
            LockedSourceKind::Path => {
                validate_relative_path(
                    &format!("tool `{tool}` path source"),
                    &self.locator,
                    false,
                )?;
                self.tree_sha256.as_deref().is_some_and(valid_sha256)
            }
        };

        if !exact {
            return Err(EnvironmentLockError::MutableSource {
                tool: tool.to_string(),
                kind: self.kind,
            });
        }
        let path_source = self.kind == LockedSourceKind::Path;
        let directory_artifact = artifact.format == LockedArtifactFormat::Directory;
        if path_source != directory_artifact || (!path_source && self.tree_sha256.is_some()) {
            return Err(EnvironmentLockError::SourceArtifactMismatch {
                tool: tool.to_string(),
                kind: self.kind,
                format: artifact.format,
            });
        }

        if self.kind == LockedSourceKind::Path
            && mode == EnvironmentLockValidationMode::Portable
            && !self.portable
        {
            return Err(EnvironmentLockError::LocalSourceNotPortable {
                tool: tool.to_string(),
                locator: self.locator.clone(),
            });
        }
        if self.kind != LockedSourceKind::Path && self.portable {
            return Err(EnvironmentLockError::MutableSource {
                tool: tool.to_string(),
                kind: self.kind,
            });
        }
        Ok(())
    }

    fn normalize(&mut self) {
        if let Some(digest) = &mut self.tree_sha256 {
            digest.make_ascii_lowercase();
        }
        if self.kind == LockedSourceKind::Path {
            self.locator = portable_path(&self.locator);
        }
    }
}

impl LockedArtifact {
    fn validate(&self, tool: &str) -> Result<(), EnvironmentLockError> {
        validate_sha256(&format!("tool `{tool}` artifact digest"), &self.sha256)?;
        for (index, mirror) in self.mirrors.iter().enumerate() {
            let field = format!("tool `{tool}` mirror {index}");
            validate_text(&field, mirror)?;
            validate_network_locator(&field, mirror, false)?;
        }
        for (index, signature) in self.signatures.iter().enumerate() {
            validate_text(
                &format!("tool `{tool}` signature {index} kind"),
                &signature.kind,
            )?;
            validate_text(
                &format!("tool `{tool}` signature {index} identity"),
                &signature.identity,
            )?;
            if let Some(digest) = &signature.sha256 {
                validate_sha256(&format!("tool `{tool}` signature {index} digest"), digest)?;
            }
        }
        validate_extensions(
            &format!("tool `{tool}` artifact extensions"),
            &self.extensions,
        )?;
        Ok(())
    }

    fn normalize(&mut self) {
        self.sha256.make_ascii_lowercase();
        self.mirrors.sort();
        self.mirrors.dedup();
        for signature in &mut self.signatures {
            if let Some(digest) = &mut signature.sha256 {
                digest.make_ascii_lowercase();
            }
        }
        self.signatures.sort();
        self.signatures.dedup();
    }
}

impl LockedPlatform {
    fn validate(&self, tool: &str) -> Result<(), EnvironmentLockError> {
        validate_text(&format!("tool `{tool}` platform target"), &self.target)?;
        for (field, value) in [
            ("os", &self.os),
            ("arch", &self.arch),
            ("libc", &self.libc),
            ("abi", &self.abi),
        ] {
            if let Some(value) = value {
                validate_text(&format!("tool `{tool}` platform {field}"), value)?;
            }
        }
        Ok(())
    }
}

impl LockedInstall {
    fn validate(&self, tool: &str, variant: &str) -> Result<(), EnvironmentLockError> {
        validate_relative_path(&format!("tool `{tool}` install root"), &self.root, true)?;
        for (index, path) in self.bin_dirs.iter().enumerate() {
            validate_relative_path(&format!("tool `{tool}` bin directory {index}"), path, true)?;
        }
        if let Some(digest) = &self.layout_digest_sha256 {
            validate_sha256(&format!("tool `{tool}` layout digest"), digest)?;
        }
        validate_extensions(
            &format!("tool `{tool}` install extensions"),
            &self.extensions,
        )?;

        let mut names = BTreeSet::new();
        for executable in &self.executables {
            validate_executable_name(tool, variant, &executable.name)?;
            if !names.insert(portable_executable_key(&executable.name)) {
                return Err(EnvironmentLockError::ExecutableCollision {
                    tool: tool.to_string(),
                    variant: variant.to_string(),
                    name: executable.name.clone(),
                });
            }
            validate_relative_path(
                &format!("tool `{tool}` executable `{}`", executable.name),
                &executable.path,
                false,
            )?;
            for alias in &executable.aliases {
                validate_executable_name(tool, variant, alias)?;
                if !names.insert(portable_executable_key(alias)) {
                    return Err(EnvironmentLockError::ExecutableCollision {
                        tool: tool.to_string(),
                        variant: variant.to_string(),
                        name: alias.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    fn normalize(&mut self) {
        self.root = portable_path(&self.root);
        for path in &mut self.bin_dirs {
            *path = portable_path(path);
        }
        self.bin_dirs.sort();
        self.bin_dirs.dedup();
        for executable in &mut self.executables {
            executable.path = portable_path(&executable.path);
            executable.aliases.sort();
            executable.aliases.dedup();
        }
        self.executables
            .sort_by(|left, right| left.name.cmp(&right.name).then(left.path.cmp(&right.path)));
        if let Some(digest) = &mut self.layout_digest_sha256 {
            digest.make_ascii_lowercase();
        }
    }
}

fn validate_executable_name(
    tool: &str,
    variant: &str,
    name: &str,
) -> Result<(), EnvironmentLockError> {
    let valid = !name.trim().is_empty()
        && name == name.trim()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\'])
        && !name.chars().any(char::is_control);
    if valid {
        Ok(())
    } else {
        Err(EnvironmentLockError::InvalidExecutableName {
            tool: tool.to_string(),
            variant: variant.to_string(),
            name: name.to_string(),
        })
    }
}

fn portable_executable_key(name: &str) -> String {
    let lower = name.to_ascii_lowercase();
    for suffix in [".exe", ".cmd", ".bat", ".com"] {
        if let Some(stem) = lower.strip_suffix(suffix)
            && !stem.is_empty()
        {
            return stem.to_string();
        }
    }
    lower
}

fn validate_source_locator(
    field: &str,
    value: &str,
    kind: LockedSourceKind,
) -> Result<(), EnvironmentLockError> {
    if kind == LockedSourceKind::Path || kind == LockedSourceKind::Registry {
        return Ok(());
    }
    validate_network_locator(field, value, kind == LockedSourceKind::Vcs)
}

fn validate_network_locator(
    field: &str,
    value: &str,
    allow_git_user: bool,
) -> Result<(), EnvironmentLockError> {
    if value.contains('?') || value.contains('#') {
        return Err(EnvironmentLockError::UnsafeLocator {
            field: field.to_string(),
            value: value.to_string(),
        });
    }

    if let Some((_, remainder)) = value.split_once("://") {
        let authority = remainder.split('/').next().unwrap_or(remainder);
        if let Some((userinfo, _)) = authority.rsplit_once('@') {
            let allowed = allow_git_user && userinfo == "git";
            if !allowed {
                return Err(EnvironmentLockError::UnsafeLocator {
                    field: field.to_string(),
                    value: value.to_string(),
                });
            }
        }
    }
    Ok(())
}

fn validate_text(field: &str, value: &str) -> Result<(), EnvironmentLockError> {
    if value.trim().is_empty() {
        return Err(EnvironmentLockError::EmptyField {
            field: field.to_string(),
        });
    }
    if value.chars().any(char::is_control) {
        return Err(EnvironmentLockError::ControlCharacter {
            field: field.to_string(),
        });
    }
    Ok(())
}

fn validate_sha256(field: &str, value: &str) -> Result<(), EnvironmentLockError> {
    if valid_sha256(value) {
        Ok(())
    } else {
        Err(EnvironmentLockError::InvalidSha256 {
            field: field.to_string(),
        })
    }
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_prefixed_sha256(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(valid_sha256)
}

fn validate_relative_path(
    field: &str,
    value: &str,
    allow_dot: bool,
) -> Result<(), EnvironmentLockError> {
    let value = value.trim();
    let windows_drive = value.as_bytes().get(1).is_some_and(|byte| *byte == b':')
        && value
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic);
    let has_parent = value.split(['/', '\\']).any(|part| part == "..");
    let dot_is_invalid = value == "." && !allow_dot;
    let unsafe_path = value.is_empty()
        || dot_is_invalid
        || Path::new(value).is_absolute()
        || windows_drive
        || value.starts_with('~')
        || value.starts_with("$HOME")
        || value.starts_with("${HOME}")
        || value.starts_with("%USERPROFILE%")
        || value.starts_with("//")
        || value.starts_with("\\\\")
        || has_parent
        || value.chars().any(char::is_control);
    if unsafe_path {
        Err(EnvironmentLockError::UnsafeRelativePath {
            field: field.to_string(),
            path: value.to_string(),
        })
    } else {
        Ok(())
    }
}

fn portable_path(value: &str) -> String {
    value.replace('\\', "/")
}

fn validate_extensions(
    field: &str,
    extensions: &BTreeMap<String, serde_json::Value>,
) -> Result<(), EnvironmentLockError> {
    for (key, value) in extensions {
        validate_text(&format!("{field} key"), key)?;
        validate_extension_value(&format!("{field}.{key}"), value)?;
    }
    serde_json::to_vec(extensions).map_err(|error| EnvironmentLockError::JsonSerialize {
        message: format!("{field}: {error}"),
    })?;
    Ok(())
}

fn validate_extension_value(
    path: &str,
    value: &serde_json::Value,
) -> Result<(), EnvironmentLockError> {
    match value {
        serde_json::Value::Null => Err(EnvironmentLockError::NullExtension {
            path: path.to_string(),
        }),
        serde_json::Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                validate_extension_value(&format!("{path}[{index}]"), value)?;
            }
            Ok(())
        }
        serde_json::Value::Object(values) => {
            for (key, value) in values {
                validate_text(&format!("{path} key"), key)?;
                validate_extension_value(&format!("{path}.{key}"), value)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn looks_floating(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    value.is_empty()
        || matches!(
            value.as_str(),
            "latest"
                | "stable"
                | "current"
                | "system"
                | "present"
                | "head"
                | "main"
                | "master"
                | "nightly"
                | "canary"
                | "beta"
                | "alpha"
                | "lts"
        )
        || value.contains('*')
        || value.ends_with(".x")
        || value.starts_with(['^', '~', '>', '<', '='])
        || value.starts_with("lts/")
        || value.starts_with("prefix:")
        || value.starts_with("path:")
        || value.starts_with("env:")
        || value.starts_with("ref:main")
        || value.starts_with("ref:master")
        || value.contains(" || ")
        || value.contains(" && ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

    fn registry_tool(version: &str, target: &str) -> LockedTool {
        LockedTool {
            requirement: "22".to_string(),
            resolved: version.to_string(),
            backend: "core".to_string(),
            backend_version: Some("1.0.0".to_string()),
            backend_options_digest_sha256: Some(A.to_string()),
            source: LockedSource {
                kind: LockedSourceKind::Registry,
                locator: "core:node".to_string(),
                revision: Some(version.to_string()),
                tree_sha256: None,
                immutable: true,
                portable: false,
                extensions: BTreeMap::new(),
            },
            artifact: LockedArtifact {
                sha256: B.to_string(),
                size: 42,
                format: LockedArtifactFormat::TarGz,
                mirrors: vec![
                    "https://mirror-b.invalid/node.tgz".to_string(),
                    "https://mirror-a.invalid/node.tgz".to_string(),
                ],
                signatures: vec![LockedSignature {
                    kind: "minisign".to_string(),
                    identity: "node-release-key".to_string(),
                    sha256: Some(A.to_string()),
                }],
                extensions: BTreeMap::new(),
            },
            platform: LockedPlatform {
                target: target.to_string(),
                os: Some("linux".to_string()),
                arch: Some("x86_64".to_string()),
                libc: Some("gnu".to_string()),
                abi: None,
            },
            install: LockedInstall {
                root: ".".to_string(),
                bin_dirs: vec!["bin".to_string()],
                executables: vec![LockedExecutable {
                    name: "node".to_string(),
                    path: "bin/node".to_string(),
                    aliases: vec!["nodejs".to_string()],
                }],
                layout_digest_sha256: Some(A.to_string()),
                extensions: BTreeMap::new(),
            },
            extensions: BTreeMap::new(),
        }
    }

    fn lock_with(tool: LockedTool) -> EnvironmentLock {
        EnvironmentLock {
            schema_version: ENVIRONMENT_LOCK_SCHEMA_VERSION,
            plan_digest_sha256: A.to_string(),
            tools: BTreeMap::from([("node".to_string(), vec![tool])]),
            extensions: BTreeMap::new(),
        }
    }

    #[test]
    fn exact_registry_lock_is_portable() {
        let lock = lock_with(registry_tool("22.4.0", "x86_64-unknown-linux-gnu"));
        assert_eq!(
            lock.validate(EnvironmentLockValidationMode::Portable),
            Ok(())
        );
    }

    #[test]
    fn moving_resolved_version_is_rejected() {
        let lock = lock_with(registry_tool("latest", "x86_64-unknown-linux-gnu"));
        assert!(matches!(
            lock.validate(EnvironmentLockValidationMode::Portable),
            Err(EnvironmentLockError::FloatingResolvedVersion { .. })
        ));
    }

    #[test]
    fn mutable_vcs_source_is_rejected() {
        let mut tool = registry_tool("1.2.3", "x86_64-unknown-linux-gnu");
        tool.source = LockedSource {
            kind: LockedSourceKind::Vcs,
            locator: "https://github.com/acme/tool".to_string(),
            revision: Some("main".to_string()),
            tree_sha256: None,
            immutable: false,
            portable: false,
            extensions: BTreeMap::new(),
        };
        let lock = lock_with(tool);
        assert!(matches!(
            lock.validate(EnvironmentLockValidationMode::Portable),
            Err(EnvironmentLockError::MutableSource { .. })
        ));
    }

    #[test]
    fn digest_pinned_http_source_is_exact() {
        let mut tool = registry_tool("1.2.3", "aarch64-apple-darwin");
        tool.source = LockedSource {
            kind: LockedSourceKind::Http,
            locator: "https://example.invalid/tool.tar.gz".to_string(),
            revision: None,
            tree_sha256: None,
            immutable: false,
            portable: false,
            extensions: BTreeMap::new(),
        };
        let lock = lock_with(tool);
        assert_eq!(
            lock.validate(EnvironmentLockValidationMode::Portable),
            Ok(())
        );
    }

    #[test]
    fn local_tree_requires_explicit_portability() {
        let mut tool = registry_tool("1.2.3", "x86_64-unknown-linux-gnu");
        tool.source = LockedSource {
            kind: LockedSourceKind::Path,
            locator: "vendor/tool".to_string(),
            revision: None,
            tree_sha256: Some(A.to_string()),
            immutable: false,
            portable: false,
            extensions: BTreeMap::new(),
        };
        tool.artifact.format = LockedArtifactFormat::Directory;
        let lock = lock_with(tool);
        assert_eq!(lock.validate(EnvironmentLockValidationMode::Local), Ok(()));
        assert!(matches!(
            lock.validate(EnvironmentLockValidationMode::Portable),
            Err(EnvironmentLockError::LocalSourceNotPortable { .. })
        ));

        let mut portable = lock.clone();
        portable.tools.get_mut("node").unwrap()[0].source.portable = true;
        assert_eq!(
            portable.validate(EnvironmentLockValidationMode::Portable),
            Ok(())
        );
    }

    #[test]
    fn unsafe_install_paths_are_rejected_cross_platform() {
        for path in ["../bin", "/usr/bin", r"C:\\tool\\bin", r"\\\\server\\share"] {
            let mut tool = registry_tool("22.4.0", "x86_64-unknown-linux-gnu");
            tool.install.executables[0].path = path.to_string();
            let lock = lock_with(tool);
            assert!(matches!(
                lock.validate(EnvironmentLockValidationMode::Portable),
                Err(EnvironmentLockError::UnsafeRelativePath { .. })
            ));
        }
    }

    #[test]
    fn executable_alias_collisions_are_rejected() {
        let mut tool = registry_tool("22.4.0", "x86_64-unknown-linux-gnu");
        tool.install.executables.push(LockedExecutable {
            name: "npm".to_string(),
            path: "bin/npm".to_string(),
            aliases: vec!["nodejs".to_string()],
        });
        let lock = lock_with(tool);
        assert!(matches!(
            lock.validate(EnvironmentLockValidationMode::Portable),
            Err(EnvironmentLockError::ExecutableCollision { .. })
        ));
    }

    #[test]
    fn duplicate_backend_version_target_variants_are_rejected() {
        let tool = registry_tool("22.4.0", "x86_64-unknown-linux-gnu");
        let mut lock = lock_with(tool.clone());
        let mut duplicate = tool;
        duplicate.artifact.sha256 = A.to_string();
        lock.tools.get_mut("node").unwrap().push(duplicate);
        assert!(matches!(
            lock.validate(EnvironmentLockValidationMode::Portable),
            Err(EnvironmentLockError::DuplicateVariant { .. })
        ));
    }

    #[test]
    fn normalization_ignores_set_and_variant_insertion_order() {
        let mut first = EnvironmentLock {
            schema_version: ENVIRONMENT_LOCK_SCHEMA_VERSION,
            plan_digest_sha256: A.to_ascii_uppercase(),
            tools: BTreeMap::from([(
                "node".to_string(),
                vec![
                    registry_tool("22.4.0", "x86_64-unknown-linux-gnu"),
                    registry_tool("22.4.0", "aarch64-apple-darwin"),
                ],
            )]),
            extensions: BTreeMap::new(),
        };
        first.tools.get_mut("node").unwrap()[0]
            .artifact
            .mirrors
            .reverse();
        first.tools.get_mut("node").unwrap()[0]
            .install
            .bin_dirs
            .extend(["libexec".to_string(), "bin".to_string()]);

        let mut second = first.clone();
        second.tools.get_mut("node").unwrap().reverse();
        second.tools.get_mut("node").unwrap()[1]
            .artifact
            .mirrors
            .reverse();

        assert_eq!(
            first.normalized_digest_sha256().unwrap(),
            second.normalized_digest_sha256().unwrap()
        );
    }

    #[test]
    fn plan_digest_mismatch_is_explicit() {
        let lock = lock_with(registry_tool("22.4.0", "x86_64-unknown-linux-gnu"));
        assert!(matches!(
            lock.verify_plan_digest(B),
            Err(EnvironmentLockError::PlanDigestMismatch { .. })
        ));
    }

    #[test]
    fn variants_are_selected_by_exact_target() {
        let lock = EnvironmentLock {
            schema_version: ENVIRONMENT_LOCK_SCHEMA_VERSION,
            plan_digest_sha256: A.to_string(),
            tools: BTreeMap::from([(
                "node".to_string(),
                vec![
                    registry_tool("22.4.0", "x86_64-unknown-linux-gnu"),
                    registry_tool("22.4.0", "aarch64-apple-darwin"),
                ],
            )]),
            extensions: BTreeMap::new(),
        };
        let selected: Vec<_> = lock
            .variants_for_target("node", "aarch64-apple-darwin")
            .collect();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].platform.target, "aarch64-apple-darwin");
    }

    #[test]
    fn oci_source_requires_digest_revision() {
        let mut tool = registry_tool("1.2.3", "x86_64-unknown-linux-gnu");
        tool.source = LockedSource {
            kind: LockedSourceKind::Oci,
            locator: "ghcr.io/acme/tool".to_string(),
            revision: Some(format!("sha256:{A}")),
            tree_sha256: None,
            immutable: true,
            portable: false,
            extensions: BTreeMap::new(),
        };
        assert_eq!(
            lock_with(tool).validate(EnvironmentLockValidationMode::Portable),
            Ok(())
        );
    }

    #[test]
    fn toml_and_json_round_trip() {
        let lock = lock_with(registry_tool("22.4.0", "x86_64-unknown-linux-gnu"));
        let toml = lock.to_toml_string().unwrap();
        assert_eq!(
            EnvironmentLock::parse_toml(&toml).unwrap(),
            lock.normalized()
        );
        let json = lock.canonical_json_string().unwrap();
        assert_eq!(
            EnvironmentLock::parse_json(&json).unwrap(),
            lock.normalized()
        );
    }

    #[test]
    fn credential_bearing_and_signed_urls_are_rejected() {
        let mut credential = registry_tool("1.2.3", "x86_64-unknown-linux-gnu");
        credential.source = LockedSource {
            kind: LockedSourceKind::Http,
            locator: "https://user:placeholder@example.invalid/tool.tar.gz".to_string(),
            revision: None,
            tree_sha256: None,
            immutable: false,
            portable: false,
            extensions: BTreeMap::new(),
        };
        assert!(matches!(
            lock_with(credential).validate(EnvironmentLockValidationMode::Portable),
            Err(EnvironmentLockError::UnsafeLocator { .. })
        ));

        let mut signed = registry_tool("1.2.3", "x86_64-unknown-linux-gnu");
        signed.artifact.mirrors =
            vec!["https://example.invalid/tool.tar.gz?X-Signature=placeholder".to_string()];
        assert!(matches!(
            lock_with(signed).validate(EnvironmentLockValidationMode::Portable),
            Err(EnvironmentLockError::UnsafeLocator { .. })
        ));
    }

    #[test]
    fn source_and_artifact_format_must_agree() {
        let mut local = registry_tool("1.2.3", "x86_64-unknown-linux-gnu");
        local.source = LockedSource {
            kind: LockedSourceKind::Path,
            locator: "vendor/tool".to_string(),
            revision: None,
            tree_sha256: Some(A.to_string()),
            immutable: false,
            portable: true,
            extensions: BTreeMap::new(),
        };
        assert!(matches!(
            lock_with(local).validate(EnvironmentLockValidationMode::Portable),
            Err(EnvironmentLockError::SourceArtifactMismatch { .. })
        ));

        let mut remote_directory = registry_tool("1.2.3", "x86_64-unknown-linux-gnu");
        remote_directory.artifact.format = LockedArtifactFormat::Directory;
        assert!(matches!(
            lock_with(remote_directory).validate(EnvironmentLockValidationMode::Portable),
            Err(EnvironmentLockError::SourceArtifactMismatch { .. })
        ));
    }

    #[test]
    fn executable_collisions_follow_windows_command_semantics() {
        let mut tool = registry_tool("22.4.0", "x86_64-pc-windows-msvc");
        tool.install.executables.push(LockedExecutable {
            name: "Node.EXE".to_string(),
            path: "bin/Node.EXE".to_string(),
            aliases: Vec::new(),
        });
        assert!(matches!(
            lock_with(tool).validate(EnvironmentLockValidationMode::Portable),
            Err(EnvironmentLockError::ExecutableCollision { .. })
        ));
    }

    #[test]
    fn null_extension_values_are_rejected_recursively() {
        let mut lock = lock_with(registry_tool("22.4.0", "x86_64-unknown-linux-gnu"));
        lock.extensions.insert(
            "future".to_string(),
            serde_json::json!({"nested": [1, null]}),
        );
        assert!(matches!(
            lock.validate(EnvironmentLockValidationMode::Portable),
            Err(EnvironmentLockError::NullExtension { .. })
        ));
    }

    #[test]
    fn malformed_digest_is_rejected() {
        let mut lock = lock_with(registry_tool("22.4.0", "x86_64-unknown-linux-gnu"));
        lock.plan_digest_sha256 = "sha256:not-a-digest".to_string();
        assert!(matches!(
            lock.validate(EnvironmentLockValidationMode::Portable),
            Err(EnvironmentLockError::InvalidSha256 { .. })
        ));
    }
}
