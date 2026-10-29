//! Domain-separated, device-signed build assertions for Zed Package.
//!
//! A valid publisher signature attributes a *claim*. It is NOT evidence that
//! the device actually compiled the source, and it cannot consume an Oreslang
//! API challenge or promote a release. Those require server-side durable state.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::signing::{DetachedSignatureV1, SIGNING_ALGORITHM};

pub const SIGNED_BUILD_SCHEMA_V1: &str = "zpkg.signed-build/1";
pub const SIGNED_BUILD_AUDIENCE_V1: &str = "zed-pkg/oreslang-build-api/v1";
const PREIMAGE_DOMAIN: &[u8] = b"zpkg.signed-build/1\n";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BuildClaimsV1 {
    pub audience: String,
    pub org: String,
    pub package: String,
    pub version: String,
    pub challenge_id: String,
    /// Hex SHA-256 of the API-provided 256-bit nonce. Not an upload token.
    pub nonce_sha256: String,
    pub device_key_id: String,
    pub source_commit: String,
    pub source_sha256: String,
    pub toolchain_sha256: String,
    pub target: String,
    pub artifact_sha256: String,
    pub artifact_size: u64,
    /// Exact archive suffix used in Zed's content-addressed store.
    pub artifact_format: String,
    pub issued_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SignedBuildV1 {
    pub schema: String,
    pub claims: BuildClaimsV1,
    pub signatures: Vec<DetachedSignatureV1>,
}

fn lower_hex(s: &str, n: usize) -> bool {
    s.len() == n && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn slug(s: &str) -> bool {
    (1..=128).contains(&s.len())
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        && s.bytes().any(|b| b.is_ascii_alphanumeric())
}
fn challenge_id(s: &str) -> bool {
    s.len() == 36 && s.chars().enumerate().all(|(i, c)| {
        if [8, 13, 18, 23].contains(&i) { c == '-' }
        else { c.is_ascii_hexdigit() && !c.is_ascii_uppercase() }
    })
}

impl BuildClaimsV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.audience != SIGNED_BUILD_AUDIENCE_V1 {
            return Err("signed build audience mismatch".into());
        }
        if !slug(&self.org) || !slug(&self.package) || !slug(&self.device_key_id)
            || self.version.is_empty() || self.version.len() > 128
            || self.version.bytes().any(|b| b.is_ascii_control() || b == b'/') {
            return Err("invalid build identity".into());
        }
        if !challenge_id(&self.challenge_id) || !lower_hex(&self.nonce_sha256, 64)
            || !lower_hex(&self.source_commit, 40)
            || !lower_hex(&self.source_sha256, 64)
            || !lower_hex(&self.toolchain_sha256, 64)
            || !lower_hex(&self.artifact_sha256, 64) {
            return Err("invalid signed build source, challenge or artifact digest".into());
        }
        if !matches!(self.target.as_str(), "java" | "llvm" | "wasm" | "net" | "forge" | "js")
            || !matches!(self.artifact_format.as_str(), "tar.gz" | "zip")
            || self.artifact_size == 0
            || self.issued_at_unix_ms == 0
            || self.expires_at_unix_ms <= self.issued_at_unix_ms
            || self.expires_at_unix_ms - self.issued_at_unix_ms > 300_000 {
            return Err("invalid target, artifact size or expiration".into());
        }
        Ok(())
    }

    pub fn preimage(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        let mut output = Vec::with_capacity(PREIMAGE_DOMAIN.len() + bytes.len());
        output.extend_from_slice(PREIMAGE_DOMAIN);
        output.extend_from_slice(&bytes);
        Ok(output)
    }
}
impl SignedBuildV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != SIGNED_BUILD_SCHEMA_V1 {
            return Err("unsupported signed-build schema".into());
        }
        self.claims.validate()?;
        if self.signatures.len() != 1
            || self.signatures[0].algorithm != SIGNING_ALGORITHM
            || self.signatures[0].key_id != self.claims.device_key_id
            || self.signatures[0].signature_bytes().is_err() {
            return Err("expected one matching Ed25519 device signature".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> BuildClaimsV1 {
        BuildClaimsV1 {
            audience: SIGNED_BUILD_AUDIENCE_V1.into(), org: "acme".into(),
            package: "widget".into(), version: "1.2.3".into(),
            challenge_id: "12345678-1234-1234-1234-123456789abc".into(),
            nonce_sha256: "a".repeat(64), device_key_id: "device-1".into(),
            source_commit: "b".repeat(40), source_sha256: "c".repeat(64),
            toolchain_sha256: "d".repeat(64), target: "llvm".into(),
            artifact_sha256: "e".repeat(64), artifact_size: 128,
            artifact_format: "tar.gz".into(), issued_at_unix_ms: 1_799_999_880_000,
            expires_at_unix_ms: 1_800_000_000_000,
        }
    }
    #[test] fn canonical_preimage_is_stable_and_domain_separated() {
        let x = sample();
        assert_eq!(x.preimage().unwrap(), x.preimage().unwrap());
        assert!(x.preimage().unwrap().starts_with(PREIMAGE_DOMAIN));
        let mut changed = x.clone();
        changed.artifact_sha256 = "f".repeat(64);
        assert_ne!(x.preimage().unwrap(), changed.preimage().unwrap());
    }
    #[test] fn refuse_wrong_audience_size_or_digest() {
        let mut x = sample();
        x.audience = "another-tenant".into();
        assert!(x.validate().is_err());
        x = sample();
        x.source_sha256 = "A".repeat(64);
        assert!(x.validate().is_err());
        x = sample();
        x.artifact_size = 0;
        assert!(x.validate().is_err());
    }
    #[test] fn reject_unknown_fields() {
        let mut value = serde_json::to_value(sample()).unwrap();
        value["trusted_release"] = serde_json::json!(true);
        assert!(serde_json::from_value::<BuildClaimsV1>(value).is_err());
    }
    #[test] fn reject_unsigned_claims() {
        assert!(SignedBuildV1 {schema: SIGNED_BUILD_SCHEMA_V1.into(), claims: sample(),
            signatures: vec![]}.validate().is_err());
    }
}
