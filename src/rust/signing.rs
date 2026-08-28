//! Publisher signing: the contract that lets a mirror serve *metadata*, not
//! just bytes.
//!
//! A frozen install needs no signatures. The lockfile pins a sha256, and any
//! transport that produces matching bytes is as good as the registry. The hard
//! case is the unpinned one — resolving `^1.2` to `1.2.4`, or installing a
//! package for the first time — because there the answer *is* the metadata,
//! and a mirror that could choose the answer could choose an old version with
//! a known hole and call it the newest.
//!
//! So a mirror that serves metadata serves it signed. The publisher holds an
//! Ed25519 key; the registry, the package's own manifest, and (after first
//! use) the consumer's lockfile all carry the public half, so no single one of
//! them has to be reachable for verification to happen. That is the whole
//! point: a trust anchor that lives only on the server you are trying to route
//! around is not a trust anchor.
//!
//! This module defines the documents and the exact bytes that get signed. It
//! performs no cryptography — `zed-cli` owns the Ed25519 implementation, the
//! same way `zed-api-server` owns the storage backend. Keeping the preimage
//! here, in the shared contract, is what lets a client verify a signature
//! itself rather than asking the registry whether the registry is honest.
//!
//! Encodings follow the ones already established by
//! [`crate::registry_protocol_v1`]: public keys are multibase base58btc with a
//! `z` prefix, signatures are unpadded base64url.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::artifact::ArtifactFormat;
use crate::mirror::MirrorDescriptorV1;

/// Publisher key-set schema marker.
pub const PUBLISHER_KEYS_SCHEMA_V1: &str = "zpkg.publisher-keys/v1";
/// Signed version-metadata document schema marker.
pub const SIGNED_VERSION_SCHEMA_V1: &str = "zpkg.signed-version/v1";
/// Signed package-index document schema marker.
pub const SIGNED_INDEX_SCHEMA_V1: &str = "zpkg.signed-index/v1";

/// The only signature algorithm v1 accepts.
pub const SIGNING_ALGORITHM: &str = "ed25519";
/// Raw Ed25519 public key length.
pub const ED25519_PUBLIC_KEY_BYTES: usize = 32;
/// Raw Ed25519 signature length.
pub const ED25519_SIGNATURE_BYTES: usize = 64;
/// Multibase prefix for base58btc.
pub const MULTIBASE_BASE58BTC: char = 'z';

/// Bounded so a hostile key set cannot make verification quadratic.
pub const MAX_KEYS_PER_ORG: usize = 8;
/// Bounded for the same reason.
pub const MAX_SIGNATURES: usize = 4;

/// Lifecycle of a publisher key.
///
/// Rotation is additive: a new key is enrolled `active` while the old one
/// moves to `retired`. Retired keys still verify — revoking them would break
/// every already-signed historical version, which is the opposite of what
/// rotation is for. Compromise is expressed by `revoked`, which invalidates
/// past signatures too and is deliberately a separate, louder state.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "kebab-case")]
pub enum PublisherKeyStateV1 {
    /// Signs new publications and verifies everything.
    Active,
    /// No longer signs; still verifies historical signatures.
    Retired,
    /// Compromised. Verification must fail, including for versions published
    /// before the revocation.
    Revoked,
}

impl PublisherKeyStateV1 {
    /// Whether a signature made by a key in this state may be accepted.
    pub fn verifies(&self) -> bool {
        match self {
            PublisherKeyStateV1::Active | PublisherKeyStateV1::Retired => true,
            PublisherKeyStateV1::Revoked => false,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            PublisherKeyStateV1::Active => "active",
            PublisherKeyStateV1::Retired => "retired",
            PublisherKeyStateV1::Revoked => "revoked",
        }
    }
}

/// One publisher signing key.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub struct PublisherKeyV1 {
    /// Short stable label, unique within the org. Names the key in
    /// diagnostics and lets a signature say which key made it without
    /// carrying the key itself.
    #[schemars(length(min = 1, max = 64), regex(pattern = r"^[a-z0-9][a-z0-9._-]*$"))]
    pub key_id: String,
    /// Always `ed25519` in v1.
    #[schemars(regex(pattern = r"^ed25519$"))]
    pub algorithm: String,
    /// Multibase base58btc, `z`-prefixed.
    #[schemars(
        length(min = 8, max = 128),
        regex(pattern = r"^z[1-9A-HJ-NP-Za-km-z]+$")
    )]
    pub public_key_multibase: String,
    pub state: PublisherKeyStateV1,
    /// RFC 3339 timestamp the key was enrolled.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enrolled_at: Option<String>,
    /// Why the key was revoked. Present only for `revoked` keys, so an
    /// operator reading a failed verification learns the reason from the same
    /// document that caused the failure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_reason: Option<String>,
}

impl PublisherKeyV1 {
    pub fn new(key_id: &str, public_key: &[u8; ED25519_PUBLIC_KEY_BYTES]) -> Self {
        Self {
            key_id: key_id.to_owned(),
            algorithm: SIGNING_ALGORITHM.to_owned(),
            public_key_multibase: encode_multibase_base58btc(public_key),
            state: PublisherKeyStateV1::Active,
            enrolled_at: None,
            revoked_reason: None,
        }
    }

    /// The raw 32-byte key.
    pub fn public_key(&self) -> Result<[u8; ED25519_PUBLIC_KEY_BYTES], SigningError> {
        let bytes = decode_multibase_base58btc(&self.public_key_multibase)?;
        bytes
            .as_slice()
            .try_into()
            .map_err(|_| SigningError::KeyLength(bytes.len()))
    }

    pub fn validate(&self) -> Result<(), SigningError> {
        if self.algorithm != SIGNING_ALGORITHM {
            return Err(SigningError::UnsupportedAlgorithm(self.algorithm.clone()));
        }
        validate_key_id(&self.key_id)?;
        self.public_key()?;
        match (self.state, self.revoked_reason.is_some()) {
            (PublisherKeyStateV1::Revoked, false) => Err(SigningError::MissingField {
                field: "revoked_reason",
            }),
            (state, true) if state != PublisherKeyStateV1::Revoked => {
                Err(SigningError::UnexpectedField {
                    field: "revoked_reason",
                })
            }
            _ => Ok(()),
        }
    }
}

/// The set of keys an org publishes with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PublisherKeySetV1 {
    pub schema: String,
    #[schemars(length(min = 1), regex(pattern = r"^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$"))]
    pub org: String,
    pub keys: Vec<PublisherKeyV1>,
}

impl PublisherKeySetV1 {
    pub const SCHEMA_V1: &'static str = PUBLISHER_KEYS_SCHEMA_V1;

    pub fn validate(&self) -> Result<(), SigningError> {
        if self.schema != Self::SCHEMA_V1 {
            return Err(SigningError::InvalidSchema {
                expected: Self::SCHEMA_V1,
                actual: self.schema.clone(),
            });
        }
        if self.keys.len() > MAX_KEYS_PER_ORG {
            return Err(SigningError::TooManyKeys(self.keys.len()));
        }
        let mut seen = std::collections::BTreeSet::new();
        for key in &self.keys {
            key.validate()?;
            if !seen.insert(key.key_id.clone()) {
                return Err(SigningError::DuplicateKeyId(key.key_id.clone()));
            }
        }
        Ok(())
    }

    pub fn find(&self, key_id: &str) -> Option<&PublisherKeyV1> {
        self.keys.iter().find(|key| key.key_id == key_id)
    }
}

/// A signature over a document, carried beside it rather than inside it.
///
/// Detached because the signed bytes must be reproducible: embedding the
/// signature in the document it signs forces a "remove this field first" rule,
/// and every such rule is a place where a signer and a verifier can disagree.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
pub struct DetachedSignatureV1 {
    #[schemars(length(min = 1, max = 64), regex(pattern = r"^[a-z0-9][a-z0-9._-]*$"))]
    pub key_id: String,
    #[schemars(regex(pattern = r"^ed25519$"))]
    pub algorithm: String,
    /// Unpadded base64url of the raw 64-byte signature.
    #[schemars(length(min = 86, max = 88), regex(pattern = r"^[A-Za-z0-9_-]+$"))]
    pub signature: String,
}

impl DetachedSignatureV1 {
    pub fn new(key_id: &str, signature: &[u8; ED25519_SIGNATURE_BYTES]) -> Self {
        Self {
            key_id: key_id.to_owned(),
            algorithm: SIGNING_ALGORITHM.to_owned(),
            signature: encode_base64url(signature),
        }
    }

    pub fn signature_bytes(&self) -> Result<[u8; ED25519_SIGNATURE_BYTES], SigningError> {
        let bytes = decode_base64url(&self.signature)?;
        bytes
            .as_slice()
            .try_into()
            .map_err(|_| SigningError::SignatureLength(bytes.len()))
    }

    pub fn validate(&self) -> Result<(), SigningError> {
        if self.algorithm != SIGNING_ALGORITHM {
            return Err(SigningError::UnsupportedAlgorithm(self.algorithm.clone()));
        }
        validate_key_id(&self.key_id)?;
        self.signature_bytes().map(|_| ())
    }
}

/// What a publisher asserts about one published version.
///
/// This is a superset of what a lockfile pins, plus the mirror set — so a
/// consumer that learned about this version from a mirror learns where else to
/// look from the same signed document, and an attacker who controls one mirror
/// cannot quietly delete the others from the list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct VersionAttestationV1 {
    #[schemars(length(min = 1), regex(pattern = r"^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$"))]
    pub org: String,
    #[schemars(length(min = 1), regex(pattern = r"^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$"))]
    pub name: String,
    #[schemars(length(min = 1))]
    pub version: String,
    #[schemars(length(equal = 64), regex(pattern = r"^[0-9a-f]{64}$"))]
    pub sha256: String,
    #[schemars(range(min = 1))]
    pub size: u64,
    pub format: ArtifactFormat,
    #[schemars(length(min = 1))]
    pub vcs_tag: String,
    #[schemars(length(min = 7, max = 128))]
    pub vcs_commit: String,
    /// RFC 3339 timestamp.
    pub published_at: String,
    /// Where these bytes can be fetched, in try order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mirrors: Vec<MirrorDescriptorV1>,
}

/// A version-metadata document as a mirror serves it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SignedVersionV1 {
    pub schema: String,
    pub payload: VersionAttestationV1,
    pub signatures: Vec<DetachedSignatureV1>,
}

impl SignedVersionV1 {
    pub const SCHEMA_V1: &'static str = SIGNED_VERSION_SCHEMA_V1;

    pub fn validate(&self) -> Result<(), SigningError> {
        if self.schema != Self::SCHEMA_V1 {
            return Err(SigningError::InvalidSchema {
                expected: Self::SCHEMA_V1,
                actual: self.schema.clone(),
            });
        }
        validate_signature_set(&self.signatures)?;
        crate::mirror::normalize_mirrors(&self.payload.mirrors)
            .map_err(|error| SigningError::Mirror(error.to_string()))?;
        Ok(())
    }

    /// The exact bytes covered by [`Self::signatures`].
    pub fn preimage(&self) -> Result<Vec<u8>, SigningError> {
        version_attestation_preimage(&self.payload)
    }
}

/// One entry in a signed package index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IndexEntryV1 {
    #[schemars(length(min = 1))]
    pub version: String,
    #[schemars(length(equal = 64), regex(pattern = r"^[0-9a-f]{64}$"))]
    pub sha256: String,
    #[schemars(range(min = 1))]
    pub size: u64,
    pub format: ArtifactFormat,
    #[schemars(length(min = 1))]
    pub vcs_tag: String,
    #[schemars(length(min = 7, max = 128))]
    pub vcs_commit: String,
    pub published_at: String,
    #[serde(default)]
    pub yanked: bool,
}

/// What a publisher asserts about a package as a whole.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct IndexAttestationV1 {
    #[schemars(length(min = 1), regex(pattern = r"^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$"))]
    pub org: String,
    #[schemars(length(min = 1), regex(pattern = r"^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$"))]
    pub name: String,
    /// RFC 3339 timestamp this index was generated.
    pub generated_at: String,
    /// Monotonic counter, incremented on every publish.
    ///
    /// Without it, a mirror could serve a genuinely-signed but stale index
    /// forever and hide a security release. A client that has seen sequence
    /// `n` must refuse anything below it, which converts an undetectable
    /// rollback into a loud, attributable failure.
    #[schemars(range(min = 1))]
    pub sequence: u64,
    /// Every published version, newest first.
    pub versions: Vec<IndexEntryV1>,
    /// Where this package's artifacts can be fetched, in try order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mirrors: Vec<MirrorDescriptorV1>,
}

/// A package index as a mirror serves it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SignedIndexV1 {
    pub schema: String,
    pub payload: IndexAttestationV1,
    pub signatures: Vec<DetachedSignatureV1>,
}

impl SignedIndexV1 {
    pub const SCHEMA_V1: &'static str = SIGNED_INDEX_SCHEMA_V1;

    pub fn validate(&self) -> Result<(), SigningError> {
        if self.schema != Self::SCHEMA_V1 {
            return Err(SigningError::InvalidSchema {
                expected: Self::SCHEMA_V1,
                actual: self.schema.clone(),
            });
        }
        validate_signature_set(&self.signatures)?;
        crate::mirror::normalize_mirrors(&self.payload.mirrors)
            .map_err(|error| SigningError::Mirror(error.to_string()))?;
        let mut seen = std::collections::BTreeSet::new();
        for entry in &self.payload.versions {
            if !seen.insert(entry.version.clone()) {
                return Err(SigningError::DuplicateVersion(entry.version.clone()));
            }
        }
        Ok(())
    }

    pub fn preimage(&self) -> Result<Vec<u8>, SigningError> {
        index_attestation_preimage(&self.payload)
    }
}

/// The exact bytes a [`VersionAttestationV1`] is signed over.
///
/// Domain-separated and length-prefixed for the same reason
/// [`crate::registry::audit_chain_preimage`] is: a plain separator is
/// forgeable. A version string containing the separator could otherwise shift
/// field boundaries and reproduce another version's preimage — which is the
/// one attack a signature is supposed to make impossible.
///
/// The domain tag means a signature over a version can never be replayed as a
/// signature over an index, even though both are made by the same key.
pub fn version_attestation_preimage(
    attestation: &VersionAttestationV1,
) -> Result<Vec<u8>, SigningError> {
    let mut out = Vec::new();
    write_domain(&mut out, "zpkg.version-attestation/v1");
    write_field(&mut out, attestation.org.as_bytes());
    write_field(&mut out, attestation.name.as_bytes());
    write_field(&mut out, attestation.version.as_bytes());
    write_field(&mut out, attestation.sha256.as_bytes());
    write_field(&mut out, attestation.size.to_string().as_bytes());
    write_field(&mut out, attestation.format.extension().as_bytes());
    write_field(&mut out, attestation.vcs_tag.as_bytes());
    write_field(&mut out, attestation.vcs_commit.as_bytes());
    write_field(&mut out, attestation.published_at.as_bytes());
    write_mirrors(&mut out, &attestation.mirrors)?;
    Ok(out)
}

/// The exact bytes an [`IndexAttestationV1`] is signed over.
pub fn index_attestation_preimage(
    attestation: &IndexAttestationV1,
) -> Result<Vec<u8>, SigningError> {
    let mut out = Vec::new();
    write_domain(&mut out, "zpkg.index-attestation/v1");
    write_field(&mut out, attestation.org.as_bytes());
    write_field(&mut out, attestation.name.as_bytes());
    write_field(&mut out, attestation.generated_at.as_bytes());
    write_field(&mut out, attestation.sequence.to_string().as_bytes());
    write_field(&mut out, attestation.versions.len().to_string().as_bytes());
    for entry in &attestation.versions {
        write_field(&mut out, entry.version.as_bytes());
        write_field(&mut out, entry.sha256.as_bytes());
        write_field(&mut out, entry.size.to_string().as_bytes());
        write_field(&mut out, entry.format.extension().as_bytes());
        write_field(&mut out, entry.vcs_tag.as_bytes());
        write_field(&mut out, entry.vcs_commit.as_bytes());
        write_field(&mut out, entry.published_at.as_bytes());
        write_field(&mut out, if entry.yanked { b"1" } else { b"0" });
    }
    write_mirrors(&mut out, &attestation.mirrors)?;
    Ok(out)
}

/// Mirrors enter the preimage as canonical JSON of the normalized set.
///
/// Normalizing first means a publisher and a verifier that disagree about
/// declaration order still compute the same bytes, so reordering a manifest
/// does not silently invalidate every signature the org has made.
fn write_mirrors(out: &mut Vec<u8>, mirrors: &[MirrorDescriptorV1]) -> Result<(), SigningError> {
    let normalized = crate::mirror::normalize_mirrors(mirrors)
        .map_err(|error| SigningError::Mirror(error.to_string()))?;
    let value = serde_json::to_value(&normalized)?;
    let mut bytes = Vec::new();
    write_canonical_json(&value, &mut bytes)?;
    write_field(out, &bytes);
    Ok(())
}

fn write_domain(out: &mut Vec<u8>, domain: &str) {
    write_field(out, domain.as_bytes());
}

fn write_field(out: &mut Vec<u8>, value: &[u8]) {
    out.extend_from_slice(value.len().to_string().as_bytes());
    out.push(b':');
    out.extend_from_slice(value);
    out.push(b'\n');
}

fn validate_signature_set(signatures: &[DetachedSignatureV1]) -> Result<(), SigningError> {
    if signatures.is_empty() {
        return Err(SigningError::MissingField {
            field: "signatures",
        });
    }
    if signatures.len() > MAX_SIGNATURES {
        return Err(SigningError::TooManySignatures(signatures.len()));
    }
    let mut seen = std::collections::BTreeSet::new();
    for signature in signatures {
        signature.validate()?;
        if !seen.insert(signature.key_id.clone()) {
            return Err(SigningError::DuplicateKeyId(signature.key_id.clone()));
        }
    }
    Ok(())
}

fn validate_key_id(key_id: &str) -> Result<(), SigningError> {
    let bytes = key_id.as_bytes();
    let ok = !bytes.is_empty()
        && bytes.len() <= 64
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes.iter().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(*b, b'.' | b'_' | b'-')
        });
    if ok {
        Ok(())
    } else {
        Err(SigningError::InvalidKeyId(key_id.to_owned()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SigningError {
    #[error("unsupported signing algorithm `{0}` (v1 accepts only `ed25519`)")]
    UnsupportedAlgorithm(String),
    #[error("invalid key id `{0}`")]
    InvalidKeyId(String),
    #[error("duplicate key id `{0}`")]
    DuplicateKeyId(String),
    #[error("duplicate version `{0}` in index")]
    DuplicateVersion(String),
    #[error("expected schema `{expected}`, got `{actual}`")]
    InvalidSchema {
        expected: &'static str,
        actual: String,
    },
    #[error("field `{field}` is required")]
    MissingField { field: &'static str },
    #[error("field `{field}` is not allowed in this state")]
    UnexpectedField { field: &'static str },
    #[error("public key must be {ED25519_PUBLIC_KEY_BYTES} bytes, got {0}")]
    KeyLength(usize),
    #[error("signature must be {ED25519_SIGNATURE_BYTES} bytes, got {0}")]
    SignatureLength(usize),
    #[error("at most {MAX_KEYS_PER_ORG} keys per org, got {0}")]
    TooManyKeys(usize),
    #[error("at most {MAX_SIGNATURES} signatures per document, got {0}")]
    TooManySignatures(usize),
    #[error("value is not multibase base58btc: {0}")]
    Multibase(String),
    #[error("value is not unpadded base64url: {0}")]
    Base64(String),
    #[error("invalid mirror set: {0}")]
    Mirror(String),
    #[error("canonical JSON forbids non-integer number: {0}")]
    UnsupportedJsonNumber(String),
    #[error("JSON serialization failed: {0}")]
    Serialization(String),
}

impl From<serde_json::Error> for SigningError {
    fn from(error: serde_json::Error) -> Self {
        SigningError::Serialization(error.to_string())
    }
}

const BASE58BTC: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// Encode raw bytes as multibase base58btc (`z`-prefixed).
pub fn encode_multibase_base58btc(input: &[u8]) -> String {
    let mut digits: Vec<u8> = Vec::with_capacity(input.len() * 2);
    for &byte in input {
        let mut carry = u32::from(byte);
        for digit in digits.iter_mut() {
            carry += u32::from(*digit) << 8;
            *digit = (carry % 58) as u8;
            carry /= 58;
        }
        while carry > 0 {
            digits.push((carry % 58) as u8);
            carry /= 58;
        }
    }
    let mut out = String::with_capacity(digits.len() + 1);
    out.push(MULTIBASE_BASE58BTC);
    // Leading zero bytes carry no magnitude, so they must be encoded
    // positionally or `00ff` and `ff` would share an encoding.
    for _ in input.iter().take_while(|byte| **byte == 0) {
        out.push(BASE58BTC[0] as char);
    }
    for digit in digits.iter().rev() {
        out.push(BASE58BTC[*digit as usize] as char);
    }
    out
}

/// Decode a multibase base58btc string.
pub fn decode_multibase_base58btc(input: &str) -> Result<Vec<u8>, SigningError> {
    let body = input
        .strip_prefix(MULTIBASE_BASE58BTC)
        .ok_or_else(|| SigningError::Multibase(input.to_owned()))?;
    if body.is_empty() {
        return Err(SigningError::Multibase(input.to_owned()));
    }
    let mut bytes: Vec<u8> = Vec::with_capacity(body.len());
    for character in body.chars() {
        let value = BASE58BTC
            .iter()
            .position(|candidate| *candidate as char == character)
            .ok_or_else(|| SigningError::Multibase(input.to_owned()))?;
        let mut carry = value as u32;
        for byte in bytes.iter_mut() {
            carry += u32::from(*byte) * 58;
            *byte = (carry & 0xff) as u8;
            carry >>= 8;
        }
        while carry > 0 {
            bytes.push((carry & 0xff) as u8);
            carry >>= 8;
        }
    }
    let leading_zeros = body
        .chars()
        .take_while(|character| *character == BASE58BTC[0] as char)
        .count();
    let mut out = vec![0_u8; leading_zeros];
    out.extend(bytes.iter().rev());
    Ok(out)
}

const BASE64URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Encode raw bytes as unpadded base64url.
pub fn encode_base64url(input: &[u8]) -> String {
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = chunk.get(1).copied().map_or(0, u32::from);
        let b2 = chunk.get(2).copied().map_or(0, u32::from);
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(BASE64URL[((triple >> 18) & 0x3f) as usize] as char);
        out.push(BASE64URL[((triple >> 12) & 0x3f) as usize] as char);
        if chunk.len() > 1 {
            out.push(BASE64URL[((triple >> 6) & 0x3f) as usize] as char);
        }
        if chunk.len() > 2 {
            out.push(BASE64URL[(triple & 0x3f) as usize] as char);
        }
    }
    out
}

/// Decode unpadded base64url. Padding and whitespace are rejected rather than
/// tolerated: a signature with two spellings is a signature that can be
/// replayed past a duplicate check.
pub fn decode_base64url(input: &str) -> Result<Vec<u8>, SigningError> {
    let invalid = || SigningError::Base64(input.to_owned());
    if input.is_empty() || input.len() % 4 == 1 {
        return Err(invalid());
    }
    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    let mut buffer = 0_u32;
    let mut bits = 0_u32;
    for character in input.chars() {
        let value = BASE64URL
            .iter()
            .position(|candidate| *candidate as char == character)
            .ok_or_else(invalid)? as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buffer >> bits) & 0xff) as u8);
        }
    }
    // Leftover bits must be zero, or two distinct strings decode alike.
    if bits > 0 && (buffer & ((1 << bits) - 1)) != 0 {
        return Err(invalid());
    }
    Ok(out)
}

/// Canonical JSON: sorted object keys, integer numbers only, no insignificant
/// whitespace. Same rules as [`crate::registry_protocol_v1`], restated here so
/// the two never drift apart silently.
fn write_canonical_json(
    value: &serde_json::Value,
    bytes: &mut Vec<u8>,
) -> Result<(), SigningError> {
    use serde_json::Value;
    match value {
        Value::Null => bytes.extend_from_slice(b"null"),
        Value::Bool(value) => bytes.extend_from_slice(if *value { b"true" } else { b"false" }),
        Value::Number(number) => {
            if !number.is_i64() && !number.is_u64() {
                return Err(SigningError::UnsupportedJsonNumber(number.to_string()));
            }
            bytes.extend_from_slice(number.to_string().as_bytes());
        }
        Value::String(value) => bytes.extend_from_slice(serde_json::to_string(value)?.as_bytes()),
        Value::Array(values) => {
            bytes.push(b'[');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    bytes.push(b',');
                }
                write_canonical_json(value, bytes)?;
            }
            bytes.push(b']');
        }
        Value::Object(values) => {
            bytes.push(b'{');
            let mut keys: Vec<_> = values.keys().collect();
            keys.sort_unstable();
            for (index, key) in keys.into_iter().enumerate() {
                if index != 0 {
                    bytes.push(b',');
                }
                bytes.extend_from_slice(serde_json::to_string(key)?.as_bytes());
                bytes.push(b':');
                write_canonical_json(&values[key], bytes)?;
            }
            bytes.push(b'}');
        }
    }
    Ok(())
}
