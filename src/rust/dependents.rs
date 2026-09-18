//! Contracts for unpublished consumers to register their locked Zed dependencies
//! and for registry publish events to compute deterministic notification/update work.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const CONSUMER_REGISTRATION_PROTOCOL_V1: &str = "zed.consumer-registration.v1";
pub const CONSUMER_REGISTRATION_RECEIPT_PROTOCOL_V1: &str = "zed.consumer-registration-receipt.v1";
pub const RELEASE_IMPACT_PROTOCOL_V1: &str = "zed.release-impact.v1";
pub const RELEASE_IMPACT_PLAN_PROTOCOL_V1: &str = "zed.release-impact-plan.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConsumerKindV1 {
    Server,
    Cli,
    Worker,
    App,
    Library,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AutomationModeV1 {
    Off,
    NotifyOnly,
    DraftPr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseClassV1 {
    PatchSuppressed,
    PatchSecurityNotify,
    Minor,
    Major,
    Calendar,
}

impl ReleaseClassV1 {
    pub const fn notifies_dependents(self) -> bool {
        !matches!(self, Self::PatchSuppressed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LockedDependencySnapshotV1 {
    pub package: String,
    pub resolved_version: String,
    pub requirement: Option<String>,
    pub source: String,
    pub checksum_sha256: Option<String>,
    pub direct: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConsumerRegistrationRequestV1 {
    pub protocol: String,
    pub repository: String,
    pub default_branch: String,
    pub source_commit: String,
    pub consumer_kind: ConsumerKindV1,
    pub automation_mode: AutomationModeV1,
    pub registration_ttl_seconds: u64,
    pub notify_major: bool,
    pub notify_minor: bool,
    pub notify_patch: bool,
    pub security_patch_notify: bool,
    pub dependencies: Vec<LockedDependencySnapshotV1>,
    #[serde(default)]
    pub validation_commands: Vec<String>,
}

impl ConsumerRegistrationRequestV1 {
    pub fn validate(&self) -> Result<(), DependentsContractError> {
        if self.protocol != CONSUMER_REGISTRATION_PROTOCOL_V1 {
            return Err(DependentsContractError::Protocol(self.protocol.clone()));
        }
        validate_repository(&self.repository)?;
        validate_branch(&self.default_branch)?;
        validate_git_sha(&self.source_commit)?;
        if !(300..=2_592_000).contains(&self.registration_ttl_seconds) {
            return Err(DependentsContractError::RegistrationTtl(
                self.registration_ttl_seconds,
            ));
        }
        if self.dependencies.is_empty() {
            return Err(DependentsContractError::EmptyDependencies);
        }
        let mut identities = BTreeSet::new();
        for dependency in &self.dependencies {
            validate_package(&dependency.package)?;
            validate_non_empty("resolved_version", &dependency.resolved_version)?;
            validate_non_empty("source", &dependency.source)?;
            if let Some(checksum) = &dependency.checksum_sha256 {
                validate_sha256(checksum)?;
            }
            if !identities.insert(dependency.package.as_str()) {
                return Err(DependentsContractError::DuplicateDependency(
                    dependency.package.clone(),
                ));
            }
        }
        for command in &self.validation_commands {
            validate_non_empty("validation_command", command)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConsumerRegistrationReceiptV1 {
    pub protocol: String,
    pub registration_id: String,
    pub consumer_key: String,
    pub repository: String,
    pub source_commit: String,
    pub registered_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
    pub dependency_snapshot_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReleaseImpactRequestV1 {
    pub protocol: String,
    pub release_event_id: String,
    pub package: String,
    pub previous_version: Option<String>,
    pub published_version: String,
    pub release_class: ReleaseClassV1,
    pub security_critical: bool,
    pub published_source_commit: String,
}

impl ReleaseImpactRequestV1 {
    pub fn validate(&self) -> Result<(), DependentsContractError> {
        if self.protocol != RELEASE_IMPACT_PROTOCOL_V1 {
            return Err(DependentsContractError::Protocol(self.protocol.clone()));
        }
        validate_non_empty("release_event_id", &self.release_event_id)?;
        validate_package(&self.package)?;
        validate_non_empty("published_version", &self.published_version)?;
        validate_git_sha(&self.published_source_commit)?;
        if self.security_critical && self.release_class == ReleaseClassV1::PatchSuppressed {
            return Err(DependentsContractError::SecurityPatchSuppressed);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DependentActionV1 {
    Off,
    Suppressed,
    Notify,
    DraftPr,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DependentImpactV1 {
    pub registration_id: String,
    pub repository: String,
    pub default_branch: String,
    pub current_version: String,
    pub target_version: String,
    pub action: DependentActionV1,
    pub reason: String,
    pub requires_manifest_change: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReleaseImpactPlanV1 {
    pub protocol: String,
    pub release_event_id: String,
    pub package: String,
    pub published_version: String,
    pub release_class: ReleaseClassV1,
    pub registered_dependents: u64,
    pub active_dependents: u64,
    pub stale_dependents: u64,
    pub impacts: Vec<DependentImpactV1>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DependentsContractError {
    #[error("unsupported protocol `{0}`")]
    Protocol(String),
    #[error("repository must be canonical owner/name, got `{0}`")]
    Repository(String),
    #[error("invalid default branch `{0}`")]
    Branch(String),
    #[error("source commit must be a 40-character lowercase hex Git SHA")]
    GitSha,
    #[error("registration TTL {0} is outside 300..=2592000 seconds")]
    RegistrationTtl(u64),
    #[error("consumer registration must contain at least one locked dependency")]
    EmptyDependencies,
    #[error("duplicate dependency `{0}` in canonical locked snapshot")]
    DuplicateDependency(String),
    #[error("invalid package identity `{0}`; expected org/name")]
    Package(String),
    #[error("{0} cannot be empty")]
    Empty(&'static str),
    #[error("checksum_sha256 must be 64 lowercase hexadecimal characters")]
    Sha256,
    #[error("security-critical patch cannot use patch_suppressed release class")]
    SecurityPatchSuppressed,
}

fn validate_repository(repository: &str) -> Result<(), DependentsContractError> {
    let Some((owner, name)) = repository.split_once('/') else {
        return Err(DependentsContractError::Repository(repository.to_owned()));
    };
    if owner.is_empty()
        || name.is_empty()
        || name.contains('/')
        || !repository
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-._/".contains(&byte))
    {
        return Err(DependentsContractError::Repository(repository.to_owned()));
    }
    Ok(())
}

fn validate_branch(branch: &str) -> Result<(), DependentsContractError> {
    if branch.is_empty()
        || branch.starts_with('-')
        || branch.ends_with('/')
        || branch.contains("..")
        || branch.contains("//")
        || branch.chars().any(char::is_whitespace)
    {
        return Err(DependentsContractError::Branch(branch.to_owned()));
    }
    Ok(())
}

fn validate_git_sha(value: &str) -> Result<(), DependentsContractError> {
    if value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(DependentsContractError::GitSha)
    }
}

fn validate_sha256(value: &str) -> Result<(), DependentsContractError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(DependentsContractError::Sha256)
    }
}

fn validate_package(package: &str) -> Result<(), DependentsContractError> {
    let Some((org, name)) = package.split_once('/') else {
        return Err(DependentsContractError::Package(package.to_owned()));
    };
    if org.is_empty() || name.is_empty() || name.contains('/') {
        return Err(DependentsContractError::Package(package.to_owned()));
    }
    Ok(())
}

fn validate_non_empty(field: &'static str, value: &str) -> Result<(), DependentsContractError> {
    if value.trim().is_empty() {
        Err(DependentsContractError::Empty(field))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registration() -> ConsumerRegistrationRequestV1 {
        ConsumerRegistrationRequestV1 {
            protocol: CONSUMER_REGISTRATION_PROTOCOL_V1.into(),
            repository: "acme/payment-api".into(),
            default_branch: "main".into(),
            source_commit: "0123456789abcdef0123456789abcdef01234567".into(),
            consumer_kind: ConsumerKindV1::Server,
            automation_mode: AutomationModeV1::DraftPr,
            registration_ttl_seconds: 86_400,
            notify_major: true,
            notify_minor: true,
            notify_patch: false,
            security_patch_notify: true,
            dependencies: vec![LockedDependencySnapshotV1 {
                package: "oresoftware/ores-otel".into(),
                resolved_version: "2.4.0".into(),
                requirement: Some("^2.4".into()),
                source: "https://zpkg.net".into(),
                checksum_sha256: Some(
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
                ),
                direct: true,
            }],
            validation_commands: vec!["cargo test --all-targets".into()],
        }
    }

    #[test]
    fn registration_accepts_unpublished_server_with_locked_dependency_snapshot() {
        registration().validate().unwrap();
    }

    #[test]
    fn ordinary_patch_is_non_notifying_but_security_patch_is_notifying() {
        assert!(!ReleaseClassV1::PatchSuppressed.notifies_dependents());
        assert!(ReleaseClassV1::PatchSecurityNotify.notifies_dependents());
    }

    #[test]
    fn duplicate_package_snapshot_is_rejected() {
        let mut value = registration();
        value.dependencies.push(value.dependencies[0].clone());
        assert!(matches!(
            value.validate(),
            Err(DependentsContractError::DuplicateDependency(_))
        ));
    }

    #[test]
    fn security_patch_cannot_be_silently_suppressed() {
        let impact = ReleaseImpactRequestV1 {
            protocol: RELEASE_IMPACT_PROTOCOL_V1.into(),
            release_event_id: "event-1".into(),
            package: "acme/lib".into(),
            previous_version: Some("1.0.0".into()),
            published_version: "1.0.1".into(),
            release_class: ReleaseClassV1::PatchSuppressed,
            security_critical: true,
            published_source_commit: "0123456789abcdef0123456789abcdef01234567".into(),
        };
        assert_eq!(
            impact.validate(),
            Err(DependentsContractError::SecurityPatchSuppressed)
        );
    }
}
