//! Versioned contracts for coordinating one organization identity across
//! package registries and source forges.
//!
//! Registries do not expose one interchangeable "organization" primitive:
//! npm has literal organization scopes, Maven Central verifies namespace
//! prefixes, crates.io has only global package names, pub.dev verifies a
//! publisher domain, and the source forges expose organizations, groups, or
//! workspaces. This module preserves those distinctions in a deterministic
//! plan and in auditable observation receipts.
//!
//! The types deliberately contain no network, credential, browser, or mutation
//! implementation. A planner may construct these documents and provider
//! adapters may later check or execute individual steps, but neither a plan nor
//! a missing receipt is evidence that an external namespace was reserved.

use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Versioned wire identity for one deterministic cross-registry claim plan.
pub const REGISTRY_NAMESPACE_PLAN_SCHEMA_V1: &str = "zed.registry-namespace-plan/v1";
/// Versioned wire identity for one observed provider claim result.
pub const REGISTRY_NAMESPACE_RECEIPT_SCHEMA_V1: &str = "zed.registry-namespace-claim-receipt/v1";

/// Registry or source-forge namespace whose identity is being coordinated.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum RegistryNamespaceProvider {
    Npm,
    MavenCentral,
    CratesIo,
    PubDev,
    #[serde(rename = "github")]
    GitHub,
    #[serde(rename = "gitlab-com")]
    GitLabCom,
    BitbucketCloud,
}

impl RegistryNamespaceProvider {
    pub const ALL: [Self; 7] = [
        Self::Npm,
        Self::MavenCentral,
        Self::CratesIo,
        Self::PubDev,
        Self::GitHub,
        Self::GitLabCom,
        Self::BitbucketCloud,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Npm => "npm",
            Self::MavenCentral => "maven-central",
            Self::CratesIo => "crates-io",
            Self::PubDev => "pub-dev",
            Self::GitHub => "github",
            Self::GitLabCom => "gitlab-com",
            Self::BitbucketCloud => "bitbucket-cloud",
        }
    }

    pub fn expected_model(self) -> RegistryNamespaceModel {
        match self {
            Self::Npm => RegistryNamespaceModel::LiteralOrganizationScope,
            Self::MavenCentral => RegistryNamespaceModel::VerifiedGroupIdPrefix,
            Self::CratesIo => RegistryNamespaceModel::GlobalPackageNames,
            Self::PubDev => RegistryNamespaceModel::VerifiedPublisherDomain,
            Self::GitHub => RegistryNamespaceModel::ForgeOrganization,
            Self::GitLabCom => RegistryNamespaceModel::ForgeGroup,
            Self::BitbucketCloud => RegistryNamespaceModel::ForgeWorkspace,
        }
    }
}

/// Identity primitive exposed by one provider.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum RegistryNamespaceModel {
    /// A literal account-owned package scope, such as `@acme` on npm.
    LiteralOrganizationScope,
    /// A verified package-prefix namespace, usually a reverse-DNS groupId.
    VerifiedGroupIdPrefix,
    /// Globally unique package names with no reservable organization scope.
    GlobalPackageNames,
    /// A publisher identity proven through control of a domain.
    VerifiedPublisherDomain,
    /// A source-forge organization login.
    ForgeOrganization,
    /// A source-forge top-level group path.
    ForgeGroup,
    /// A source-forge workspace identifier.
    ForgeWorkspace,
}

/// How far an ordinary client can take the claim without pretending that a
/// manual or first-publication boundary is an API operation.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum RegistryNamespaceAutomation {
    /// A human must use the provider's web or administration interface.
    ManualWebFlow,
    /// Proof is established separately, after which an adapter may use an API.
    ProofThenApi,
    /// The identity is established only by publishing the first real package.
    FirstPublication,
    /// The provider has no organization-level namespace to reserve.
    NotReservable,
}

/// Actionability of one provider entry before any external mutation occurs.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum RegistryNamespaceDisposition {
    /// All planner inputs exist; the ordered claim steps may now be attempted.
    Actionable,
    /// One or more required identity proofs or inputs are absent.
    MissingPrerequisite,
    /// The provider requires a human-owned web or administration flow.
    ManualActionRequired,
    /// No organization-level reservation exists for this provider.
    NotReservable,
}

/// Evidence or authority a provider requires before ownership can be asserted.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum RegistryNamespaceProof {
    RegistryAccountControl,
    DomainControl,
    #[serde(rename = "github-account-control")]
    GitHubAccountControl,
    ForgeAdministrator,
    ExistingPackageOwnership,
}

/// Provider-neutral step in an ordered namespace claim plan.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum RegistryNamespaceAction {
    CheckAvailability,
    CreateOrganization,
    RegisterNamespace,
    VerifyDomain,
    CreatePublisher,
    PublishFirstPackage,
    AddOwnerTeam,
    CreateGroup,
    CreateWorkspace,
    RecordOwnershipEvidence,
}

/// One ordered claim step. `manual` describes the provider boundary, not a
/// suggestion that the action has happened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RegistryNamespaceStep {
    pub action: RegistryNamespaceAction,
    pub summary: String,
    pub manual: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prerequisite: Option<String>,
}

impl RegistryNamespaceStep {
    fn validate(&self, field: &str) -> Result<(), RegistryNamespaceError> {
        validate_text(&format!("{field}.summary"), &self.summary, 512)?;
        if let Some(prerequisite) = &self.prerequisite {
            validate_text(&format!("{field}.prerequisite"), prerequisite, 256)?;
        }
        Ok(())
    }
}

/// User-controlled inputs from which a deterministic plan is derived.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RegistryNamespaceRequest {
    /// Portable lowercase brand slug shared by providers that support it.
    pub brand: String,
    /// Canonical lowercase registrable domain without scheme, port, path, or
    /// trailing dot. Required for domain-derived Maven and pub.dev identities.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<String>,
    /// Explicit GitHub owner used for `io.github.<owner>` fallback namespaces
    /// and ownership proof. It is never inferred from credentials.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub github_owner: Option<String>,
    /// Providers requested by the caller. A plan must contain exactly one entry
    /// for each provider and no unrequested entry.
    pub providers: Vec<RegistryNamespaceProvider>,
}

impl RegistryNamespaceRequest {
    pub fn validate(&self) -> Result<(), RegistryNamespaceError> {
        validate_portable_slug("request.brand", &self.brand)?;
        if let Some(domain) = &self.domain {
            validate_domain("request.domain", domain)?;
        }
        if let Some(owner) = &self.github_owner {
            validate_portable_slug("request.github-owner", owner)?;
        }
        if self.providers.is_empty() {
            return Err(RegistryNamespaceError::NoProviders);
        }
        let mut providers = BTreeSet::new();
        for provider in &self.providers {
            if !providers.insert(*provider) {
                return Err(RegistryNamespaceError::DuplicateProvider {
                    provider: *provider,
                });
            }
        }
        Ok(())
    }

    pub fn normalized(&self) -> Self {
        let mut request = self.clone();
        request.providers.sort();
        request
    }
}

/// One provider-specific identity and its pre-mutation claim procedure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RegistryNamespaceEntry {
    pub provider: RegistryNamespaceProvider,
    pub model: RegistryNamespaceModel,
    /// Exact provider coordinate when one exists, such as `@acme`,
    /// `com.example`, `example.com`, or `acme`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coordinate: Option<String>,
    /// Advisory prefix for global package-name registries. A prefix is not a
    /// reserved namespace and must never be represented as one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_prefix: Option<String>,
    pub automation: RegistryNamespaceAutomation,
    pub disposition: RegistryNamespaceDisposition,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub proofs: Vec<RegistryNamespaceProof>,
    pub steps: Vec<RegistryNamespaceStep>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl RegistryNamespaceEntry {
    fn validate(&self, index: usize) -> Result<(), RegistryNamespaceError> {
        let field = format!("entries[{index}]");
        let expected = self.provider.expected_model();
        if self.model != expected {
            return Err(RegistryNamespaceError::ProviderModelMismatch {
                provider: self.provider,
                expected,
                found: self.model,
            });
        }
        if let Some(coordinate) = &self.coordinate {
            validate_coordinate(&format!("{field}.coordinate"), coordinate)?;
        }
        if let Some(prefix) = &self.package_prefix {
            validate_package_prefix(&format!("{field}.package-prefix"), prefix)?;
        }
        if self.provider == RegistryNamespaceProvider::CratesIo {
            if self.coordinate.is_some() {
                return Err(RegistryNamespaceError::CratesIoCannotReserveCoordinate);
            }
            if self.package_prefix.is_none() {
                return Err(RegistryNamespaceError::CratesIoPrefixRequired);
            }
            if self.automation != RegistryNamespaceAutomation::NotReservable
                || self.disposition != RegistryNamespaceDisposition::NotReservable
            {
                return Err(RegistryNamespaceError::NotReservableInconsistency {
                    provider: self.provider,
                });
            }
        } else {
            if self.package_prefix.is_some() {
                return Err(RegistryNamespaceError::UnexpectedPackagePrefix {
                    provider: self.provider,
                });
            }
            if self.disposition == RegistryNamespaceDisposition::NotReservable
                || self.automation == RegistryNamespaceAutomation::NotReservable
            {
                return Err(RegistryNamespaceError::NotReservableInconsistency {
                    provider: self.provider,
                });
            }
        }
        if self.disposition != RegistryNamespaceDisposition::MissingPrerequisite
            && self.provider != RegistryNamespaceProvider::CratesIo
            && self.coordinate.is_none()
        {
            return Err(RegistryNamespaceError::CoordinateRequired {
                provider: self.provider,
            });
        }

        let mut proofs = BTreeSet::new();
        for proof in &self.proofs {
            if !proofs.insert(*proof) {
                return Err(RegistryNamespaceError::DuplicateProof {
                    provider: self.provider,
                    proof: *proof,
                });
            }
        }
        if self.steps.is_empty() {
            return Err(RegistryNamespaceError::NoSteps {
                provider: self.provider,
            });
        }
        for (step_index, step) in self.steps.iter().enumerate() {
            step.validate(&format!("{field}.steps[{step_index}]"))?;
        }
        for (warning_index, warning) in self.warnings.iter().enumerate() {
            validate_text(&format!("{field}.warnings[{warning_index}]"), warning, 1024)?;
        }
        Ok(())
    }

    fn normalized(&self) -> Self {
        let mut entry = self.clone();
        entry.proofs.sort();
        entry.warnings.sort();
        entry
    }
}

/// Deterministic pre-mutation claim plan. It says what must be done; it never
/// says an external namespace was successfully created or reserved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RegistryNamespacePlan {
    pub schema: String,
    pub request: RegistryNamespaceRequest,
    pub entries: Vec<RegistryNamespaceEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl RegistryNamespacePlan {
    pub const SCHEMA_V1: &'static str = REGISTRY_NAMESPACE_PLAN_SCHEMA_V1;

    pub fn validate(&self) -> Result<(), RegistryNamespaceError> {
        if self.schema != Self::SCHEMA_V1 {
            return Err(RegistryNamespaceError::UnsupportedSchema {
                found: self.schema.clone(),
                supported: Self::SCHEMA_V1.to_owned(),
            });
        }
        self.request.validate()?;
        if self.entries.is_empty() {
            return Err(RegistryNamespaceError::NoEntries);
        }

        let requested = self
            .request
            .providers
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut observed = BTreeSet::new();
        for (index, entry) in self.entries.iter().enumerate() {
            entry.validate(index)?;
            if !observed.insert(entry.provider) {
                return Err(RegistryNamespaceError::DuplicateProvider {
                    provider: entry.provider,
                });
            }
            if !requested.contains(&entry.provider) {
                return Err(RegistryNamespaceError::UnrequestedProvider {
                    provider: entry.provider,
                });
            }
        }
        if requested != observed {
            let missing = requested
                .difference(&observed)
                .next()
                .copied()
                .expect("sets differ only when a provider is missing");
            return Err(RegistryNamespaceError::MissingProvider { provider: missing });
        }
        for (index, warning) in self.warnings.iter().enumerate() {
            validate_text(&format!("warnings[{index}]"), warning, 1024)?;
        }
        Ok(())
    }

    pub fn normalized(&self) -> Self {
        let mut plan = self.clone();
        plan.request = plan.request.normalized();
        plan.entries = plan
            .entries
            .iter()
            .map(RegistryNamespaceEntry::normalized)
            .collect();
        plan.entries.sort_by_key(|entry| entry.provider);
        plan.warnings.sort();
        plan
    }

    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>, RegistryNamespaceError> {
        self.validate()?;
        serde_json::to_vec(&self.normalized())
            .map_err(|error| RegistryNamespaceError::Serialization(error.to_string()))
    }
}

/// Result observed after one provider-specific check or claim attempt.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum RegistryNamespaceClaimOutcome {
    Reserved,
    ExistingOwnershipVerified,
    FirstPublicationCompleted,
    Unavailable,
    ManualActionRequired,
    NotReservable,
}

/// Non-secret evidence attached to a claim observation. `reference` is an
/// opaque provider reference or public URL; credentials and response bodies do
/// not belong in receipts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RegistryNamespaceEvidence {
    pub kind: RegistryNamespaceProof,
    pub subject: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

impl RegistryNamespaceEvidence {
    fn validate(&self, index: usize) -> Result<(), RegistryNamespaceError> {
        validate_text(&format!("evidence[{index}].subject"), &self.subject, 256)?;
        if let Some(reference) = &self.reference {
            validate_text(&format!("evidence[{index}].reference"), reference, 2048)?;
        }
        if let Some(sha256) = &self.sha256 {
            validate_sha256(&format!("evidence[{index}].sha256"), sha256)?;
        }
        Ok(())
    }
}

/// Auditable observation for a single provider. A receipt can report
/// unavailability, manual work, or non-reservability; absence of a receipt is
/// never interpreted as success.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RegistryNamespaceClaimReceipt {
    pub schema: String,
    /// Lowercase SHA-256 of `RegistryNamespacePlan::canonical_json_bytes()`.
    pub plan_sha256: String,
    pub provider: RegistryNamespaceProvider,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coordinate: Option<String>,
    pub outcome: RegistryNamespaceClaimOutcome,
    pub observed_at_unix_seconds: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence: Vec<RegistryNamespaceEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

impl RegistryNamespaceClaimReceipt {
    pub const SCHEMA_V1: &'static str = REGISTRY_NAMESPACE_RECEIPT_SCHEMA_V1;

    pub fn validate(&self) -> Result<(), RegistryNamespaceError> {
        if self.schema != Self::SCHEMA_V1 {
            return Err(RegistryNamespaceError::UnsupportedSchema {
                found: self.schema.clone(),
                supported: Self::SCHEMA_V1.to_owned(),
            });
        }
        validate_sha256("plan-sha256", &self.plan_sha256)?;
        if let Some(coordinate) = &self.coordinate {
            validate_coordinate("coordinate", coordinate)?;
        }
        if self.provider == RegistryNamespaceProvider::CratesIo
            && self.outcome == RegistryNamespaceClaimOutcome::Reserved
        {
            return Err(RegistryNamespaceError::CratesIoCannotBeReserved);
        }
        if self.outcome == RegistryNamespaceClaimOutcome::NotReservable
            && self.provider != RegistryNamespaceProvider::CratesIo
        {
            return Err(RegistryNamespaceError::NotReservableInconsistency {
                provider: self.provider,
            });
        }
        if matches!(
            self.outcome,
            RegistryNamespaceClaimOutcome::Reserved
                | RegistryNamespaceClaimOutcome::ExistingOwnershipVerified
                | RegistryNamespaceClaimOutcome::FirstPublicationCompleted
        ) && self.evidence.is_empty()
        {
            return Err(RegistryNamespaceError::EvidenceRequired {
                outcome: self.outcome,
            });
        }
        for (index, evidence) in self.evidence.iter().enumerate() {
            evidence.validate(index)?;
        }
        for (index, warning) in self.warnings.iter().enumerate() {
            validate_text(&format!("warnings[{index}]"), warning, 1024)?;
        }
        Ok(())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RegistryNamespaceError {
    #[error("unsupported registry namespace schema `{found}`; supported schema is `{supported}`")]
    UnsupportedSchema { found: String, supported: String },
    #[error("registry namespace request must contain at least one provider")]
    NoProviders,
    #[error("registry namespace plan must contain at least one provider entry")]
    NoEntries,
    #[error("provider `{provider:?}` appears more than once")]
    DuplicateProvider { provider: RegistryNamespaceProvider },
    #[error("provider `{provider:?}` was not requested")]
    UnrequestedProvider { provider: RegistryNamespaceProvider },
    #[error("requested provider `{provider:?}` has no plan entry")]
    MissingProvider { provider: RegistryNamespaceProvider },
    #[error("provider `{provider:?}` requires model `{expected:?}`, found `{found:?}`")]
    ProviderModelMismatch {
        provider: RegistryNamespaceProvider,
        expected: RegistryNamespaceModel,
        found: RegistryNamespaceModel,
    },
    #[error("provider `{provider:?}` requires a coordinate")]
    CoordinateRequired { provider: RegistryNamespaceProvider },
    #[error("crates.io has no organization namespace coordinate to reserve")]
    CratesIoCannotReserveCoordinate,
    #[error("crates.io plan entries require an advisory package prefix")]
    CratesIoPrefixRequired,
    #[error("crates.io cannot produce a reserved organization-namespace receipt")]
    CratesIoCannotBeReserved,
    #[error("provider `{provider:?}` has an inconsistent not-reservable state")]
    NotReservableInconsistency { provider: RegistryNamespaceProvider },
    #[error("provider `{provider:?}` cannot carry an advisory package prefix")]
    UnexpectedPackagePrefix { provider: RegistryNamespaceProvider },
    #[error("provider `{provider:?}` repeats proof `{proof:?}")]
    DuplicateProof {
        provider: RegistryNamespaceProvider,
        proof: RegistryNamespaceProof,
    },
    #[error("provider `{provider:?}` must contain at least one claim step")]
    NoSteps { provider: RegistryNamespaceProvider },
    #[error("receipt outcome `{outcome:?}` requires non-secret evidence")]
    EvidenceRequired {
        outcome: RegistryNamespaceClaimOutcome,
    },
    #[error("invalid portable slug in `{field}`: `{value}`")]
    InvalidPortableSlug { field: String, value: String },
    #[error("invalid canonical domain in `{field}`: `{value}`")]
    InvalidDomain { field: String, value: String },
    #[error("invalid provider coordinate in `{field}`: `{value}`")]
    InvalidCoordinate { field: String, value: String },
    #[error("invalid package prefix in `{field}`: `{value}`")]
    InvalidPackagePrefix { field: String, value: String },
    #[error("invalid text in `{field}`")]
    InvalidText { field: String },
    #[error("invalid lowercase SHA-256 in `{field}`: `{value}`")]
    InvalidSha256 { field: String, value: String },
    #[error("registry namespace serialization failed: {0}")]
    Serialization(String),
}

fn validate_portable_slug(field: &str, value: &str) -> Result<(), RegistryNamespaceError> {
    let bytes = value.as_bytes();
    let valid = !value.is_empty()
        && value.len() <= 39
        && value == value.to_ascii_lowercase()
        && bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
        && !value.contains("--");
    if valid {
        Ok(())
    } else {
        Err(RegistryNamespaceError::InvalidPortableSlug {
            field: field.to_owned(),
            value: value.to_owned(),
        })
    }
}

fn validate_domain(field: &str, value: &str) -> Result<(), RegistryNamespaceError> {
    let valid = !value.is_empty()
        && value.len() <= 253
        && value == value.to_ascii_lowercase()
        && value.contains('.')
        && !value.ends_with('.')
        && !value.contains(['/', ':', '@', ' ', '\\'])
        && value.split('.').all(|label| {
            let bytes = label.as_bytes();
            !label.is_empty()
                && label.len() <= 63
                && bytes.first().is_some_and(u8::is_ascii_alphanumeric)
                && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
                && bytes
                    .iter()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
        });
    if valid {
        Ok(())
    } else {
        Err(RegistryNamespaceError::InvalidDomain {
            field: field.to_owned(),
            value: value.to_owned(),
        })
    }
}

fn validate_coordinate(field: &str, value: &str) -> Result<(), RegistryNamespaceError> {
    let valid = !value.is_empty()
        && value.len() <= 255
        && value.trim() == value
        && value.is_ascii()
        && !value.chars().any(char::is_control)
        && !value.chars().any(char::is_whitespace);
    if valid {
        Ok(())
    } else {
        Err(RegistryNamespaceError::InvalidCoordinate {
            field: field.to_owned(),
            value: value.to_owned(),
        })
    }
}

fn validate_package_prefix(field: &str, value: &str) -> Result<(), RegistryNamespaceError> {
    let stem = value.strip_suffix('-').unwrap_or(value);
    if value.ends_with('-') && validate_portable_slug(field, stem).is_ok() {
        Ok(())
    } else {
        Err(RegistryNamespaceError::InvalidPackagePrefix {
            field: field.to_owned(),
            value: value.to_owned(),
        })
    }
}

fn validate_text(field: &str, value: &str, max: usize) -> Result<(), RegistryNamespaceError> {
    let valid = !value.is_empty()
        && value.len() <= max
        && value.trim() == value
        && !value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'));
    if valid {
        Ok(())
    } else {
        Err(RegistryNamespaceError::InvalidText {
            field: field.to_owned(),
        })
    }
}

fn validate_sha256(field: &str, value: &str) -> Result<(), RegistryNamespaceError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        Ok(())
    } else {
        Err(RegistryNamespaceError::InvalidSha256 {
            field: field.to_owned(),
            value: value.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(action: RegistryNamespaceAction, summary: &str, manual: bool) -> RegistryNamespaceStep {
        RegistryNamespaceStep {
            action,
            summary: summary.to_owned(),
            manual,
            prerequisite: None,
        }
    }

    fn request() -> RegistryNamespaceRequest {
        RegistryNamespaceRequest {
            brand: "acme-cloud".to_owned(),
            domain: Some("acme.example".to_owned()),
            github_owner: Some("acme-cloud".to_owned()),
            providers: RegistryNamespaceProvider::ALL.to_vec(),
        }
    }

    fn entry(provider: RegistryNamespaceProvider) -> RegistryNamespaceEntry {
        let (coordinate, package_prefix, automation, disposition) = match provider {
            RegistryNamespaceProvider::Npm => (
                Some("@acme-cloud".to_owned()),
                None,
                RegistryNamespaceAutomation::ManualWebFlow,
                RegistryNamespaceDisposition::ManualActionRequired,
            ),
            RegistryNamespaceProvider::MavenCentral => (
                Some("example.acme".to_owned()),
                None,
                RegistryNamespaceAutomation::ManualWebFlow,
                RegistryNamespaceDisposition::ManualActionRequired,
            ),
            RegistryNamespaceProvider::CratesIo => (
                None,
                Some("acme-cloud-".to_owned()),
                RegistryNamespaceAutomation::NotReservable,
                RegistryNamespaceDisposition::NotReservable,
            ),
            RegistryNamespaceProvider::PubDev => (
                Some("acme.example".to_owned()),
                None,
                RegistryNamespaceAutomation::ManualWebFlow,
                RegistryNamespaceDisposition::ManualActionRequired,
            ),
            RegistryNamespaceProvider::GitHub
            | RegistryNamespaceProvider::GitLabCom
            | RegistryNamespaceProvider::BitbucketCloud => (
                Some("acme-cloud".to_owned()),
                None,
                RegistryNamespaceAutomation::ManualWebFlow,
                RegistryNamespaceDisposition::ManualActionRequired,
            ),
        };
        RegistryNamespaceEntry {
            provider,
            model: provider.expected_model(),
            coordinate,
            package_prefix,
            automation,
            disposition,
            proofs: vec![RegistryNamespaceProof::RegistryAccountControl],
            steps: vec![step(
                RegistryNamespaceAction::CheckAvailability,
                "Check the provider without claiming ownership.",
                false,
            )],
            warnings: Vec::new(),
        }
    }

    fn plan() -> RegistryNamespacePlan {
        RegistryNamespacePlan {
            schema: RegistryNamespacePlan::SCHEMA_V1.to_owned(),
            request: request(),
            entries: RegistryNamespaceProvider::ALL
                .into_iter()
                .rev()
                .map(entry)
                .collect(),
            warnings: vec![
                "A plan is not an ownership receipt.".to_owned(),
                "Package names remain provider-specific.".to_owned(),
            ],
        }
    }

    #[test]
    fn canonical_plan_orders_providers_and_warnings() {
        let plan = plan();
        plan.validate().unwrap();
        let normalized = plan.normalized();
        assert_eq!(
            normalized.entries[0].provider,
            RegistryNamespaceProvider::Npm
        );
        assert_eq!(
            normalized.entries.last().unwrap().provider,
            RegistryNamespaceProvider::BitbucketCloud
        );
        assert_eq!(
            normalized.warnings,
            vec![
                "A plan is not an ownership receipt.",
                "Package names remain provider-specific.",
            ]
        );
        assert_eq!(
            plan.canonical_json_bytes().unwrap(),
            normalized.canonical_json_bytes().unwrap()
        );
    }

    #[test]
    fn crates_io_cannot_be_misrepresented_as_a_reserved_org() {
        let mut plan = plan();
        let crates = plan
            .entries
            .iter_mut()
            .find(|entry| entry.provider == RegistryNamespaceProvider::CratesIo)
            .unwrap();
        crates.coordinate = Some("acme-cloud".to_owned());
        assert_eq!(
            plan.validate().unwrap_err(),
            RegistryNamespaceError::CratesIoCannotReserveCoordinate
        );

        let receipt = RegistryNamespaceClaimReceipt {
            schema: RegistryNamespaceClaimReceipt::SCHEMA_V1.to_owned(),
            plan_sha256: "a".repeat(64),
            provider: RegistryNamespaceProvider::CratesIo,
            coordinate: None,
            outcome: RegistryNamespaceClaimOutcome::Reserved,
            observed_at_unix_seconds: 1,
            evidence: vec![RegistryNamespaceEvidence {
                kind: RegistryNamespaceProof::ExistingPackageOwnership,
                subject: "acme-cloud-core".to_owned(),
                reference: None,
                sha256: None,
            }],
            warnings: Vec::new(),
        };
        assert_eq!(
            receipt.validate().unwrap_err(),
            RegistryNamespaceError::CratesIoCannotBeReserved
        );
    }

    #[test]
    fn provider_model_and_requested_set_fail_closed() {
        let mut model_plan = plan();
        model_plan.entries[0].model = RegistryNamespaceModel::LiteralOrganizationScope;
        assert!(matches!(
            model_plan.validate(),
            Err(RegistryNamespaceError::ProviderModelMismatch { .. })
        ));

        let mut requested_plan = plan();
        requested_plan.request.providers.pop();
        assert!(matches!(
            requested_plan.validate(),
            Err(RegistryNamespaceError::UnrequestedProvider { .. })
        ));
    }

    #[test]
    fn portable_identity_rejects_confusables_and_noncanonical_domains() {
        let mut brand_request = request();
        brand_request.brand = "acmе-cloud".to_owned(); // Cyrillic `е`.
        assert!(matches!(
            brand_request.validate(),
            Err(RegistryNamespaceError::InvalidPortableSlug { .. })
        ));

        let mut domain_request = request();
        domain_request.domain = Some("HTTPS://Acme.Example/".to_owned());
        assert!(matches!(
            domain_request.validate(),
            Err(RegistryNamespaceError::InvalidDomain { .. })
        ));
    }

    #[test]
    fn successful_receipts_require_evidence_and_validate_digest() {
        let receipt = RegistryNamespaceClaimReceipt {
            schema: RegistryNamespaceClaimReceipt::SCHEMA_V1.to_owned(),
            plan_sha256: "b".repeat(64),
            provider: RegistryNamespaceProvider::Npm,
            coordinate: Some("@acme-cloud".to_owned()),
            outcome: RegistryNamespaceClaimOutcome::Reserved,
            observed_at_unix_seconds: 1,
            evidence: vec![RegistryNamespaceEvidence {
                kind: RegistryNamespaceProof::RegistryAccountControl,
                subject: "@acme-cloud".to_owned(),
                reference: Some("npm-org:acme-cloud".to_owned()),
                sha256: None,
            }],
            warnings: Vec::new(),
        };
        receipt.validate().unwrap();

        let mut missing = receipt.clone();
        missing.evidence.clear();
        assert!(matches!(
            missing.validate(),
            Err(RegistryNamespaceError::EvidenceRequired { .. })
        ));

        let mut invalid = receipt;
        invalid.plan_sha256 = "B".repeat(64);
        assert!(matches!(
            invalid.validate(),
            Err(RegistryNamespaceError::InvalidSha256 { .. })
        ));
    }
}
