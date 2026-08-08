use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::manifest::is_sha256_hex;
use crate::nix::{
    NixExportMode, NixExportSection, NixInteropArtifact, NixPackageIdentity, NixPolicyEvidence,
    NixPolicyProfile,
};

/// Major-versioned identifier for a read-only Zed → Nix export plan.
///
/// The plan is execution-independent. It binds author intent and immutable Zed
/// inputs before any flake is generated or Nix process is started.
pub const NIX_EXPORT_PLAN_SCHEMA_V1: &str = "zed.nix-export-plan/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum NixExportPackageClass {
    /// Immutable package data with no executable entry point.
    Data,
    /// Executables already present in the immutable Zed artifact. Contract v1
    /// never infers or executes a source build to create them.
    PrebuiltBin,
}

/// Fully resolved author intent. Unlike manifest intent, the package attribute
/// is no longer optional and non-semantic arrays are canonicalized.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResolvedNixExportIntent {
    pub mode: NixExportMode,
    pub attribute: String,
    pub systems: Vec<String>,
    pub outputs: Vec<String>,
}

impl ResolvedNixExportIntent {
    pub fn normalize(&mut self) {
        self.systems.sort();
        self.outputs.sort();
    }

    pub fn validate(&self, package_name: &str) -> Result<(), NixExportPlanError> {
        let intent = NixExportSection {
            mode: self.mode,
            attribute: Some(self.attribute.clone()),
            systems: self.systems.clone(),
            outputs: self.outputs.clone(),
        };
        intent
            .validate(package_name)
            .map_err(|error| NixExportPlanError::InvalidIntent(error.to_string()))?;
        ensure_sorted_unique(&self.systems, "Nix systems")?;
        ensure_sorted_unique(&self.outputs, "Nix outputs")?;
        if self.mode != NixExportMode::Artifact {
            return Err(NixExportPlanError::InvalidIntent(
                "export plan v1 supports only artifact mode".to_string(),
            ));
        }
        Ok(())
    }
}

/// Exact immutable Zed source selected by planning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlannedZedExportArtifact {
    /// Safe artifact basename, e.g. `acme-tool-1.2.3.tar.gz`.
    pub file_name: String,
    pub artifact: NixInteropArtifact,
    /// SHA-256 of exact `.zpkg.toml` bytes, including comments and formatting.
    pub manifest_sha256: String,
    /// SHA-256 of exact `.zpkg.lock` bytes.
    pub lock_sha256: String,
}

impl PlannedZedExportArtifact {
    pub fn validate(&self, package: &NixPackageIdentity) -> Result<(), NixExportPlanError> {
        self.artifact
            .validate("planned Zed artifact")
            .map_err(|error| NixExportPlanError::InvalidArtifact(error.to_string()))?;
        validate_sha256("manifest", &self.manifest_sha256)?;
        validate_sha256("lock", &self.lock_sha256)?;

        let expected = format!(
            "{}-{}-{}.{}",
            package.org,
            package.name,
            package.version,
            self.artifact.format.extension()
        );
        if self.file_name != expected || !is_safe_basename(&self.file_name) {
            return Err(NixExportPlanError::InvalidArtifact(format!(
                "artifact filename must be the canonical safe basename `{expected}`"
            )));
        }
        Ok(())
    }
}

/// Reserved typed dependency edge for later plan revisions.
///
/// Strict v1 plans must keep `dependencies` empty. Keeping the typed field in
/// the wire contract prevents a later implementation from smuggling an opaque
/// native package-manager graph into otherwise valid-looking plan JSON.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlannedNixExportDependency {
    pub org: String,
    pub name: String,
    pub version: String,
    pub sha256: String,
}

impl PlannedNixExportDependency {
    fn validate(&self) -> Result<(), NixExportPlanError> {
        NixPackageIdentity {
            org: self.org.clone(),
            name: self.name.clone(),
            version: self.version.clone(),
            target: None,
        }
        .validate()
        .map_err(|error| NixExportPlanError::InvalidDependency(error.to_string()))?;
        validate_sha256("dependency artifact", &self.sha256)
            .map_err(|error| NixExportPlanError::InvalidDependency(error.to_string()))
    }
}

/// Canonical, credential-free plan for one Zed → Nix package export.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NixExportPlan {
    pub schema: String,
    pub package: NixPackageIdentity,
    pub package_class: NixExportPackageClass,
    pub intent: ResolvedNixExportIntent,
    pub source: PlannedZedExportArtifact,
    /// Command name → artifact-relative executable path.
    #[serde(default)]
    pub bins: BTreeMap<String, String>,
    /// Contract v1 requires this list to be empty.
    #[serde(default)]
    pub dependencies: Vec<PlannedNixExportDependency>,
    pub policy: NixPolicyEvidence,
}

impl NixExportPlan {
    pub fn new(
        package: NixPackageIdentity,
        package_class: NixExportPackageClass,
        intent: ResolvedNixExportIntent,
        source: PlannedZedExportArtifact,
        bins: BTreeMap<String, String>,
        policy: NixPolicyEvidence,
    ) -> Self {
        Self {
            schema: NIX_EXPORT_PLAN_SCHEMA_V1.to_string(),
            package,
            package_class,
            intent,
            source,
            bins,
            dependencies: Vec::new(),
            policy,
        }
    }

    pub fn normalize(&mut self) {
        self.intent.normalize();
        self.dependencies.sort();
    }

    pub fn validate(&self) -> Result<(), NixExportPlanError> {
        if self.schema != NIX_EXPORT_PLAN_SCHEMA_V1 {
            return Err(NixExportPlanError::UnsupportedSchema(self.schema.clone()));
        }
        self.package
            .validate()
            .map_err(|error| NixExportPlanError::InvalidPackage(error.to_string()))?;
        self.intent.validate(&self.package.name)?;
        self.source.validate(&self.package)?;
        self.policy
            .validate()
            .map_err(|error| NixExportPlanError::InvalidPolicy(error.to_string()))?;
        if self.policy.profile != NixPolicyProfile::StrictV1 {
            return Err(NixExportPlanError::InvalidPolicy(
                "publishable export plans require strict-v1 policy".to_string(),
            ));
        }

        match self.package_class {
            NixExportPackageClass::Data if !self.bins.is_empty() => {
                return Err(NixExportPlanError::InvalidBins(
                    "data packages must not declare executable bins".to_string(),
                ));
            }
            NixExportPackageClass::PrebuiltBin if self.bins.is_empty() => {
                return Err(NixExportPlanError::InvalidBins(
                    "prebuilt-bin packages must declare at least one executable".to_string(),
                ));
            }
            _ => {}
        }

        for (name, path) in &self.bins {
            if !is_bin_name(name) {
                return Err(NixExportPlanError::InvalidBins(format!(
                    "invalid executable name `{name}`"
                )));
            }
            if !is_safe_relative_path(path) {
                return Err(NixExportPlanError::InvalidBins(format!(
                    "executable `{name}` has unsafe artifact-relative path `{path}`"
                )));
            }
        }

        for dependency in &self.dependencies {
            dependency.validate()?;
        }
        ensure_sorted_unique(&self.dependencies, "planned dependencies")?;
        if !self.dependencies.is_empty() {
            return Err(NixExportPlanError::InvalidDependency(
                "export plan v1 accepts dependency-free packages only".to_string(),
            ));
        }
        Ok(())
    }

    /// Stable compact JSON suitable for hashing, review, and later export.
    ///
    /// Callers may construct intent arrays in any order; this method clones and
    /// normalizes the plan before validating and serializing it.
    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>, NixExportPlanError> {
        let mut canonical = self.clone();
        canonical.normalize();
        canonical.validate()?;
        serde_json::to_vec(&canonical).map_err(|error| NixExportPlanError::Json(error.to_string()))
    }

    pub fn canonical_json_string(&self) -> Result<String, NixExportPlanError> {
        String::from_utf8(self.canonical_json_bytes()?)
            .map_err(|error| NixExportPlanError::Json(error.to_string()))
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum NixExportPlanError {
    #[error("unsupported Nix export plan schema `{0}`")]
    UnsupportedSchema(String),
    #[error("invalid Nix export plan package: {0}")]
    InvalidPackage(String),
    #[error("invalid Nix export intent: {0}")]
    InvalidIntent(String),
    #[error("invalid planned Zed artifact: {0}")]
    InvalidArtifact(String),
    #[error("invalid prebuilt executable inventory: {0}")]
    InvalidBins(String),
    #[error("invalid planned dependency graph: {0}")]
    InvalidDependency(String),
    #[error("invalid Nix export policy: {0}")]
    InvalidPolicy(String),
    #[error("Nix export plan JSON error: {0}")]
    Json(String),
}

fn validate_sha256(field: &str, value: &str) -> Result<(), NixExportPlanError> {
    if !is_sha256_hex(value) {
        return Err(NixExportPlanError::InvalidArtifact(format!(
            "{field} SHA-256 must be 64 lowercase hexadecimal characters"
        )));
    }
    Ok(())
}

fn is_safe_basename(value: &str) -> bool {
    let path = Path::new(value);
    path.file_name() == Some(path.as_os_str())
        && !value.is_empty()
        && !value.starts_with('.')
        && !value.chars().any(char::is_whitespace)
}

fn is_safe_relative_path(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('/')
        && !value.ends_with('/')
        && !value.contains('\\')
        && value.split('/').all(|part| {
            !part.is_empty() && part != "." && part != ".." && !part.chars().any(char::is_control)
        })
}

fn is_bin_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && !matches!(value.chars().next(), Some('-' | '.'))
        && !matches!(value.chars().last(), Some('-' | '.'))
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        })
}

fn ensure_sorted_unique<T>(values: &[T], field: &str) -> Result<(), NixExportPlanError>
where
    T: Ord + std::fmt::Debug,
{
    let mut seen = BTreeSet::new();
    let mut previous: Option<&T> = None;
    for value in values {
        if !seen.insert(value) {
            return Err(NixExportPlanError::InvalidIntent(format!(
                "{field} contains duplicate {value:?}"
            )));
        }
        if previous.is_some_and(|prior| prior > value) {
            return Err(NixExportPlanError::InvalidIntent(format!(
                "{field} must be sorted canonically"
            )));
        }
        previous = Some(value);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact::ArtifactFormat;
    use crate::nix::{NixBuilderNetwork, NixPolicyEvidence};

    fn digest(character: char) -> String {
        std::iter::repeat_n(character, 64).collect()
    }

    fn policy() -> NixPolicyEvidence {
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

    fn data_plan() -> NixExportPlan {
        NixExportPlan::new(
            NixPackageIdentity {
                org: "acme".to_string(),
                name: "dataset".to_string(),
                version: "1.2.3".to_string(),
                target: None,
            },
            NixExportPackageClass::Data,
            ResolvedNixExportIntent {
                mode: NixExportMode::Artifact,
                attribute: "dataset".to_string(),
                systems: vec!["x86_64-linux".to_string(), "aarch64-linux".to_string()],
                outputs: vec!["out".to_string()],
            },
            PlannedZedExportArtifact {
                file_name: "acme-dataset-1.2.3.tar.gz".to_string(),
                artifact: NixInteropArtifact {
                    format: ArtifactFormat::TarGz,
                    sha256: digest('a'),
                    size: 123,
                },
                manifest_sha256: digest('b'),
                lock_sha256: digest('c'),
            },
            BTreeMap::new(),
            policy(),
        )
    }

    #[test]
    fn canonical_json_sorts_non_semantic_intent_arrays() {
        let plan = data_plan();
        let encoded = plan.canonical_json_string().unwrap();
        let decoded: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        assert_eq!(
            decoded["intent"]["systems"],
            serde_json::json!(["aarch64-linux", "x86_64-linux"])
        );
        assert_eq!(decoded["schema"], NIX_EXPORT_PLAN_SCHEMA_V1);
        assert_eq!(decoded["bins"], serde_json::json!({}));
        assert_eq!(decoded["dependencies"], serde_json::json!([]));
        assert!(!encoded.contains("registry"));
        assert!(!encoded.contains("token"));
        assert!(!encoded.contains("/tmp/"));
    }

    #[test]
    fn canonical_json_is_stable_after_round_trip() {
        let encoded = data_plan().canonical_json_string().unwrap();
        let decoded: NixExportPlan = serde_json::from_str(&encoded).unwrap();
        assert_eq!(encoded, decoded.canonical_json_string().unwrap());
    }

    #[test]
    fn prebuilt_bins_are_typed_and_path_safe() {
        let mut plan = data_plan();
        plan.package_class = NixExportPackageClass::PrebuiltBin;
        plan.bins
            .insert("dataset-tool".to_string(), "bin/dataset-tool".to_string());
        plan.canonical_json_bytes().unwrap();

        plan.bins
            .insert("escape".to_string(), "../outside".to_string());
        assert!(matches!(
            plan.canonical_json_bytes().unwrap_err(),
            NixExportPlanError::InvalidBins(_)
        ));
    }

    #[test]
    fn artifact_filename_and_exact_input_digests_fail_closed() {
        let mut plan = data_plan();
        plan.source.file_name = "other.tar.gz".to_string();
        assert!(matches!(
            plan.canonical_json_bytes().unwrap_err(),
            NixExportPlanError::InvalidArtifact(_)
        ));

        let mut plan = data_plan();
        plan.source.lock_sha256 = "not-a-digest".to_string();
        assert!(matches!(
            plan.canonical_json_bytes().unwrap_err(),
            NixExportPlanError::InvalidArtifact(_)
        ));
    }

    #[test]
    fn strict_v1_rejects_dependency_edges() {
        let mut plan = data_plan();
        plan.dependencies.push(PlannedNixExportDependency {
            org: "acme".to_string(),
            name: "other".to_string(),
            version: "1.0.0".to_string(),
            sha256: digest('d'),
        });
        assert!(matches!(
            plan.canonical_json_bytes().unwrap_err(),
            NixExportPlanError::InvalidDependency(_)
        ));
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let encoded = data_plan().canonical_json_string().unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&encoded).unwrap();
        value["registry"] = serde_json::json!("https://secret.invalid");
        assert!(serde_json::from_value::<NixExportPlan>(value).is_err());
    }
}
