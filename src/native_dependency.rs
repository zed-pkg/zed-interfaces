//! Lossless native-registry requirement translation and exact dependency locks.
//!
//! Native ecosystems do not assign identical semantics to every identical-
//! looking requirement. In npm, `1.2.3` is exact and `1.2` is an x-range. In
//! Cargo, both are caret-compatible requirements. Zed therefore records the
//! source registry, original declaration, and deterministic canonical SemVer
//! requirement before binding one exact resolved version to immutable artifact
//! identity.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use semver::{BuildMetadata, Version, VersionReq};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{NativeArtifact, NativePackageIdentity, NativeRegistry};

/// Current schema for one exact native dependency resolution.
pub const NATIVE_DEPENDENCY_LOCK_SCHEMA_V1: &str = "zed.native-dependency-lock/v1";

const MAX_REQUIREMENT_LEN: usize = 512;
const NPM_MAX_SAFE_COMPONENT: u64 = 9_007_199_254_740_991;

/// One source-aware native requirement and its canonical SemVer translation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NativeVersionRequirement {
    pub registry: NativeRegistry,
    /// Exact project declaration before translation.
    pub declared: String,
    /// Deterministic `semver::VersionReq` representation used by Zed.
    pub canonical: String,
}

impl NativeVersionRequirement {
    /// Translate the lossless v1 subset for npm or Cargo.
    pub fn parse(
        registry: NativeRegistry,
        declared: impl Into<String>,
    ) -> Result<Self, NativeDependencyError> {
        let declared = declared.into();
        validate_requirement_input(registry, &declared)?;
        let requirement = match registry {
            NativeRegistry::Npm => translate_npm_requirement(&declared)?,
            NativeRegistry::Cargo => translate_cargo_requirement(&declared)?,
        };
        Ok(Self {
            registry,
            declared,
            canonical: requirement.to_string(),
        })
    }

    /// Recompute the canonical requirement so serialized translation receipts
    /// cannot be edited independently from their source declaration.
    pub fn validate(&self) -> Result<(), NativeDependencyError> {
        let translated = Self::parse(self.registry, self.declared.clone())?;
        if translated.canonical != self.canonical {
            return Err(NativeDependencyError::CanonicalRequirementDrift {
                declared: self.declared.clone(),
                expected: translated.canonical,
                found: self.canonical.clone(),
            });
        }
        Ok(())
    }

    /// Test one strict native version against the translated requirement.
    pub fn matches(&self, version: &str) -> Result<bool, NativeDependencyError> {
        self.validate()?;
        let requirement = parse_canonical_requirement(self.registry, &self.canonical)?;
        let version = parse_native_version(self.registry, "version", version)?;
        Ok(requirement.matches(&version))
    }

    fn parsed(&self) -> Result<VersionReq, NativeDependencyError> {
        self.validate()?;
        parse_canonical_requirement(self.registry, &self.canonical)
    }
}

/// One eligible version and immutable artifact returned by a native registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NativeVersionCandidate {
    pub version: String,
    pub artifact: NativeArtifact,
}

/// Frozen native dependency identity. The declaration remains auditable, while
/// `package.version` and `artifact` are the only restore-time identities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NativeDependencyLock {
    pub schema: String,
    pub requirement: NativeVersionRequirement,
    pub package: NativePackageIdentity,
    pub artifact: NativeArtifact,
}

impl NativeDependencyLock {
    pub const SCHEMA_V1: &'static str = NATIVE_DEPENDENCY_LOCK_SCHEMA_V1;

    /// Resolve the highest satisfying candidate independent of candidate
    /// presentation order. Callers must prefilter registry policy such as
    /// yanked versions; this function validates every supplied identity.
    pub fn resolve(
        registry: NativeRegistry,
        package_name: impl Into<String>,
        declared_requirement: impl Into<String>,
        candidates: &[NativeVersionCandidate],
    ) -> Result<Self, NativeDependencyError> {
        let package_name = package_name.into();
        validate_native_package_name(registry, &package_name)?;
        let requirement = NativeVersionRequirement::parse(registry, declared_requirement)?;
        let parsed_requirement = requirement.parsed()?;

        let mut seen = BTreeSet::new();
        let mut selected: Option<(&NativeVersionCandidate, Version)> = None;
        for candidate in candidates {
            let version = parse_native_version(registry, "candidate.version", &candidate.version)?;
            validate_native_artifact(&candidate.artifact, "candidate.artifact")?;
            if !seen.insert(version.clone()) {
                return Err(NativeDependencyError::DuplicateCandidateVersion {
                    version: candidate.version.clone(),
                });
            }
            if parsed_requirement.matches(&version)
                && selected
                    .as_ref()
                    .is_none_or(|(_, current)| version > *current)
            {
                selected = Some((candidate, version));
            }
        }

        let (candidate, _) = selected.ok_or_else(|| NativeDependencyError::NoMatchingVersion {
            registry,
            package: package_name.clone(),
            requirement: requirement.canonical.clone(),
        })?;

        let lock = Self {
            schema: Self::SCHEMA_V1.to_string(),
            requirement,
            package: NativePackageIdentity {
                name: package_name,
                version: candidate.version.clone(),
            },
            artifact: candidate.artifact.clone(),
        };
        lock.validate()?;
        Ok(lock)
    }

    pub fn validate(&self) -> Result<(), NativeDependencyError> {
        if self.schema != Self::SCHEMA_V1 {
            return Err(NativeDependencyError::UnsupportedSchema {
                found: self.schema.clone(),
                supported: Self::SCHEMA_V1.to_string(),
            });
        }
        self.requirement.validate()?;
        validate_native_package_name(self.requirement.registry, &self.package.name)?;
        let resolved = parse_native_version(
            self.requirement.registry,
            "package.version",
            &self.package.version,
        )?;
        let requirement = self.requirement.parsed()?;
        if !requirement.matches(&resolved) {
            return Err(NativeDependencyError::ResolvedVersionDoesNotMatch {
                package: self.package.name.clone(),
                version: self.package.version.clone(),
                requirement: self.requirement.canonical.clone(),
            });
        }
        validate_native_artifact(&self.artifact, "artifact")
    }

    /// Stable compact JSON for lock provenance, signatures, and generated
    /// client fixtures.
    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>, NativeDependencyError> {
        self.validate()?;
        serde_json::to_vec(self)
            .map_err(|error| NativeDependencyError::Serialization(error.to_string()))
    }
}

fn validate_requirement_input(
    registry: NativeRegistry,
    declared: &str,
) -> Result<(), NativeDependencyError> {
    if declared.is_empty() {
        return Err(NativeDependencyError::EmptyRequirement { registry });
    }
    if declared.len() > MAX_REQUIREMENT_LEN {
        return Err(NativeDependencyError::RequirementTooLong {
            registry,
            length: declared.len(),
            maximum: MAX_REQUIREMENT_LEN,
        });
    }
    if declared != declared.trim() {
        return Err(NativeDependencyError::SurroundingWhitespace {
            registry,
            declared: declared.to_string(),
        });
    }
    if declared.chars().any(char::is_control) {
        return Err(NativeDependencyError::ControlCharacter {
            registry,
            declared: declared.to_string(),
        });
    }
    if declared.contains("||") {
        return Err(unsupported(
            registry,
            declared,
            "logical unions are outside the lossless v1 subset",
        ));
    }
    if declared.split_whitespace().any(|token| token == "-") {
        return Err(unsupported(
            registry,
            declared,
            "hyphen ranges are outside the lossless v1 subset",
        ));
    }
    if declared.contains('+') {
        return Err(NativeDependencyError::BuildMetadataNotAllowed {
            field: "declared".to_string(),
            version: declared.to_string(),
        });
    }

    let lower = declared.to_ascii_lowercase();
    let source_prefixes = [
        "workspace:",
        "file:",
        "link:",
        "git:",
        "git+",
        "http:",
        "https:",
        "ssh:",
        "github:",
        "npm:",
    ];
    if declared.contains("://")
        || source_prefixes
            .iter()
            .any(|prefix| lower.starts_with(prefix))
    {
        return Err(unsupported(
            registry,
            declared,
            "source protocols, aliases, and workspace requirements are not SemVer ranges",
        ));
    }
    Ok(())
}

fn translate_npm_requirement(declared: &str) -> Result<VersionReq, NativeDependencyError> {
    if declared.contains(',') {
        return Err(unsupported(
            NativeRegistry::Npm,
            declared,
            "npm comparator intersections use whitespace, not Cargo commas, in strict v1",
        ));
    }
    let normalized = coalesce_npm_tokens(declared)?
        .iter()
        .map(|token| normalize_npm_token(token))
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");
    parse_requirement(NativeRegistry::Npm, declared, &normalized)
}

fn coalesce_npm_tokens(declared: &str) -> Result<Vec<String>, NativeDependencyError> {
    let mut normalized = Vec::new();
    let mut words = declared.split_whitespace();
    while let Some(word) = words.next() {
        if is_operator_token(word) {
            let body = words.next().ok_or_else(|| {
                invalid_requirement(
                    NativeRegistry::Npm,
                    declared,
                    "missing version after comparator",
                )
            })?;
            if !split_operator(body).0.is_empty() {
                return Err(invalid_requirement(
                    NativeRegistry::Npm,
                    declared,
                    "multiple comparator operators may not be separated by whitespace",
                ));
            }
            normalized.push(format!("{word}{body}"));
        } else {
            normalized.push(word.to_string());
        }
    }
    Ok(normalized)
}

fn normalize_npm_token(token: &str) -> Result<String, NativeDependencyError> {
    let (operator, body) = split_operator(token);
    if body.is_empty() {
        return Err(invalid_requirement(
            NativeRegistry::Npm,
            token,
            "missing version after comparator",
        ));
    }
    let body = strip_numeric_v_prefix(body);
    let body = normalize_x_components(body);

    if operator.is_empty() || operator == "=" {
        return normalize_npm_bare(&body).ok_or_else(|| {
            invalid_requirement(
                NativeRegistry::Npm,
                token,
                "expected an exact version, partial version, or wildcard",
            )
        });
    }
    normalize_npm_comparator(operator, &body, token)
}

fn normalize_npm_bare(body: &str) -> Option<String> {
    if let Ok(version) = Version::parse(body) {
        if version.build != BuildMetadata::EMPTY || !npm_components_supported(&version) {
            return None;
        }
        return Some(format!("={version}"));
    }

    let partial = parse_npm_partial(body)?;
    match partial.components.as_slice() {
        [] if partial.wildcard => Some("*".to_string()),
        [major] => Some(format!("{major}.*")),
        [major, minor] => Some(format!("{major}.{minor}.*")),
        [major, minor, patch] if !partial.wildcard => Some(format!("={major}.{minor}.{patch}")),
        _ => None,
    }
}

fn normalize_npm_comparator(
    operator: &str,
    body: &str,
    declared: &str,
) -> Result<String, NativeDependencyError> {
    if let Ok(version) = Version::parse(body) {
        if version.build != BuildMetadata::EMPTY {
            return Err(NativeDependencyError::BuildMetadataNotAllowed {
                field: "declared".to_string(),
                version: declared.to_string(),
            });
        }
        if !npm_components_supported(&version) {
            return Err(invalid_requirement(
                NativeRegistry::Npm,
                declared,
                "npm numeric components must not exceed Number.MAX_SAFE_INTEGER",
            ));
        }
        return Ok(format!("{operator}{version}"));
    }

    let partial = parse_npm_partial(body).ok_or_else(|| {
        invalid_requirement(
            NativeRegistry::Npm,
            declared,
            "expected a strict or partial numeric version after comparator",
        )
    })?;
    if partial.components.is_empty() {
        return Err(unsupported(
            NativeRegistry::Npm,
            declared,
            "comparators against an unconstrained wildcard are outside strict v1",
        ));
    }

    match operator {
        "^" | "~" => Ok(format!("{operator}{}", partial.numeric_prefix())),
        ">=" => Ok(format!(">={}", partial.lower_bound())),
        "<" => Ok(format!("<{}", partial.lower_bound())),
        ">" => Ok(format!(">={}", partial.next_prefix(declared)?)),
        "<=" => Ok(format!("<{}", partial.next_prefix(declared)?)),
        _ => Err(invalid_requirement(
            NativeRegistry::Npm,
            declared,
            "unsupported comparator operator",
        )),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NpmPartialVersion {
    components: Vec<u64>,
    wildcard: bool,
}

impl NpmPartialVersion {
    fn numeric_prefix(&self) -> String {
        self.components
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(".")
    }

    fn lower_bound(&self) -> String {
        match self.components.as_slice() {
            [major] => format!("{major}.0.0"),
            [major, minor] => format!("{major}.{minor}.0"),
            [major, minor, patch] => format!("{major}.{minor}.{patch}"),
            _ => unreachable!("validated npm partial has one to three numeric components"),
        }
    }

    fn next_prefix(&self, declared: &str) -> Result<String, NativeDependencyError> {
        match self.components.as_slice() {
            [major] => Ok(format!(
                "{}.0.0",
                increment_npm_component(*major, declared)?
            )),
            [major, minor] => Ok(format!(
                "{major}.{}.0",
                increment_npm_component(*minor, declared)?
            )),
            [major, minor, patch] if !self.wildcard => Ok(format!(
                "{major}.{minor}.{}",
                increment_npm_component(*patch, declared)?
            )),
            _ => Err(invalid_requirement(
                NativeRegistry::Npm,
                declared,
                "cannot advance this partial comparator without changing its meaning",
            )),
        }
    }
}

fn parse_npm_partial(body: &str) -> Option<NpmPartialVersion> {
    let parts: Vec<&str> = body.split('.').collect();
    if parts.is_empty() || parts.len() > 3 {
        return None;
    }

    let mut components = Vec::new();
    let mut wildcard = false;
    for part in parts {
        if part == "*" {
            wildcard = true;
            continue;
        }
        if wildcard
            || part.is_empty()
            || !part.bytes().all(|byte| byte.is_ascii_digit())
            || (part.len() > 1 && part.starts_with('0'))
        {
            return None;
        }
        let component: u64 = part.parse().ok()?;
        if component > NPM_MAX_SAFE_COMPONENT {
            return None;
        }
        components.push(component);
    }

    Some(NpmPartialVersion {
        components,
        wildcard,
    })
}

fn increment_npm_component(component: u64, declared: &str) -> Result<u64, NativeDependencyError> {
    let incremented = component.checked_add(1).ok_or_else(|| {
        invalid_requirement(
            NativeRegistry::Npm,
            declared,
            "partial comparator component overflows SemVer",
        )
    })?;
    if incremented > NPM_MAX_SAFE_COMPONENT {
        return Err(invalid_requirement(
            NativeRegistry::Npm,
            declared,
            "partial comparator increment exceeds Number.MAX_SAFE_INTEGER",
        ));
    }
    Ok(incremented)
}

fn translate_cargo_requirement(declared: &str) -> Result<VersionReq, NativeDependencyError> {
    if cargo_contains_x_wildcard(declared) {
        return Err(unsupported(
            NativeRegistry::Cargo,
            declared,
            "Cargo wildcards use `*`; npm-style `x` and `X` are rejected",
        ));
    }

    let normalized = declared
        .split(',')
        .map(str::trim)
        .map(|segment| normalize_cargo_segment(declared, segment))
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");
    parse_requirement(NativeRegistry::Cargo, declared, &normalized)
}

fn normalize_cargo_segment(declared: &str, segment: &str) -> Result<String, NativeDependencyError> {
    if segment.is_empty() {
        return Err(invalid_requirement(
            NativeRegistry::Cargo,
            declared,
            "empty comparator in comma-separated requirement",
        ));
    }
    let words: Vec<&str> = segment.split_whitespace().collect();
    match words.as_slice() {
        [single] => Ok((*single).to_string()),
        [operator, body] if is_operator_token(operator) && split_operator(body).0.is_empty() => {
            Ok(format!("{operator}{body}"))
        }
        _ => Err(unsupported(
            NativeRegistry::Cargo,
            declared,
            "multiple Cargo comparators require commas; whitespace is only allowed between one operator and its version",
        )),
    }
}

fn is_operator_token(token: &str) -> bool {
    matches!(token, ">=" | "<=" | "^" | "~" | ">" | "<" | "=")
}

fn split_operator(token: &str) -> (&str, &str) {
    for operator in [">=", "<=", "^", "~", ">", "<", "="] {
        if let Some(body) = token.strip_prefix(operator) {
            return (operator, body);
        }
    }
    ("", token)
}

fn strip_numeric_v_prefix(body: &str) -> &str {
    body.strip_prefix('v')
        .filter(|rest| {
            rest.bytes()
                .next()
                .is_some_and(|byte| byte.is_ascii_digit())
        })
        .unwrap_or(body)
}

fn normalize_x_components(body: &str) -> String {
    body.split('.')
        .map(|part| {
            if part.eq_ignore_ascii_case("x") {
                "*"
            } else {
                part
            }
        })
        .collect::<Vec<_>>()
        .join(".")
}

fn cargo_contains_x_wildcard(declared: &str) -> bool {
    declared
        .split(|character: char| character.is_whitespace() || character == ',')
        .filter(|token| !token.is_empty())
        .any(|token| {
            let (_, body) = split_operator(token);
            let core = body.split('-').next().unwrap_or(body);
            core.split('.').any(|part| part.eq_ignore_ascii_case("x"))
        })
}

fn parse_requirement(
    registry: NativeRegistry,
    declared: &str,
    normalized: &str,
) -> Result<VersionReq, NativeDependencyError> {
    VersionReq::parse(normalized).map_err(|error| NativeDependencyError::InvalidRequirement {
        registry,
        declared: declared.to_string(),
        detail: error.to_string(),
    })
}

fn parse_canonical_requirement(
    registry: NativeRegistry,
    canonical: &str,
) -> Result<VersionReq, NativeDependencyError> {
    VersionReq::parse(canonical).map_err(|error| {
        NativeDependencyError::InvalidCanonicalRequirement {
            registry,
            canonical: canonical.to_string(),
            detail: error.to_string(),
        }
    })
}

fn parse_native_version(
    registry: NativeRegistry,
    field: &str,
    raw: &str,
) -> Result<Version, NativeDependencyError> {
    let version = parse_strict_version(field, raw)?;
    if registry == NativeRegistry::Npm && !npm_components_supported(&version) {
        return Err(NativeDependencyError::InvalidVersion {
            field: field.to_string(),
            version: raw.to_string(),
            detail: "npm numeric components must not exceed Number.MAX_SAFE_INTEGER".to_string(),
        });
    }
    Ok(version)
}

fn npm_components_supported(version: &Version) -> bool {
    version.major <= NPM_MAX_SAFE_COMPONENT
        && version.minor <= NPM_MAX_SAFE_COMPONENT
        && version.patch <= NPM_MAX_SAFE_COMPONENT
}

fn parse_strict_version(field: &str, raw: &str) -> Result<Version, NativeDependencyError> {
    let version = Version::parse(raw).map_err(|error| NativeDependencyError::InvalidVersion {
        field: field.to_string(),
        version: raw.to_string(),
        detail: error.to_string(),
    })?;
    if version.build != BuildMetadata::EMPTY {
        return Err(NativeDependencyError::BuildMetadataNotAllowed {
            field: field.to_string(),
            version: raw.to_string(),
        });
    }
    Ok(version)
}

fn validate_native_package_name(
    registry: NativeRegistry,
    name: &str,
) -> Result<(), NativeDependencyError> {
    match registry {
        NativeRegistry::Cargo => {
            if name.is_empty()
                || name.len() > 64
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            {
                return Err(NativeDependencyError::InvalidPackageName {
                    registry,
                    name: name.to_string(),
                    detail: "expected 1-64 ASCII alphanumeric, `-`, or `_` characters".to_string(),
                });
            }
        }
        NativeRegistry::Npm => validate_npm_package_name(name)?,
    }
    Ok(())
}

fn validate_npm_package_name(name: &str) -> Result<(), NativeDependencyError> {
    if name.is_empty() || name.len() > 214 || name.bytes().any(|byte| byte.is_ascii_uppercase()) {
        return Err(NativeDependencyError::InvalidPackageName {
            registry: NativeRegistry::Npm,
            name: name.to_string(),
            detail: "expected a lowercase npm name no longer than 214 bytes".to_string(),
        });
    }
    let components: Vec<&str> = if let Some(scoped) = name.strip_prefix('@') {
        let mut parts = scoped.split('/');
        let scope = parts.next().unwrap_or_default();
        let package = parts.next().unwrap_or_default();
        if scope.is_empty() || package.is_empty() || parts.next().is_some() {
            return Err(NativeDependencyError::InvalidPackageName {
                registry: NativeRegistry::Npm,
                name: name.to_string(),
                detail: "scoped names must use exactly `@scope/package`".to_string(),
            });
        }
        vec![scope, package]
    } else {
        if name.contains('/') {
            return Err(NativeDependencyError::InvalidPackageName {
                registry: NativeRegistry::Npm,
                name: name.to_string(),
                detail: "unscoped names may not contain `/`".to_string(),
            });
        }
        vec![name]
    };

    for component in components {
        let first =
            component
                .bytes()
                .next()
                .ok_or_else(|| NativeDependencyError::InvalidPackageName {
                    registry: NativeRegistry::Npm,
                    name: name.to_string(),
                    detail: "name components must not be empty".to_string(),
                })?;
        if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
            return Err(NativeDependencyError::InvalidPackageName {
                registry: NativeRegistry::Npm,
                name: name.to_string(),
                detail: "name components must start with a lowercase letter or digit".to_string(),
            });
        }
        if !component.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        }) {
            return Err(NativeDependencyError::InvalidPackageName {
                registry: NativeRegistry::Npm,
                name: name.to_string(),
                detail: "name components may contain lowercase letters, digits, `-`, `_`, or `.`"
                    .to_string(),
            });
        }
    }
    Ok(())
}

fn validate_native_artifact(
    artifact: &NativeArtifact,
    field: &str,
) -> Result<(), NativeDependencyError> {
    if artifact.sha256.len() != 64
        || !artifact
            .sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || artifact.sha256.bytes().all(|byte| byte == b'0')
    {
        return Err(NativeDependencyError::InvalidSha256 {
            field: format!("{field}.sha256"),
            value: artifact.sha256.clone(),
        });
    }
    if artifact.size == 0 {
        return Err(NativeDependencyError::EmptyArtifact {
            field: field.to_string(),
        });
    }
    Ok(())
}

fn unsupported(registry: NativeRegistry, declared: &str, reason: &str) -> NativeDependencyError {
    NativeDependencyError::UnsupportedRequirement {
        registry,
        declared: declared.to_string(),
        reason: reason.to_string(),
    }
}

fn invalid_requirement(
    registry: NativeRegistry,
    declared: &str,
    detail: &str,
) -> NativeDependencyError {
    NativeDependencyError::InvalidRequirement {
        registry,
        declared: declared.to_string(),
        detail: detail.to_string(),
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NativeDependencyError {
    #[error("unsupported native dependency lock schema `{found}`; expected `{supported}`")]
    UnsupportedSchema { found: String, supported: String },
    #[error("empty {registry:?} requirement is not reproducible")]
    EmptyRequirement { registry: NativeRegistry },
    #[error("{registry:?} requirement length {length} exceeds the {maximum}-byte limit")]
    RequirementTooLong {
        registry: NativeRegistry,
        length: usize,
        maximum: usize,
    },
    #[error("{registry:?} requirement must not contain surrounding whitespace: `{declared}`")]
    SurroundingWhitespace {
        registry: NativeRegistry,
        declared: String,
    },
    #[error("{registry:?} requirement contains a control character: `{declared}`")]
    ControlCharacter {
        registry: NativeRegistry,
        declared: String,
    },
    #[error("unsupported {registry:?} requirement `{declared}`: {reason}")]
    UnsupportedRequirement {
        registry: NativeRegistry,
        declared: String,
        reason: String,
    },
    #[error("invalid {registry:?} requirement `{declared}`: {detail}")]
    InvalidRequirement {
        registry: NativeRegistry,
        declared: String,
        detail: String,
    },
    #[error("invalid canonical {registry:?} requirement `{canonical}`: {detail}")]
    InvalidCanonicalRequirement {
        registry: NativeRegistry,
        canonical: String,
        detail: String,
    },
    #[error("canonical requirement drift for `{declared}`: expected `{expected}`, found `{found}`")]
    CanonicalRequirementDrift {
        declared: String,
        expected: String,
        found: String,
    },
    #[error("invalid strict SemVer `{version}` at `{field}`: {detail}")]
    InvalidVersion {
        field: String,
        version: String,
        detail: String,
    },
    #[error("SemVer build metadata is not allowed at `{field}` (`{version}`)")]
    BuildMetadataNotAllowed { field: String, version: String },
    #[error("invalid native package name `{name}` for {registry:?}: {detail}")]
    InvalidPackageName {
        registry: NativeRegistry,
        name: String,
        detail: String,
    },
    #[error("duplicate native candidate version `{version}`")]
    DuplicateCandidateVersion { version: String },
    #[error("no {registry:?} version of `{package}` satisfies `{requirement}`")]
    NoMatchingVersion {
        registry: NativeRegistry,
        package: String,
        requirement: String,
    },
    #[error("resolved native package `{package}@{version}` does not satisfy `{requirement}`")]
    ResolvedVersionDoesNotMatch {
        package: String,
        version: String,
        requirement: String,
    },
    #[error("invalid lowercase nonzero SHA-256 `{value}` at `{field}`")]
    InvalidSha256 { field: String, value: String },
    #[error("artifact at `{field}` must contain at least one byte")]
    EmptyArtifact { field: String },
    #[error("failed to serialize native dependency lock: {0}")]
    Serialization(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ArtifactFormat;

    fn artifact(digit: char) -> NativeArtifact {
        NativeArtifact {
            sha256: std::iter::repeat_n(digit, 64).collect(),
            size: 128,
            format: ArtifactFormat::TarGz,
        }
    }

    fn candidate(version: &str, digit: char) -> NativeVersionCandidate {
        NativeVersionCandidate {
            version: version.to_string(),
            artifact: artifact(digit),
        }
    }

    #[test]
    fn npm_bare_exact_and_cargo_default_caret_are_distinct() {
        let npm = NativeVersionRequirement::parse(NativeRegistry::Npm, "1.2.3").unwrap();
        let cargo = NativeVersionRequirement::parse(NativeRegistry::Cargo, "1.2.3").unwrap();
        assert_eq!(npm.canonical, "=1.2.3");
        assert_eq!(cargo.canonical, "^1.2.3");
        assert!(npm.matches("1.2.3").unwrap());
        assert!(!npm.matches("1.2.4").unwrap());
        assert!(cargo.matches("1.9.9").unwrap());
        assert!(!cargo.matches("2.0.0").unwrap());
    }

    #[test]
    fn npm_partial_versions_are_x_ranges_while_cargo_partials_are_caret() {
        let npm_minor = NativeVersionRequirement::parse(NativeRegistry::Npm, "1.2").unwrap();
        let cargo_minor = NativeVersionRequirement::parse(NativeRegistry::Cargo, "1.2").unwrap();
        let npm_major = NativeVersionRequirement::parse(NativeRegistry::Npm, "1").unwrap();
        let cargo_major = NativeVersionRequirement::parse(NativeRegistry::Cargo, "1").unwrap();

        assert_eq!(npm_minor.canonical, "1.2.*");
        assert_eq!(cargo_minor.canonical, "^1.2");
        assert_eq!(npm_major.canonical, "1.*");
        assert_eq!(cargo_major.canonical, "^1");
        assert!(!npm_minor.matches("1.3.0").unwrap());
        assert!(cargo_minor.matches("1.3.0").unwrap());
    }

    #[test]
    fn native_wildcards_and_comparator_intersections_are_source_aware() {
        let npm = NativeVersionRequirement::parse(NativeRegistry::Npm, ">=1.2.3 <2.0.0").unwrap();
        let cargo =
            NativeVersionRequirement::parse(NativeRegistry::Cargo, ">=1.2.3, <2.0.0").unwrap();
        let npm_x = NativeVersionRequirement::parse(NativeRegistry::Npm, "1.2.X").unwrap();

        assert_eq!(npm.canonical, ">=1.2.3, <2.0.0");
        assert_eq!(cargo.canonical, ">=1.2.3, <2.0.0");
        assert_eq!(npm_x.canonical, "1.2.*");
        assert!(npm.matches("1.9.0").unwrap());
        assert!(!cargo.matches("2.0.0").unwrap());
    }

    #[test]
    fn major_zero_and_prerelease_rules_follow_semver() {
        let cargo = NativeVersionRequirement::parse(NativeRegistry::Cargo, "0.2.3").unwrap();
        assert!(cargo.matches("0.2.9").unwrap());
        assert!(!cargo.matches("0.3.0").unwrap());

        let ordinary = NativeVersionRequirement::parse(NativeRegistry::Npm, "^1.2.3").unwrap();
        assert!(!ordinary.matches("1.3.0-beta.1").unwrap());
        let explicit =
            NativeVersionRequirement::parse(NativeRegistry::Npm, "1.3.0-beta.1").unwrap();
        assert!(explicit.matches("1.3.0-beta.1").unwrap());
    }

    #[test]
    fn npm_partial_comparators_follow_node_semver_boundaries() {
        let gt_major = NativeVersionRequirement::parse(NativeRegistry::Npm, ">1").unwrap();
        let gt_minor = NativeVersionRequirement::parse(NativeRegistry::Npm, ">1.2").unwrap();
        let lte_minor = NativeVersionRequirement::parse(NativeRegistry::Npm, "<=1.2").unwrap();
        let spaced =
            NativeVersionRequirement::parse(NativeRegistry::Npm, ">= 1.2.3 < 2.0.0").unwrap();

        assert_eq!(gt_major.canonical, ">=2.0.0");
        assert_eq!(gt_minor.canonical, ">=1.3.0");
        assert_eq!(lte_minor.canonical, "<1.3.0");
        assert_eq!(spaced.canonical, ">=1.2.3, <2.0.0");
        assert!(!gt_major.matches("1.99.99").unwrap());
        assert!(gt_major.matches("2.0.0").unwrap());
        assert!(!gt_minor.matches("1.2.999").unwrap());
        assert!(gt_minor.matches("1.3.0").unwrap());
        assert!(lte_minor.matches("1.2.999").unwrap());
        assert!(!lte_minor.matches("1.3.0").unwrap());
    }

    #[test]
    fn cargo_allows_operator_whitespace_but_still_requires_comma_intersections() {
        let requirement =
            NativeVersionRequirement::parse(NativeRegistry::Cargo, ">= 1.2, < 1.5").unwrap();
        assert_eq!(requirement.canonical, ">=1.2, <1.5");
        assert!(requirement.matches("1.4.99").unwrap());
        assert!(!requirement.matches("1.5.0").unwrap());
        assert!(NativeVersionRequirement::parse(NativeRegistry::Cargo, ">= 1.2 < 1.5",).is_err());
    }

    #[test]
    fn npm_rejects_leading_zero_and_unsafe_integer_components() {
        for requirement in [
            "01.2",
            "1.02",
            ">1.02",
            "^01.2",
            "9007199254740992.0.0",
            ">9007199254740991",
        ] {
            assert!(NativeVersionRequirement::parse(NativeRegistry::Npm, requirement).is_err());
        }

        assert!(matches!(
            NativeDependencyLock::resolve(
                NativeRegistry::Npm,
                "@fiducia/core",
                "*",
                &[candidate("9007199254740992.0.0", 'a')],
            ),
            Err(NativeDependencyError::InvalidVersion { .. })
        ));
    }

    #[test]
    fn resolution_is_highest_satisfying_and_order_independent() {
        let candidates = vec![
            candidate("1.2.3", 'a'),
            candidate("1.9.0", 'b'),
            candidate("2.0.0", 'c'),
            candidate("1.10.0", 'd'),
        ];
        let first = NativeDependencyLock::resolve(
            NativeRegistry::Cargo,
            "fiducia_core",
            "1.2.3",
            &candidates,
        )
        .unwrap();
        let mut reversed = candidates;
        reversed.reverse();
        let second = NativeDependencyLock::resolve(
            NativeRegistry::Cargo,
            "fiducia_core",
            "1.2.3",
            &reversed,
        )
        .unwrap();

        assert_eq!(first.package.version, "1.10.0");
        assert_eq!(first, second);
        assert_eq!(
            first.canonical_json_bytes().unwrap(),
            second.canonical_json_bytes().unwrap()
        );
    }

    #[test]
    fn npm_exact_resolution_does_not_float() {
        let lock = NativeDependencyLock::resolve(
            NativeRegistry::Npm,
            "@fiducia/core",
            "1.2.3",
            &[candidate("1.2.3", 'a'), candidate("1.2.4", 'b')],
        )
        .unwrap();
        assert_eq!(lock.package.version, "1.2.3");
        assert_eq!(lock.requirement.canonical, "=1.2.3");
    }

    #[test]
    fn unsupported_or_mutable_native_syntax_fails_closed() {
        for requirement in [
            "1.0.0 || 2.0.0",
            "1.0.0 - 2.0.0",
            "latest",
            "workspace:^1.0.0",
            "file:../core",
            "npm:@scope/core@1.0.0",
            ">=1.0.0, <2.0.0",
        ] {
            assert!(NativeVersionRequirement::parse(NativeRegistry::Npm, requirement).is_err());
        }
        for requirement in [
            "1.0.0 || 2.0.0",
            ">=1.0.0 <2.0.0",
            "1.x",
            "git+https://example.invalid/core",
        ] {
            assert!(NativeVersionRequirement::parse(NativeRegistry::Cargo, requirement).is_err());
        }
    }

    #[test]
    fn exact_lock_rejects_translation_version_and_artifact_drift() {
        let mut lock = NativeDependencyLock::resolve(
            NativeRegistry::Npm,
            "@fiducia/core",
            "^1.2.3",
            &[candidate("1.9.0", 'a')],
        )
        .unwrap();

        lock.requirement.canonical = "^1.3.0".to_string();
        assert!(matches!(
            lock.validate(),
            Err(NativeDependencyError::CanonicalRequirementDrift { .. })
        ));

        let mut lock = NativeDependencyLock::resolve(
            NativeRegistry::Npm,
            "@fiducia/core",
            "^1.2.3",
            &[candidate("1.9.0", 'a')],
        )
        .unwrap();
        lock.package.version = "2.0.0".to_string();
        assert!(matches!(
            lock.validate(),
            Err(NativeDependencyError::ResolvedVersionDoesNotMatch { .. })
        ));

        let mut lock = NativeDependencyLock::resolve(
            NativeRegistry::Npm,
            "@fiducia/core",
            "^1.2.3",
            &[candidate("1.9.0", 'a')],
        )
        .unwrap();
        lock.artifact.sha256 = "0".repeat(64);
        assert!(matches!(
            lock.validate(),
            Err(NativeDependencyError::InvalidSha256 { .. })
        ));
    }

    #[test]
    fn candidate_identity_is_strict_and_unambiguous() {
        let duplicate = [candidate("1.2.3", 'a'), candidate("1.2.3", 'b')];
        assert!(matches!(
            NativeDependencyLock::resolve(
                NativeRegistry::Cargo,
                "fiducia_core",
                "1.2.3",
                &duplicate,
            ),
            Err(NativeDependencyError::DuplicateCandidateVersion { .. })
        ));

        let build_metadata = [candidate("1.2.3+linux", 'a')];
        assert!(matches!(
            NativeDependencyLock::resolve(
                NativeRegistry::Cargo,
                "fiducia_core",
                "1.2.3",
                &build_metadata,
            ),
            Err(NativeDependencyError::BuildMetadataNotAllowed { .. })
        ));

        let malformed_artifact = [NativeVersionCandidate {
            version: "1.2.3".to_string(),
            artifact: NativeArtifact {
                sha256: "A".repeat(64),
                size: 128,
                format: ArtifactFormat::TarGz,
            },
        }];
        assert!(matches!(
            NativeDependencyLock::resolve(
                NativeRegistry::Cargo,
                "fiducia_core",
                "1.2.3",
                &malformed_artifact,
            ),
            Err(NativeDependencyError::InvalidSha256 { .. })
        ));
    }
}
