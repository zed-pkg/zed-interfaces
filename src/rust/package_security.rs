#![allow(clippy::needless_return)]

//! Portable package-security identities promoted from the independently authored
//! `validation/package-security` TypeSpec and JSON Schema authorities.
//!
//! These types describe evidence. They do not authorize publication or runtime
//! deployment by themselves.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const PACKAGE_DEPENDENCY_CLOSURE_FORMAT_V1: &str = "zed-package-dependency-closure-v1";
pub const PACKAGE_DEPENDENCY_CLOSURE_MAX_DEPENDENCIES_V1: usize = 4096;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum RegistryPackageEcosystem {
    Npm,
    Cargo,
    Python,
    Git,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistryPackageArtifactIdentity {
    pub ecosystem: RegistryPackageEcosystem,
    pub package_name: String,
    pub package_version: String,
    pub source_uri: String,
    pub source_digest: String,
    pub resolved_revision: String,
    pub artifact_digest: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum HexPackageEcosystem {
    Hex,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HexPackageArtifactIdentity {
    pub ecosystem: HexPackageEcosystem,
    pub package_name: String,
    pub package_version: String,
    pub source_uri: String,
    pub source_digest: String,
    pub resolved_revision: String,
    pub artifact_digest: String,
    pub registry_uri: String,
    pub outer_checksum: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum PackageArtifactIdentity {
    Registry(RegistryPackageArtifactIdentity),
    Hex(HexPackageArtifactIdentity),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PackageDependencyClosure {
    pub format: String,
    pub root: PackageArtifactIdentity,
    pub dependencies: Vec<PackageArtifactIdentity>,
    pub dependency_lock_digest: String,
    pub closure_digest: String,
    pub resolver_id: String,
    pub resolver_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PackageSecurityContractError {
    #[error("package-security field `{field}` is empty")]
    EmptyField { field: &'static str },
    #[error("package-security field `{field}` exceeds {max} bytes")]
    FieldTooLong { field: &'static str, max: usize },
    #[error("package-security field `{field}` must be lowercase sha256 hex")]
    InvalidSha256 { field: &'static str },
    #[error("package dependency closure format is unsupported")]
    InvalidClosureFormat,
    #[error("package dependency closure exceeds {max} dependencies")]
    TooManyDependencies { max: usize },
}

impl RegistryPackageArtifactIdentity {
    pub fn validate(&self) -> Result<(), PackageSecurityContractError> {
        validate_common_identity(
            &self.package_name,
            &self.package_version,
            &self.source_uri,
            &self.source_digest,
            &self.resolved_revision,
            &self.artifact_digest,
        )?;
        return Ok(());
    }
}

impl HexPackageArtifactIdentity {
    pub fn validate(&self) -> Result<(), PackageSecurityContractError> {
        validate_common_identity(
            &self.package_name,
            &self.package_version,
            &self.source_uri,
            &self.source_digest,
            &self.resolved_revision,
            &self.artifact_digest,
        )?;
        validate_bounded_text("registry_uri", &self.registry_uri, 2048)?;
        validate_sha256("outer_checksum", &self.outer_checksum)?;
        return Ok(());
    }
}

impl PackageArtifactIdentity {
    pub fn validate(&self) -> Result<(), PackageSecurityContractError> {
        match self {
            Self::Registry(identity) => {
                return identity.validate();
            }
            Self::Hex(identity) => {
                return identity.validate();
            }
        }
    }
}

impl PackageDependencyClosure {
    pub fn validate(&self) -> Result<(), PackageSecurityContractError> {
        if self.format != PACKAGE_DEPENDENCY_CLOSURE_FORMAT_V1 {
            return Err(PackageSecurityContractError::InvalidClosureFormat);
        }
        self.root.validate()?;
        if self.dependencies.len() > PACKAGE_DEPENDENCY_CLOSURE_MAX_DEPENDENCIES_V1 {
            return Err(PackageSecurityContractError::TooManyDependencies {
                max: PACKAGE_DEPENDENCY_CLOSURE_MAX_DEPENDENCIES_V1,
            });
        }
        for dependency in &self.dependencies {
            dependency.validate()?;
        }
        validate_sha256("dependency_lock_digest", &self.dependency_lock_digest)?;
        validate_sha256("closure_digest", &self.closure_digest)?;
        validate_bounded_text("resolver_id", &self.resolver_id, 128)?;
        validate_bounded_text("resolver_version", &self.resolver_version, 128)?;
        return Ok(());
    }
}

fn validate_common_identity(
    package_name: &str,
    package_version: &str,
    source_uri: &str,
    source_digest: &str,
    resolved_revision: &str,
    artifact_digest: &str,
) -> Result<(), PackageSecurityContractError> {
    validate_bounded_text("package_name", package_name, 256)?;
    validate_bounded_text("package_version", package_version, 128)?;
    validate_bounded_text("source_uri", source_uri, 2048)?;
    validate_sha256("source_digest", source_digest)?;
    validate_bounded_text("resolved_revision", resolved_revision, 256)?;
    validate_sha256("artifact_digest", artifact_digest)?;
    return Ok(());
}

fn validate_bounded_text(
    field: &'static str,
    value: &str,
    max: usize,
) -> Result<(), PackageSecurityContractError> {
    if value.is_empty() {
        return Err(PackageSecurityContractError::EmptyField { field });
    }
    if value.len() > max {
        return Err(PackageSecurityContractError::FieldTooLong { field, max });
    }
    return Ok(());
}

fn validate_sha256(field: &'static str, value: &str) -> Result<(), PackageSecurityContractError> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(PackageSecurityContractError::InvalidSha256 { field });
    }
    return Ok(());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(character: char) -> String {
        return character.to_string().repeat(64);
    }

    fn hex_identity(name: &str, version: &str, seed: char) -> PackageArtifactIdentity {
        return PackageArtifactIdentity::Hex(HexPackageArtifactIdentity {
            ecosystem: HexPackageEcosystem::Hex,
            package_name: name.into(),
            package_version: version.into(),
            source_uri: format!("https://github.com/example/{name}"),
            source_digest: digest(seed),
            resolved_revision: version.into(),
            artifact_digest: digest(if seed == 'a' { 'b' } else { seed }),
            registry_uri: "https://repo.hex.pm".into(),
            outer_checksum: digest(if seed == 'a' { 'c' } else { seed }),
        });
    }

    #[test]
    fn hex_closure_round_trips_and_validates() {
        let closure = PackageDependencyClosure {
            format: PACKAGE_DEPENDENCY_CLOSURE_FORMAT_V1.into(),
            root: hex_identity("beam_app", "1.0.0", 'a'),
            dependencies: vec![hex_identity("gleam_stdlib", "0.62.1", 'd')],
            dependency_lock_digest: digest('e'),
            closure_digest: digest('f'),
            resolver_id: "zed-hex-resolver".into(),
            resolver_version: "1.0.0".into(),
        };
        closure
            .validate()
            .expect("valid package dependency closure");

        let encoded = serde_json::to_value(&closure).expect("serialize closure");
        assert_eq!(encoded["root"]["ecosystem"], "hex");
        assert_eq!(encoded["dependencies"][0]["ecosystem"], "hex");
        let decoded: PackageDependencyClosure =
            serde_json::from_value(encoded).expect("deserialize closure");
        assert_eq!(decoded, closure);
    }

    #[test]
    fn hex_identity_requires_outer_checksum_and_rejects_unknown_fields() {
        let missing_outer_checksum = serde_json::json!({
            "ecosystem": "hex",
            "package_name": "gleam_stdlib",
            "package_version": "0.62.1",
            "source_uri": "https://github.com/gleam-lang/stdlib",
            "source_digest": digest('a'),
            "resolved_revision": "0.62.1",
            "artifact_digest": digest('b'),
            "registry_uri": "https://repo.hex.pm"
        });
        assert!(serde_json::from_value::<PackageArtifactIdentity>(missing_outer_checksum).is_err());

        let mut unknown_field = serde_json::to_value(hex_identity("gleam_stdlib", "0.62.1", 'a'))
            .expect("serialize hex identity");
        unknown_field["deployment_authorized"] = serde_json::Value::Bool(true);
        assert!(serde_json::from_value::<PackageArtifactIdentity>(unknown_field).is_err());
    }

    #[test]
    fn validation_requires_lowercase_sha256_and_bounds_closure_size() {
        let mut closure = PackageDependencyClosure {
            format: PACKAGE_DEPENDENCY_CLOSURE_FORMAT_V1.into(),
            root: hex_identity("beam_app", "1.0.0", 'a'),
            dependencies: Vec::new(),
            dependency_lock_digest: digest('d'),
            closure_digest: digest('e'),
            resolver_id: "zed-hex-resolver".into(),
            resolver_version: "1.0.0".into(),
        };
        closure.closure_digest = "A".repeat(64);
        assert_eq!(
            closure.validate(),
            Err(PackageSecurityContractError::InvalidSha256 {
                field: "closure_digest"
            })
        );

        closure.closure_digest = digest('e');
        closure.dependencies = (0..=PACKAGE_DEPENDENCY_CLOSURE_MAX_DEPENDENCIES_V1)
            .map(|index| hex_identity(&format!("dep_{index}"), "1.0.0", 'd'))
            .collect();
        assert_eq!(
            closure.validate(),
            Err(PackageSecurityContractError::TooManyDependencies {
                max: PACKAGE_DEPENDENCY_CLOSURE_MAX_DEPENDENCIES_V1
            })
        );
    }
}
