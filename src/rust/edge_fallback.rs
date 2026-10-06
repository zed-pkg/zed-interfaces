use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const EDGE_FALLBACK_CAPABILITY_VERSION_V1: u8 = 1;
pub const EDGE_FALLBACK_CAPABILITY_VERSION_V2: u8 = 2;
pub const EDGE_FALLBACK_CAPABILITY_VERSION_V3: u8 = 3;
pub const EDGE_FALLBACK_AUDIENCE_V1: &str = "zed-edge-fallback";
pub const EDGE_FALLBACK_AUDIENCE_V2: &str = EDGE_FALLBACK_AUDIENCE_V1;
pub const EDGE_FALLBACK_AUDIENCE_V3: &str = EDGE_FALLBACK_AUDIENCE_V1;
pub const EDGE_FALLBACK_MAX_GRANTS_V1: usize = 16;
pub const EDGE_FALLBACK_MAX_GRANTS_V2: usize = EDGE_FALLBACK_MAX_GRANTS_V1;
pub const EDGE_FALLBACK_MAX_GRANTS_V3: usize = EDGE_FALLBACK_MAX_GRANTS_V1;
pub const EDGE_FALLBACK_MAX_TTL_SECONDS_V1: u64 = 300;
pub const EDGE_FALLBACK_MAX_TTL_SECONDS_V2: u64 = EDGE_FALLBACK_MAX_TTL_SECONDS_V1;
pub const EDGE_FALLBACK_MAX_TTL_SECONDS_V3: u64 = EDGE_FALLBACK_MAX_TTL_SECONDS_V1;
pub const EDGE_FALLBACK_MAX_JWKS_STALENESS_SECONDS_V3: u64 = EDGE_FALLBACK_MAX_TTL_SECONDS_V3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EdgeFallbackCapabilityV1 {
    pub zed_edge_capability: u8,
    pub iss: String,
    pub aud: String,
    pub sub: String,
    pub iat: u64,
    pub nbf: u64,
    pub exp: u64,
    pub jti: String,
    pub grants: Vec<EdgeFallbackGrantV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EdgeFallbackCapabilityV2 {
    pub zed_edge_capability: u8,
    pub iss: String,
    pub aud: String,
    pub sub: String,
    pub sid: String,
    pub parent_jti: String,
    pub iat: u64,
    pub nbf: u64,
    pub exp: u64,
    pub jti: String,
    pub grants: Vec<EdgeFallbackGrantV2>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EdgeFallbackCapabilityV3 {
    pub zed_edge_capability: u8,
    pub iss: String,
    pub aud: String,
    pub sub: String,
    pub sid: String,
    pub parent_jti: String,
    pub assurance: u8,
    pub session_epoch: u64,
    pub policy_epoch: u64,
    pub revocation_checked_at: u64,
    pub iat: u64,
    pub nbf: u64,
    pub exp: u64,
    pub jti: String,
    pub grants: Vec<EdgeFallbackGrantV3>,
}

/// Trusted verifier-local bounds. These values are deployment policy, not
/// caller-controlled claims and must be loaded from reviewed configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EdgeFallbackOutagePolicyV3 {
    pub minimum_assurance: u8,
    pub max_capability_age_seconds: u64,
    pub max_revocation_age_seconds: u64,
    pub max_jwks_age_seconds: u64,
    pub max_outage_seconds: u64,
}

pub type EdgeFallbackGrantV2 = EdgeFallbackGrantV1;
pub type ReadOperationV2 = ReadOperationV1;
pub type EdgeFallbackGrantV3 = EdgeFallbackGrantV1;
pub type ReadOperationV3 = ReadOperationV1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "provider", deny_unknown_fields)]
pub enum EdgeFallbackGrantV1 {
    #[serde(rename = "github")]
    Github {
        operation: ReadOperationV1,
        package: String,
        resource: String,
        credential_ref: String,
    },
    #[serde(rename = "npm")]
    Npm {
        operation: ReadOperationV1,
        package: String,
        resource: String,
        credential_ref: String,
    },
    #[serde(rename = "cargo-registry")]
    CargoRegistry {
        operation: ReadOperationV1,
        package: String,
        resource: String,
        origin: String,
        credential_ref: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReadOperationV1 {
    #[serde(rename = "read")]
    Read,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EdgeFallbackContractError {
    #[error("capability version is invalid")]
    InvalidVersion,
    #[error("audience must be zed-edge-fallback")]
    InvalidAudience,
    #[error("issuer is invalid")]
    InvalidIssuer,
    #[error("subject is invalid")]
    InvalidSubject,
    #[error("capability id is invalid")]
    InvalidJti,
    #[error("session lineage is invalid")]
    InvalidLineage,
    #[error("assurance is invalid")]
    InvalidAssurance,
    #[error("revocation checkpoint is invalid")]
    InvalidRevocationCheckpoint,
    #[error("capability lifetime is invalid")]
    InvalidLifetime,
    #[error("bounded outage policy rejected the capability")]
    OutagePolicyRejected,
    #[error("capability must contain 1..16 grants")]
    InvalidGrantCount,
    #[error("package coordinate is invalid")]
    InvalidPackage,
    #[error("provider resource is invalid")]
    InvalidResource,
    #[error("credential reference is invalid")]
    InvalidCredentialRef,
    #[error("cargo registry origin is invalid")]
    InvalidCargoOrigin,
}

impl EdgeFallbackCapabilityV1 {
    pub fn validate(&self) -> Result<(), EdgeFallbackContractError> {
        if self.zed_edge_capability != EDGE_FALLBACK_CAPABILITY_VERSION_V1 {
            return Err(EdgeFallbackContractError::InvalidVersion);
        }
        if self.aud != EDGE_FALLBACK_AUDIENCE_V1 {
            return Err(EdgeFallbackContractError::InvalidAudience);
        }
        if !bounded_text(&self.iss, 1, 512) {
            return Err(EdgeFallbackContractError::InvalidIssuer);
        }
        if !bounded_text(&self.sub, 1, 256) {
            return Err(EdgeFallbackContractError::InvalidSubject);
        }
        if !valid_jti(&self.jti) {
            return Err(EdgeFallbackContractError::InvalidJti);
        }
        if self.exp <= self.iat || self.nbf < self.iat || self.nbf > self.exp {
            return Err(EdgeFallbackContractError::InvalidLifetime);
        }
        if self.exp - self.iat > EDGE_FALLBACK_MAX_TTL_SECONDS_V1 {
            return Err(EdgeFallbackContractError::InvalidLifetime);
        }
        if self.grants.is_empty() || self.grants.len() > EDGE_FALLBACK_MAX_GRANTS_V1 {
            return Err(EdgeFallbackContractError::InvalidGrantCount);
        }
        self.grants
            .iter()
            .try_for_each(EdgeFallbackGrantV1::validate)
    }
}

impl EdgeFallbackCapabilityV2 {
    pub fn validate(&self) -> Result<(), EdgeFallbackContractError> {
        if self.zed_edge_capability != EDGE_FALLBACK_CAPABILITY_VERSION_V2 {
            return Err(EdgeFallbackContractError::InvalidVersion);
        }
        if self.aud != EDGE_FALLBACK_AUDIENCE_V2 {
            return Err(EdgeFallbackContractError::InvalidAudience);
        }
        if !bounded_text(&self.iss, 1, 512) {
            return Err(EdgeFallbackContractError::InvalidIssuer);
        }
        if !bounded_text(&self.sub, 1, 256) {
            return Err(EdgeFallbackContractError::InvalidSubject);
        }
        if !valid_lineage_id(&self.sid) || !valid_lineage_id(&self.parent_jti) {
            return Err(EdgeFallbackContractError::InvalidLineage);
        }
        if !valid_jti(&self.jti) {
            return Err(EdgeFallbackContractError::InvalidJti);
        }
        if self.exp <= self.iat || self.nbf < self.iat || self.nbf > self.exp {
            return Err(EdgeFallbackContractError::InvalidLifetime);
        }
        if self.exp - self.iat > EDGE_FALLBACK_MAX_TTL_SECONDS_V2 {
            return Err(EdgeFallbackContractError::InvalidLifetime);
        }
        if self.grants.is_empty() || self.grants.len() > EDGE_FALLBACK_MAX_GRANTS_V2 {
            return Err(EdgeFallbackContractError::InvalidGrantCount);
        }
        self.grants
            .iter()
            .try_for_each(EdgeFallbackGrantV2::validate)
    }
}

impl EdgeFallbackCapabilityV3 {
    pub fn validate(&self) -> Result<(), EdgeFallbackContractError> {
        if self.zed_edge_capability != EDGE_FALLBACK_CAPABILITY_VERSION_V3 {
            return Err(EdgeFallbackContractError::InvalidVersion);
        }
        if self.aud != EDGE_FALLBACK_AUDIENCE_V3 {
            return Err(EdgeFallbackContractError::InvalidAudience);
        }
        if !bounded_text(&self.iss, 1, 512) {
            return Err(EdgeFallbackContractError::InvalidIssuer);
        }
        if !bounded_text(&self.sub, 1, 256) {
            return Err(EdgeFallbackContractError::InvalidSubject);
        }
        if !valid_lineage_id(&self.sid) || !valid_lineage_id(&self.parent_jti) {
            return Err(EdgeFallbackContractError::InvalidLineage);
        }
        if !matches!(self.assurance, 1 | 2) {
            return Err(EdgeFallbackContractError::InvalidAssurance);
        }
        if self.revocation_checked_at > self.iat {
            return Err(EdgeFallbackContractError::InvalidRevocationCheckpoint);
        }
        if !valid_jti(&self.jti) {
            return Err(EdgeFallbackContractError::InvalidJti);
        }
        if self.exp <= self.iat || self.nbf < self.iat || self.nbf > self.exp {
            return Err(EdgeFallbackContractError::InvalidLifetime);
        }
        if self.exp - self.iat > EDGE_FALLBACK_MAX_TTL_SECONDS_V3 {
            return Err(EdgeFallbackContractError::InvalidLifetime);
        }
        if self.grants.is_empty() || self.grants.len() > EDGE_FALLBACK_MAX_GRANTS_V3 {
            return Err(EdgeFallbackContractError::InvalidGrantCount);
        }
        self.grants
            .iter()
            .try_for_each(EdgeFallbackGrantV3::validate)
    }

    /// Apply outage-only admission using verifier-local freshness facts.
    /// JOSE signature, algorithm, issuer and unknown-kid rejection happen
    /// before this method; a capability cannot self-assert JWKS freshness.
    pub fn admit_outage(
        &self,
        now: u64,
        outage_started_at: u64,
        jwks_refreshed_at: u64,
        policy: EdgeFallbackOutagePolicyV3,
    ) -> Result<(), EdgeFallbackContractError> {
        self.validate()?;

        if !matches!(policy.minimum_assurance, 1 | 2)
            || policy.max_capability_age_seconds > EDGE_FALLBACK_MAX_TTL_SECONDS_V3
            || policy.max_revocation_age_seconds > EDGE_FALLBACK_MAX_TTL_SECONDS_V3
            || policy.max_outage_seconds > EDGE_FALLBACK_MAX_TTL_SECONDS_V3
            || policy.max_jwks_age_seconds > EDGE_FALLBACK_MAX_JWKS_STALENESS_SECONDS_V3
        {
            return Err(EdgeFallbackContractError::OutagePolicyRejected);
        }
        if self.assurance < policy.minimum_assurance || now < self.nbf || now >= self.exp {
            return Err(EdgeFallbackContractError::OutagePolicyRejected);
        }

        let capability_age = now
            .checked_sub(self.iat)
            .ok_or(EdgeFallbackContractError::OutagePolicyRejected)?;
        let revocation_age = now
            .checked_sub(self.revocation_checked_at)
            .ok_or(EdgeFallbackContractError::OutagePolicyRejected)?;
        let jwks_age = now
            .checked_sub(jwks_refreshed_at)
            .ok_or(EdgeFallbackContractError::OutagePolicyRejected)?;
        let outage_age = now
            .checked_sub(outage_started_at)
            .ok_or(EdgeFallbackContractError::OutagePolicyRejected)?;

        if capability_age > policy.max_capability_age_seconds
            || revocation_age > policy.max_revocation_age_seconds
            || jwks_age > policy.max_jwks_age_seconds
            || outage_age > policy.max_outage_seconds
        {
            return Err(EdgeFallbackContractError::OutagePolicyRejected);
        }

        Ok(())
    }
}

impl EdgeFallbackGrantV1 {
    pub fn validate(&self) -> Result<(), EdgeFallbackContractError> {
        match self {
            Self::Github {
                package,
                resource,
                credential_ref,
                ..
            } => {
                validate_package(package)?;
                if !valid_github_repo(resource) {
                    return Err(EdgeFallbackContractError::InvalidResource);
                }
                validate_credential_ref(credential_ref)
            }
            Self::Npm {
                package,
                resource,
                credential_ref,
                ..
            } => {
                validate_package(package)?;
                if !valid_npm_package(resource) {
                    return Err(EdgeFallbackContractError::InvalidResource);
                }
                validate_credential_ref(credential_ref)
            }
            Self::CargoRegistry {
                package,
                resource,
                origin,
                credential_ref,
                ..
            } => {
                validate_package(package)?;
                if !valid_cargo_resource(resource) {
                    return Err(EdgeFallbackContractError::InvalidResource);
                }
                if !valid_https_origin(origin) {
                    return Err(EdgeFallbackContractError::InvalidCargoOrigin);
                }
                validate_credential_ref(credential_ref)
            }
        }
    }

    pub fn provider(&self) -> &'static str {
        match self {
            Self::Github { .. } => "github",
            Self::Npm { .. } => "npm",
            Self::CargoRegistry { .. } => "cargo-registry",
        }
    }

    pub fn package(&self) -> &str {
        match self {
            Self::Github { package, .. }
            | Self::Npm { package, .. }
            | Self::CargoRegistry { package, .. } => package,
        }
    }

    pub fn resource(&self) -> &str {
        match self {
            Self::Github { resource, .. }
            | Self::Npm { resource, .. }
            | Self::CargoRegistry { resource, .. } => resource,
        }
    }

    pub fn credential_ref(&self) -> &str {
        match self {
            Self::Github { credential_ref, .. }
            | Self::Npm { credential_ref, .. }
            | Self::CargoRegistry { credential_ref, .. } => credential_ref,
        }
    }
}

fn validate_package(value: &str) -> Result<(), EdgeFallbackContractError> {
    if valid_package_coordinate(value) {
        Ok(())
    } else {
        Err(EdgeFallbackContractError::InvalidPackage)
    }
}

fn validate_credential_ref(value: &str) -> Result<(), EdgeFallbackContractError> {
    if valid_credential_ref(value) {
        Ok(())
    } else {
        Err(EdgeFallbackContractError::InvalidCredentialRef)
    }
}

fn bounded_text(value: &str, min: usize, max: usize) -> bool {
    let len = value.len();
    len >= min && len <= max && value.trim() == value && !value.chars().any(char::is_control)
}

fn valid_package_coordinate(value: &str) -> bool {
    let mut parts = value.split('/');
    let Some(org) = parts.next() else {
        return false;
    };
    let Some(name) = parts.next() else {
        return false;
    };
    parts.next().is_none() && valid_slug(org) && valid_slug(name) && value.len() <= 257
}

fn valid_slug(value: &str) -> bool {
    if value.is_empty() || value.len() > 128 {
        return false;
    }
    let bytes = value.as_bytes();
    bytes[0].is_ascii_lowercase()
        && bytes[bytes.len() - 1].is_ascii_alphanumeric()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
        && !value.contains("--")
}

fn valid_github_repo(value: &str) -> bool {
    let mut parts = value.split('/');
    let Some(owner) = parts.next() else {
        return false;
    };
    let Some(repo) = parts.next() else {
        return false;
    };
    parts.next().is_none()
        && valid_github_component(owner)
        && valid_github_component(repo)
        && value.len() <= 256
}

fn valid_github_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-'))
}

fn valid_npm_package(value: &str) -> bool {
    if !value.starts_with('@') || value.len() > 256 {
        return false;
    }
    let Some((scope, name)) = value[1..].split_once('/') else {
        return false;
    };
    valid_npm_component(scope) && valid_npm_component(name)
}

fn valid_npm_component(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'.' | b'-')
        })
}

fn valid_cargo_resource(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphanumeric() || (index > 0 && matches!(byte, b'_' | b'-'))
        })
}

fn valid_https_origin(value: &str) -> bool {
    let Some(authority) = value.strip_prefix("https://") else {
        return false;
    };
    if authority.is_empty() || authority.len() > 504 || authority.contains(['/', '?', '#', '@']) {
        return false;
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => (host, Some(port)),
        _ => (authority, None),
    };
    if host.is_empty()
        || host.starts_with('.')
        || host.ends_with('.')
        || !host
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
    {
        return false;
    }
    match port {
        None => true,
        Some(value) => {
            !value.is_empty()
                && value.len() <= 5
                && value.bytes().all(|byte| byte.is_ascii_digit())
                && value.parse::<u16>().is_ok_and(|port| port > 0)
        }
    }
}

fn valid_credential_ref(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 192
        && value.bytes().enumerate().all(|(index, byte)| {
            if index == 0 {
                return byte.is_ascii_alphanumeric();
            }
            byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'.' | b'_' | b'/' | b'-')
        })
}

fn valid_lineage_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && value.bytes().enumerate().all(|(index, byte)| {
            if index == 0 {
                return byte.is_ascii_alphanumeric();
            }
            byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'.' | b'_' | b'/' | b'-')
        })
}

fn valid_jti(value: &str) -> bool {
    value.len() >= 8
        && value.len() <= 192
        && value.bytes().enumerate().all(|(index, byte)| {
            if index == 0 {
                return byte.is_ascii_alphanumeric();
            }
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_capability() -> EdgeFallbackCapabilityV1 {
        EdgeFallbackCapabilityV1 {
            zed_edge_capability: 1,
            iss: "https://api.zpkg.net".into(),
            aud: EDGE_FALLBACK_AUDIENCE_V1.into(),
            sub: "user:test".into(),
            iat: 100,
            nbf: 100,
            exp: 220,
            jti: "capability-0001".into(),
            grants: vec![EdgeFallbackGrantV1::Github {
                operation: ReadOperationV1::Read,
                package: "acme/private-lib".into(),
                resource: "acme/private-lib".into(),
                credential_ref: "github-app:acme:installation-42".into(),
            }],
        }
    }

    fn valid_capability_v2() -> EdgeFallbackCapabilityV2 {
        EdgeFallbackCapabilityV2 {
            zed_edge_capability: 2,
            iss: "https://api.zpkg.net".into(),
            aud: EDGE_FALLBACK_AUDIENCE_V2.into(),
            sub: "user:test".into(),
            sid: "session:abc-123".into(),
            parent_jti: "parent-token-0001".into(),
            iat: 100,
            nbf: 100,
            exp: 220,
            jti: "capability-0002".into(),
            grants: vec![EdgeFallbackGrantV2::Github {
                operation: ReadOperationV2::Read,
                package: "acme/private-lib".into(),
                resource: "acme/private-lib".into(),
                credential_ref: "github-app:acme:installation-42".into(),
            }],
        }
    }

    fn valid_capability_v3() -> EdgeFallbackCapabilityV3 {
        EdgeFallbackCapabilityV3 {
            zed_edge_capability: 3,
            iss: "https://api.zpkg.net".into(),
            aud: EDGE_FALLBACK_AUDIENCE_V3.into(),
            sub: "user:test".into(),
            sid: "session:abc-123".into(),
            parent_jti: "parent-token-0001".into(),
            assurance: 2,
            session_epoch: 7,
            policy_epoch: 4,
            revocation_checked_at: 95,
            iat: 100,
            nbf: 100,
            exp: 220,
            jti: "capability-0003".into(),
            grants: vec![EdgeFallbackGrantV3::Github {
                operation: ReadOperationV3::Read,
                package: "acme/private-lib".into(),
                resource: "acme/private-lib".into(),
                credential_ref: "github-app:acme:installation-42".into(),
            }],
        }
    }

    fn outage_policy_v3() -> EdgeFallbackOutagePolicyV3 {
        EdgeFallbackOutagePolicyV3 {
            minimum_assurance: 2,
            max_capability_age_seconds: 120,
            max_revocation_age_seconds: 30,
            max_jwks_age_seconds: 300,
            max_outage_seconds: 120,
        }
    }

    #[test]
    fn valid_capability_round_trips_with_provider_tag() {
        let capability = valid_capability();
        capability.validate().unwrap();

        let json = serde_json::to_value(&capability).unwrap();
        assert_eq!(json["grants"][0]["provider"], "github");
        assert_eq!(json["grants"][0]["operation"], "read");
        assert!(json["grants"][0].get("token").is_none());

        let decoded: EdgeFallbackCapabilityV1 = serde_json::from_value(json).unwrap();
        assert_eq!(decoded, capability);
    }

    #[test]
    fn v2_round_trips_signed_lineage_and_rejects_invalid_lineage() {
        let capability = valid_capability_v2();
        capability.validate().unwrap();

        let json = serde_json::to_value(&capability).unwrap();
        assert_eq!(json["zed_edge_capability"], 2);
        assert_eq!(json["sid"], "session:abc-123");
        assert_eq!(json["parent_jti"], "parent-token-0001");
        assert!(json.get("access_token").is_none());

        let decoded: EdgeFallbackCapabilityV2 = serde_json::from_value(json).unwrap();
        assert_eq!(decoded, capability);

        let mut invalid = valid_capability_v2();
        invalid.parent_jti = "parent token with spaces".into();
        assert_eq!(
            invalid.validate(),
            Err(EdgeFallbackContractError::InvalidLineage)
        );
    }

    #[test]
    fn v3_binds_assurance_epochs_and_revocation_checkpoint() {
        let capability = valid_capability_v3();
        capability.validate().unwrap();

        let json = serde_json::to_value(&capability).unwrap();
        assert_eq!(json["zed_edge_capability"], 3);
        assert_eq!(json["assurance"], 2);
        assert_eq!(json["session_epoch"], 7);
        assert_eq!(json["policy_epoch"], 4);
        assert_eq!(json["revocation_checked_at"], 95);
        assert!(json.get("access_token").is_none());

        let decoded: EdgeFallbackCapabilityV3 = serde_json::from_value(json).unwrap();
        assert_eq!(decoded, capability);

        let mut invalid = valid_capability_v3();
        invalid.assurance = 0;
        assert_eq!(
            invalid.validate(),
            Err(EdgeFallbackContractError::InvalidAssurance)
        );

        let mut impossible = valid_capability_v3();
        impossible.revocation_checked_at = impossible.iat + 1;
        assert_eq!(
            impossible.validate(),
            Err(EdgeFallbackContractError::InvalidRevocationCheckpoint)
        );
    }

    #[test]
    fn v3_outage_admission_fails_closed_on_staleness_expiry_and_assurance() {
        let capability = valid_capability_v3();
        let policy = outage_policy_v3();
        capability.admit_outage(110, 105, 90, policy).unwrap();

        let mut low_assurance = capability.clone();
        low_assurance.assurance = 1;
        assert_eq!(
            low_assurance.admit_outage(110, 105, 90, policy),
            Err(EdgeFallbackContractError::OutagePolicyRejected)
        );

        assert_eq!(
            capability.admit_outage(220, 105, 90, policy),
            Err(EdgeFallbackContractError::OutagePolicyRejected)
        );
        assert_eq!(
            capability.admit_outage(130, 105, 90, EdgeFallbackOutagePolicyV3 {
                max_revocation_age_seconds: 20,
                ..policy
            }),
            Err(EdgeFallbackContractError::OutagePolicyRejected)
        );
        assert_eq!(
            capability.admit_outage(110, 105, 0, EdgeFallbackOutagePolicyV3 {
                max_jwks_age_seconds: 100,
                ..policy
            }),
            Err(EdgeFallbackContractError::OutagePolicyRejected)
        );
        assert_eq!(
            capability.admit_outage(130, 0, 90, EdgeFallbackOutagePolicyV3 {
                max_outage_seconds: 120,
                ..policy
            }),
            Err(EdgeFallbackContractError::OutagePolicyRejected)
        );
    }

    #[test]
    fn v3_rejects_unbounded_local_outage_policy() {
        let capability = valid_capability_v3();
        for policy in [
            EdgeFallbackOutagePolicyV3 {
                max_capability_age_seconds: EDGE_FALLBACK_MAX_TTL_SECONDS_V3 + 1,
                ..outage_policy_v3()
            },
            EdgeFallbackOutagePolicyV3 {
                max_revocation_age_seconds: EDGE_FALLBACK_MAX_TTL_SECONDS_V3 + 1,
                ..outage_policy_v3()
            },
            EdgeFallbackOutagePolicyV3 {
                max_outage_seconds: EDGE_FALLBACK_MAX_TTL_SECONDS_V3 + 1,
                ..outage_policy_v3()
            },
            EdgeFallbackOutagePolicyV3 {
                max_jwks_age_seconds: EDGE_FALLBACK_MAX_JWKS_STALENESS_SECONDS_V3 + 1,
                ..outage_policy_v3()
            },
        ] {
            assert_eq!(
                capability.admit_outage(110, 105, 90, policy),
                Err(EdgeFallbackContractError::OutagePolicyRejected)
            );
        }
    }

    #[test]
    fn v1_and_v2_reject_inconsistent_temporal_ordering() {
        let mut v1 = valid_capability();
        v1.nbf = v1.iat - 1;
        assert_eq!(
            v1.validate(),
            Err(EdgeFallbackContractError::InvalidLifetime)
        );

        let mut v2 = valid_capability_v2();
        v2.nbf = v2.exp + 1;
        assert_eq!(
            v2.validate(),
            Err(EdgeFallbackContractError::InvalidLifetime)
        );
    }

    #[test]
    fn rejects_unknown_secret_bearing_fields() {
        let value = serde_json::json!({
            "zed_edge_capability": 1,
            "iss": "https://api.zpkg.net",
            "aud": "zed-edge-fallback",
            "sub": "user:test",
            "iat": 100,
            "nbf": 100,
            "exp": 220,
            "jti": "capability-0001",
            "grants": [{
                "provider": "github",
                "operation": "read",
                "package": "acme/private-lib",
                "resource": "acme/private-lib",
                "credential_ref": "github-app:acme:installation-42",
                "access_token": "must-never-appear"
            }]
        });
        assert!(serde_json::from_value::<EdgeFallbackCapabilityV1>(value).is_err());
    }

    #[test]
    fn rejects_lifetime_and_scope_widening() {
        let mut capability = valid_capability();
        capability.exp = capability.iat + EDGE_FALLBACK_MAX_TTL_SECONDS_V1 + 1;
        assert_eq!(
            capability.validate(),
            Err(EdgeFallbackContractError::InvalidLifetime)
        );

        let mut capability = valid_capability();
        capability.grants = vec![EdgeFallbackGrantV1::Github {
            operation: ReadOperationV1::Read,
            package: "acme/private-lib".into(),
            resource: "acme/private-lib/releases".into(),
            credential_ref: "github-app:acme:installation-42".into(),
        }];
        assert_eq!(
            capability.validate(),
            Err(EdgeFallbackContractError::InvalidResource)
        );
    }

    #[test]
    fn provider_helpers_are_secret_free() {
        let grant = EdgeFallbackGrantV1::Npm {
            operation: ReadOperationV1::Read,
            package: "acme/private-lib".into(),
            resource: "@acme/private-lib".into(),
            credential_ref: "npm:acme:private-lib".into(),
        };
        grant.validate().unwrap();
        assert_eq!(grant.provider(), "npm");
        assert_eq!(grant.package(), "acme/private-lib");
        assert_eq!(grant.resource(), "@acme/private-lib");
        assert_eq!(grant.credential_ref(), "npm:acme:private-lib");
    }
}
