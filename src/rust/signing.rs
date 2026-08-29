//! Publisher signatures for unpinned metadata.
//!
//! A locked sha256 needs no signature. Signatures exist when resolving a
//! range or adding a package for the first time, so a mirror cannot choose
//! the metadata. Encoding is multibase base58btc (`z` prefix) over raw
//! Ed25519 keys and signatures.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::artifact::ArtifactFormat;
use crate::mirror::MirrorDescriptorV1;

pub const SIGNING_ALGORITHM: &str = "ed25519";
pub const ED25519_PUBLIC_KEY_BYTES: usize = 32;
pub const ED25519_SIGNATURE_BYTES: usize = 64;
pub const PUBLISHER_KEYS_SCHEMA_V1: &str = "zpkg.publisher-keys/v1";
pub const SIGNED_VERSION_SCHEMA_V1: &str = "zpkg.signed-version/v1";
pub const SIGNED_INDEX_SCHEMA_V1: &str = "zpkg.signed-index/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum PublisherKeyStateV1 {
    Active,
    Retired,
    Revoked,
}

impl PublisherKeyStateV1 {
    pub fn verifies(self) -> bool {
        matches!(self, Self::Active | Self::Retired)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PublisherKeyV1 {
    pub key_id: String,
    pub algorithm: String,
    pub public_key_multibase: String,
    pub state: PublisherKeyStateV1,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enrolled_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revoked_reason: Option<String>,
}

impl PublisherKeyV1 {
    pub fn public_key(&self) -> Result<[u8; ED25519_PUBLIC_KEY_BYTES], String> {
        let bytes = decode_multibase_base58btc(&self.public_key_multibase)?;
        bytes
            .try_into()
            .map_err(|bytes: Vec<u8>| {
                format!(
                    "public key is {} bytes, expected {ED25519_PUBLIC_KEY_BYTES}",
                    bytes.len()
                )
            })
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.key_id.is_empty() {
            return Err("publisher key_id is empty".into());
        }
        if self.algorithm != SIGNING_ALGORITHM {
            return Err(format!("unsupported signing algorithm `{}`", self.algorithm));
        }
        let bytes = decode_multibase_base58btc(&self.public_key_multibase)?;
        if bytes.len() != ED25519_PUBLIC_KEY_BYTES {
            return Err(format!(
                "public key is {} bytes, expected {ED25519_PUBLIC_KEY_BYTES}",
                bytes.len()
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PublisherKeySetV1 {
    pub schema: String,
    pub org: String,
    #[serde(default)]
    pub keys: Vec<PublisherKeyV1>,
}

impl PublisherKeySetV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != PUBLISHER_KEYS_SCHEMA_V1 {
            return Err(format!("unsupported publisher-keys schema `{}`", self.schema));
        }
        if self.org.is_empty() {
            return Err("publisher key set org is empty".into());
        }
        for key in &self.keys {
            key.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DetachedSignatureV1 {
    pub key_id: String,
    pub algorithm: String,
    pub signature_multibase: String,
}

impl DetachedSignatureV1 {
    pub fn new(key_id: &str, signature: &[u8]) -> Self {
        Self {
            key_id: key_id.to_owned(),
            algorithm: SIGNING_ALGORITHM.to_owned(),
            signature_multibase: encode_multibase_base58btc(signature),
        }
    }

    pub fn signature_bytes(&self) -> Result<[u8; ED25519_SIGNATURE_BYTES], String> {
        let bytes = decode_multibase_base58btc(&self.signature_multibase)?;
        bytes.try_into().map_err(|bytes: Vec<u8>| {
            format!(
                "signature is {} bytes, expected {ED25519_SIGNATURE_BYTES}",
                bytes.len()
            )
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct VersionAttestationV1 {
    pub org: String,
    pub name: String,
    pub version: String,
    pub sha256: String,
    pub size: u64,
    pub format: ArtifactFormat,
    pub vcs_tag: String,
    pub vcs_commit: String,
    pub published_at: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mirrors: Vec<MirrorDescriptorV1>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SignedVersionV1 {
    pub schema: String,
    pub payload: VersionAttestationV1,
    #[serde(default)]
    pub signatures: Vec<DetachedSignatureV1>,
}

impl SignedVersionV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != SIGNED_VERSION_SCHEMA_V1 {
            return Err(format!("unsupported signed-version schema `{}`", self.schema));
        }
        if self.payload.org.is_empty() || self.payload.name.is_empty() {
            return Err("signed version is missing org/name".into());
        }
        if self.signatures.is_empty() {
            return Err("signed version has no signatures".into());
        }
        Ok(())
    }

    pub fn preimage(&self) -> Result<Vec<u8>, String> {
        version_attestation_preimage(&self.payload)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct IndexVersionV1 {
    pub version: String,
    #[serde(default)]
    pub yanked: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct IndexAttestationV1 {
    pub org: String,
    pub name: String,
    pub sequence: u64,
    #[serde(default)]
    pub versions: Vec<IndexVersionV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mirrors: Vec<MirrorDescriptorV1>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SignedIndexV1 {
    pub schema: String,
    pub payload: IndexAttestationV1,
    #[serde(default)]
    pub signatures: Vec<DetachedSignatureV1>,
}

impl SignedIndexV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != SIGNED_INDEX_SCHEMA_V1 {
            return Err(format!("unsupported signed-index schema `{}`", self.schema));
        }
        if self.payload.org.is_empty() || self.payload.name.is_empty() {
            return Err("signed index is missing org/name".into());
        }
        Ok(())
    }

    pub fn preimage(&self) -> Result<Vec<u8>, String> {
        serde_json::to_vec(&self.payload).map_err(|error| error.to_string())
    }
}

/// Canonical preimage: stable JSON object, no extra whitespace, sorted keys.
pub fn version_attestation_preimage(attestation: &VersionAttestationV1) -> Result<Vec<u8>, String> {
    serde_json::to_vec(attestation).map_err(|error| error.to_string())
}

pub fn encode_multibase_base58btc(bytes: &[u8]) -> String {
    format!("z{}", bs58::encode(bytes).into_string())
}

pub fn decode_multibase_base58btc(value: &str) -> Result<Vec<u8>, String> {
    let rest = value
        .strip_prefix('z')
        .ok_or_else(|| "multibase value must start with z".to_string())?;
    bs58::decode(rest)
        .into_vec()
        .map_err(|error| error.to_string())
}

/// Optional `[signing]` table on `.zpkg.toml`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SigningSection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestSigningKey {
    pub key_id: String,
}

impl SigningSection {
    pub fn is_empty(&self) -> bool {
        self.key_id.as_deref().unwrap_or("").is_empty()
    }

    pub fn signing_key(&self) -> Result<Option<ManifestSigningKey>, String> {
        match &self.key_id {
            Some(key_id) if !key_id.is_empty() => Ok(Some(ManifestSigningKey {
                key_id: key_id.clone(),
            })),
            _ => Ok(None),
        }
    }
}
