#!/usr/bin/env python3
"""One-use hardening patch for DEN-2854 canonical trust metadata.

This transformation runs after the immutable-snapshot and archive-endpoint
patchers. It makes discovery root-signed/versioned, makes signing and digest
bytes cross-language deterministic, tightens checkpoint/timestamp invariants,
and turns the golden fixture into a digest-consistent chain.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(
            f"{path}: expected one replacement, found {count}: {old[:100]!r}"
        )
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


def replace_section(path: str, start: str, end: str, replacement: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    start_index = text.find(start)
    if start_index < 0:
        raise SystemExit(f"{path}: start marker not found: {start!r}")
    end_index = text.find(end, start_index + len(start))
    if end_index < 0:
        raise SystemExit(f"{path}: end marker not found: {end!r}")
    file.write_text(
        text[:start_index] + replacement + text[end_index:], encoding="utf-8"
    )


def canonical_json_bytes(value: object) -> bytes:
    # The Rust implementation uses the same RFC-8785-compatible subset:
    # lexicographic ASCII object keys, UTF-8 strings, integers only, no floats.
    return json.dumps(
        value,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")


def main() -> None:
    source = "src/registry_protocol_v1.rs"

    replace_once(
        source,
        "use serde::{Deserialize, Serialize};\nuse thiserror::Error;\n",
        "use serde::{Deserialize, Serialize};\nuse serde_json::Value;\nuse thiserror::Error;\n",
    )

    discovery_section = r'''/// Signature made by a locally enrolled recovery/root key.
///
/// Root public keys are deliberately not self-asserted by discovery. Their
/// fingerprints and threshold policy are enrolled out of band. These
/// signatures delegate the online checkpoint-signing keys carried by the
/// discovery payload.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct RegistryRootSignatureV1 {
    pub key_id: String,
    pub algorithm: String,
    pub signature: String,
}

impl RegistryRootSignatureV1 {
    fn validate(&self, field: &str) -> Result<(), RegistryProtocolV1Error> {
        validate_lower_token(&format!("{field}.key_id"), &self.key_id)?;
        if self.algorithm != "ed25519" {
            return Err(RegistryProtocolV1Error::UnsupportedValue {
                field: format!("{field}.algorithm"),
                value: self.algorithm.clone(),
            });
        }
        validate_signature(&format!("{field}.signature"), &self.signature)
    }
}

/// Root-signed, versioned registry discovery document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistryDiscoveryV1 {
    pub schema: String,
    pub version: u64,
    pub generated_at: String,
    pub expires_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_discovery_sha256: Option<String>,
    pub registry_id: String,
    pub canonical_url: String,
    pub protocol_versions: Vec<String>,
    pub endpoints: RegistryEndpointsV1,
    pub capabilities: RegistryCapabilitiesV1,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub auth: Vec<RegistryAuthDescriptorV1>,
    pub signing_keys: Vec<RegistrySigningKeyV1>,
    pub accepted_digest_algorithms: Vec<String>,
    pub accepted_archive_formats: Vec<RegistryArchiveFormatV1>,
    pub limits: RegistryLimitsV1,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub root_signatures: Vec<RegistryRootSignatureV1>,
}

impl RegistryDiscoveryV1 {
    pub const SCHEMA_V1: &'static str = REGISTRY_DISCOVERY_SCHEMA_V1;

    fn validate_payload_fields(&self) -> Result<(), RegistryProtocolV1Error> {
        validate_schema("schema", &self.schema, Self::SCHEMA_V1)?;
        if self.version == 0 {
            return Err(RegistryProtocolV1Error::ZeroValue {
                field: "version".to_owned(),
            });
        }
        validate_utc_timestamp("generated_at", &self.generated_at)?;
        validate_utc_timestamp("expires_at", &self.expires_at)?;
        if self.generated_at >= self.expires_at {
            return Err(RegistryProtocolV1Error::InvalidRelationship {
                message: "generated_at must precede expires_at".to_owned(),
            });
        }
        match (self.version, self.previous_discovery_sha256.as_deref()) {
            (1, None) => {}
            (1, Some(_)) => {
                return Err(RegistryProtocolV1Error::UnexpectedField {
                    field: "previous_discovery_sha256".to_owned(),
                });
            }
            (_, Some(previous)) => {
                validate_sha256("previous_discovery_sha256", previous)?;
            }
            (_, None) => {
                return Err(RegistryProtocolV1Error::MissingField {
                    field: "previous_discovery_sha256".to_owned(),
                });
            }
        }

        validate_registry_id(&self.registry_id)?;
        validate_canonical_url(&self.canonical_url)?;
        if self.protocol_versions.is_empty()
            || !self
                .protocol_versions
                .iter()
                .any(|version| version == REGISTRY_PROTOCOL_V1)
        {
            return Err(RegistryProtocolV1Error::MissingProtocolVersion {
                version: REGISTRY_PROTOCOL_V1.to_owned(),
            });
        }
        ensure_unique("protocol_versions", &self.protocol_versions)?;
        self.endpoints.validate()?;

        for auth in &self.auth {
            auth.validate()?;
        }
        ensure_unique_by(
            "auth",
            self.auth
                .iter()
                .map(|descriptor| format!("{:?}", descriptor.mode)),
        )?;
        let has_anonymous = self
            .auth
            .iter()
            .any(|descriptor| descriptor.mode == RegistryAuthModeV1::AnonymousRead);
        let has_authenticated = self
            .auth
            .iter()
            .any(|descriptor| descriptor.mode != RegistryAuthModeV1::AnonymousRead);
        if self.capabilities.public_read != has_anonymous {
            return Err(RegistryProtocolV1Error::InvalidRelationship {
                message: "public_read must match the anonymous-read auth descriptor"
                    .to_owned(),
            });
        }
        if self.capabilities.publish != self.endpoints.publish.is_some() {
            return Err(RegistryProtocolV1Error::InvalidRelationship {
                message: "publish capability and endpoint must agree".to_owned(),
            });
        }
        if self.capabilities.yank != self.endpoints.yank.is_some() {
            return Err(RegistryProtocolV1Error::InvalidRelationship {
                message: "yank capability and endpoint must agree".to_owned(),
            });
        }
        if self.capabilities.static_export && !self.capabilities.public_read {
            return Err(RegistryProtocolV1Error::InvalidRelationship {
                message: "static_export requires public_read".to_owned(),
            });
        }
        if (self.capabilities.publish
            || self.capabilities.yank
            || self.capabilities.private_packages)
            && !has_authenticated
        {
            return Err(RegistryProtocolV1Error::InvalidRelationship {
                message: "write/private capabilities require an authenticated mode"
                    .to_owned(),
            });
        }

        if self.signing_keys.is_empty() {
            return Err(RegistryProtocolV1Error::MissingActiveSigningKey);
        }
        let mut active = 0_u64;
        let mut key_ids = BTreeSet::new();
        for (index, key) in self.signing_keys.iter().enumerate() {
            key.validate(&format!("signing_keys[{index}]"))?;
            if !key_ids.insert(key.key_id.clone()) {
                return Err(RegistryProtocolV1Error::DuplicateValue {
                    field: "signing_keys.key_id".to_owned(),
                    value: key.key_id.clone(),
                });
            }
            if key.state == RegistrySigningKeyStateV1::Active {
                active += 1;
            }
        }
        if active == 0 {
            return Err(RegistryProtocolV1Error::MissingActiveSigningKey);
        }

        if self.accepted_digest_algorithms.is_empty()
            || !self
                .accepted_digest_algorithms
                .iter()
                .any(|algorithm| algorithm == "sha256")
        {
            return Err(RegistryProtocolV1Error::MissingField {
                field: "accepted_digest_algorithms:sha256".to_owned(),
            });
        }
        for (index, algorithm) in self.accepted_digest_algorithms.iter().enumerate() {
            validate_lower_token(
                &format!("accepted_digest_algorithms[{index}]"),
                algorithm,
            )?;
        }
        ensure_unique(
            "accepted_digest_algorithms",
            &self.accepted_digest_algorithms,
        )?;

        if self.accepted_archive_formats.is_empty()
            || !self
                .accepted_archive_formats
                .contains(&RegistryArchiveFormatV1::TarZstd)
        {
            return Err(RegistryProtocolV1Error::MissingField {
                field: "accepted_archive_formats:tar-zstd".to_owned(),
            });
        }
        ensure_unique_by(
            "accepted_archive_formats",
            self.accepted_archive_formats
                .iter()
                .map(|format| format!("{format:?}")),
        )?;
        self.limits.validate()
    }

    pub fn validate(&self) -> Result<(), RegistryProtocolV1Error> {
        self.validate_payload_fields()?;
        if self.root_signatures.is_empty() {
            return Err(RegistryProtocolV1Error::MissingField {
                field: "root_signatures".to_owned(),
            });
        }
        let mut root_key_ids = BTreeSet::new();
        for (index, signature) in self.root_signatures.iter().enumerate() {
            signature.validate(&format!("root_signatures[{index}]"))?;
            if !root_key_ids.insert(signature.key_id.clone()) {
                return Err(RegistryProtocolV1Error::DuplicateValue {
                    field: "root_signatures.key_id".to_owned(),
                    value: signature.key_id.clone(),
                });
            }
        }
        Ok(())
    }

    fn normalize_in_place(&mut self) {
        self.protocol_versions.sort();
        self.auth.sort_by_key(|descriptor| descriptor.mode);
        self.signing_keys
            .sort_by(|left, right| left.key_id.cmp(&right.key_id));
        self.accepted_digest_algorithms.sort();
        self.accepted_archive_formats.sort();
        self.root_signatures.sort();
    }

    /// Canonical payload verified against locally enrolled recovery/root keys.
    /// Root signatures themselves are excluded from these bytes.
    pub fn signing_payload_bytes(&self) -> Result<Vec<u8>, RegistryProtocolV1Error> {
        self.validate_payload_fields()?;
        let mut payload = self.clone();
        payload.root_signatures.clear();
        payload.normalize_in_place();
        canonical_json_bytes(&serde_json::to_value(payload)?)
    }

    /// Canonical signed discovery bytes. Their SHA-256 forms the discovery
    /// predecessor link for the next version.
    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>, RegistryProtocolV1Error> {
        self.validate()?;
        let mut normalized = self.clone();
        normalized.normalize_in_place();
        canonical_json_bytes(&serde_json::to_value(normalized)?)
    }

    /// Validates the metadata relationship before cryptographic signature
    /// verification by the client implementation.
    pub fn authorize_current_checkpoint_metadata(
        &self,
        checkpoint: &RegistryCheckpointV1,
    ) -> Result<(), RegistryProtocolV1Error> {
        self.validate()?;
        checkpoint.validate()?;
        if self.registry_id != checkpoint.registry_id {
            return Err(RegistryProtocolV1Error::InvalidRelationship {
                message: "discovery and checkpoint registry_id must match".to_owned(),
            });
        }
        let authorized = self.signing_keys.iter().any(|key| {
            key.state == RegistrySigningKeyStateV1::Active
                && key.key_id == checkpoint.signing_key_id
        });
        if !authorized {
            return Err(RegistryProtocolV1Error::InvalidRelationship {
                message: "checkpoint signing key is not active in accepted discovery"
                    .to_owned(),
            });
        }
        Ok(())
    }
}

'''
    replace_section(
        source,
        "/// Signed/versioned registry discovery document.\n",
        "/// Package version lifecycle.",
        discovery_section,
    )

    replace_once(
        source,
        "#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]\n"
        "#[serde(rename_all = \"kebab-case\")]\n"
        "pub enum RegistryArchiveFormatV1 {\n",
        "#[derive(\n"
        "    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,\n"
        ")]\n"
        "#[serde(rename_all = \"kebab-case\")]\n"
        "pub enum RegistryArchiveFormatV1 {\n",
    )

    checkpoint_impl = r'''impl RegistryCheckpointV1 {
    pub const SCHEMA_V1: &'static str = REGISTRY_CHECKPOINT_SCHEMA_V1;

    fn validate_payload_fields(&self) -> Result<(), RegistryProtocolV1Error> {
        validate_schema("schema", &self.schema, Self::SCHEMA_V1)?;
        validate_registry_id(&self.registry_id)?;
        if self.sequence == 0 {
            return Err(RegistryProtocolV1Error::ZeroValue {
                field: "sequence".to_owned(),
            });
        }
        validate_utc_timestamp("generated_at", &self.generated_at)?;
        validate_utc_timestamp("expires_at", &self.expires_at)?;
        if self.generated_at >= self.expires_at {
            return Err(RegistryProtocolV1Error::InvalidRelationship {
                message: "generated_at must precede expires_at".to_owned(),
            });
        }
        validate_sha256("index_root_sha256", &self.index_root_sha256)?;
        match (self.sequence, self.previous_checkpoint_sha256.as_deref()) {
            (1, None) => {}
            (1, Some(_)) => {
                return Err(RegistryProtocolV1Error::UnexpectedField {
                    field: "previous_checkpoint_sha256".to_owned(),
                });
            }
            (_, Some(previous)) => {
                validate_sha256("previous_checkpoint_sha256", previous)?;
            }
            (_, None) => {
                return Err(RegistryProtocolV1Error::MissingField {
                    field: "previous_checkpoint_sha256".to_owned(),
                });
            }
        }
        validate_lower_token("signing_key_id", &self.signing_key_id)
    }

    pub fn validate(&self) -> Result<(), RegistryProtocolV1Error> {
        self.validate_payload_fields()?;
        validate_signature("signature", &self.signature)
    }

    /// Immutable snapshot token selected by this checkpoint.
    #[must_use]
    pub fn snapshot_id(&self) -> &str {
        &self.index_root_sha256
    }

    /// Canonical bytes covered by `signature`. The signature itself is
    /// excluded, and a publisher can obtain these bytes before a signature
    /// exists.
    pub fn signing_payload_bytes(&self) -> Result<Vec<u8>, RegistryProtocolV1Error> {
        self.validate_payload_fields()?;
        #[derive(Serialize)]
        struct Payload<'a> {
            schema: &'a str,
            registry_id: &'a str,
            sequence: u64,
            generated_at: &'a str,
            expires_at: &'a str,
            index_root_sha256: &'a str,
            #[serde(skip_serializing_if = "Option::is_none")]
            previous_checkpoint_sha256: &'a Option<String>,
            signing_key_id: &'a str,
        }
        canonical_json_bytes(&serde_json::to_value(Payload {
            schema: &self.schema,
            registry_id: &self.registry_id,
            sequence: self.sequence,
            generated_at: &self.generated_at,
            expires_at: &self.expires_at,
            index_root_sha256: &self.index_root_sha256,
            previous_checkpoint_sha256: &self.previous_checkpoint_sha256,
            signing_key_id: &self.signing_key_id,
        })?)
    }
}

'''
    replace_section(
        source,
        "impl RegistryCheckpointV1 {\n",
        "/// Entry kinds supported by a canonical package archive.",
        checkpoint_impl,
    )

    replace_once(
        source,
        "        serde_json::to_vec(&normalized).map_err(RegistryProtocolV1Error::Serialization)\n",
        "        canonical_json_bytes(&serde_json::to_value(&normalized)?)\n",
    )

    file = Path(source)
    text = file.read_text(encoding="utf-8")
    old_self = (
        "        serde_json::to_vec(self).map_err(RegistryProtocolV1Error::Serialization)\n"
    )
    if text.count(old_self) != 2:
        raise SystemExit(
            f"{source}: expected two snapshot/archive canonical replacements, "
            f"found {text.count(old_self)}"
        )
    file.write_text(
        text.replace(
            old_self,
            "        canonical_json_bytes(&serde_json::to_value(self)?)\n",
        ),
        encoding="utf-8",
    )

    replace_once(
        source,
        "    #[error(\"JSON serialization failed: {0}\")]\n"
        "    Serialization(#[from] serde_json::Error),\n",
        "    #[error(\"canonical registry JSON forbids non-integer number: {0}\")]\n"
        "    UnsupportedJsonNumber(String),\n"
        "    #[error(\"JSON serialization failed: {0}\")]\n"
        "    Serialization(#[from] serde_json::Error),\n",
    )

    endpoint_helpers = r'''fn validate_endpoint(field: &str, value: &str) -> Result<(), RegistryProtocolV1Error> {
    if !value.starts_with('/')
        || value.starts_with("//")
        || value.contains("..")
        || value.contains('\\')
        || value.contains('?')
        || value.contains('#')
        || value.contains('%')
        || value
            .bytes()
            .any(|byte| byte.is_ascii_whitespace() || byte.is_ascii_control())
    {
        return Err(RegistryProtocolV1Error::InvalidValue {
            field: field.to_owned(),
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn validate_endpoint_template(
    field: &str,
    value: &str,
    required_tokens: &[&str],
) -> Result<(), RegistryProtocolV1Error> {
    validate_endpoint(field, value)?;
    let mut remainder = value.to_owned();
    for token in required_tokens {
        match value.matches(token).count() {
            0 => {
                return Err(RegistryProtocolV1Error::MissingField {
                    field: format!("{field}:{token}"),
                });
            }
            1 => {
                remainder = remainder.replacen(token, "", 1);
            }
            _ => {
                return Err(RegistryProtocolV1Error::InvalidValue {
                    field: field.to_owned(),
                    value: value.to_owned(),
                });
            }
        }
    }
    if remainder.contains('{') || remainder.contains('}') {
        return Err(RegistryProtocolV1Error::InvalidValue {
            field: field.to_owned(),
            value: value.to_owned(),
        });
    }
    Ok(())
}

'''
    replace_section(
        source,
        "fn validate_endpoint(field: &str, value: &str) -> Result<(), RegistryProtocolV1Error> {\n",
        "fn validate_coordinate_component",
        endpoint_helpers,
    )

    timestamp_helpers = r'''fn validate_utc_timestamp(field: &str, value: &str) -> Result<(), RegistryProtocolV1Error> {
    let bytes = value.as_bytes();
    let valid_shape = bytes.len() == 20
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'T'
        && bytes[13] == b':'
        && bytes[16] == b':'
        && bytes[19] == b'Z'
        && [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18]
            .into_iter()
            .all(|index| bytes[index].is_ascii_digit());
    if !valid_shape {
        return Err(RegistryProtocolV1Error::InvalidValue {
            field: field.to_owned(),
            value: value.to_owned(),
        });
    }

    let year = parse_decimal(bytes, 0, 4);
    let month = parse_decimal(bytes, 5, 2);
    let day = parse_decimal(bytes, 8, 2);
    let hour = parse_decimal(bytes, 11, 2);
    let minute = parse_decimal(bytes, 14, 2);
    let second = parse_decimal(bytes, 17, 2);
    let valid_calendar = (1..=9999).contains(&year)
        && (1..=12).contains(&month)
        && day >= 1
        && day <= days_in_month(year, month)
        && hour <= 23
        && minute <= 59
        && second <= 59;
    if !valid_calendar {
        return Err(RegistryProtocolV1Error::InvalidValue {
            field: field.to_owned(),
            value: value.to_owned(),
        });
    }
    Ok(())
}

fn parse_decimal(bytes: &[u8], start: usize, length: usize) -> u32 {
    bytes[start..start + length]
        .iter()
        .fold(0_u32, |value, byte| value * 10 + u32::from(byte - b'0'))
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year.is_multiple_of(400) || (year.is_multiple_of(4) && !year.is_multiple_of(100)) => 29,
        2 => 28,
        _ => 0,
    }
}

'''
    replace_section(
        source,
        "fn validate_utc_timestamp(field: &str, value: &str) -> Result<(), RegistryProtocolV1Error> {\n",
        "fn validate_safe_relative_path",
        timestamp_helpers,
    )

    canonical_helpers = r'''fn canonical_json_bytes(value: &Value) -> Result<Vec<u8>, RegistryProtocolV1Error> {
    let mut bytes = Vec::new();
    write_canonical_json(value, &mut bytes)?;
    Ok(bytes)
}

fn write_canonical_json(
    value: &Value,
    bytes: &mut Vec<u8>,
) -> Result<(), RegistryProtocolV1Error> {
    match value {
        Value::Null => bytes.extend_from_slice(b"null"),
        Value::Bool(value) => bytes.extend_from_slice(if *value { b"true" } else { b"false" }),
        Value::Number(number) => {
            if !number.is_i64() && !number.is_u64() {
                return Err(RegistryProtocolV1Error::UnsupportedJsonNumber(
                    number.to_string(),
                ));
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

'''
    replace_once(
        source,
        "#[cfg(test)]\nmod tests {\n",
        canonical_helpers + "#[cfg(test)]\nmod tests {\n",
    )

    replace_once(
        source,
        "        RegistryDiscoveryV1 {\n"
        "            schema: REGISTRY_DISCOVERY_SCHEMA_V1.to_owned(),\n"
        "            registry_id: REGISTRY_ID.to_owned(),\n",
        "        RegistryDiscoveryV1 {\n"
        "            schema: REGISTRY_DISCOVERY_SCHEMA_V1.to_owned(),\n"
        "            version: 1,\n"
        "            generated_at: \"2026-08-07T00:00:00Z\".to_owned(),\n"
        "            expires_at: \"2026-08-08T00:00:00Z\".to_owned(),\n"
        "            previous_discovery_sha256: None,\n"
        "            registry_id: REGISTRY_ID.to_owned(),\n",
    )

    replace_once(
        source,
        "            protocol_versions: vec![REGISTRY_PROTOCOL_V1.to_owned()],\n"
        "            endpoints: RegistryEndpointsV1 {\n",
        "            protocol_versions: vec![REGISTRY_PROTOCOL_V1.to_owned()],\n"
        "            endpoints: RegistryEndpointsV1 {\n",
    )

    replace_once(
        source,
        "            auth: vec![RegistryAuthDescriptorV1 {\n"
        "                mode: RegistryAuthModeV1::OidcPkce,\n"
        "                issuer: Some(\"https://auth.example.test\".to_owned()),\n"
        "                audience: Some(\"registry.example.test\".to_owned()),\n"
        "            }],\n",
        "            auth: vec![\n"
        "                RegistryAuthDescriptorV1 {\n"
        "                    mode: RegistryAuthModeV1::AnonymousRead,\n"
        "                    issuer: None,\n"
        "                    audience: None,\n"
        "                },\n"
        "                RegistryAuthDescriptorV1 {\n"
        "                    mode: RegistryAuthModeV1::OidcPkce,\n"
        "                    issuer: Some(\"https://auth.example.test\".to_owned()),\n"
        "                    audience: Some(\"registry.example.test\".to_owned()),\n"
        "                },\n"
        "            ],\n",
    )

    replace_once(
        source,
        "            limits: RegistryLimitsV1 {\n"
        "                max_archive_bytes: 100 * 1024 * 1024,\n",
        "            accepted_digest_algorithms: vec![\"sha256\".to_owned()],\n"
        "            accepted_archive_formats: vec![RegistryArchiveFormatV1::TarZstd],\n"
        "            limits: RegistryLimitsV1 {\n"
        "                max_archive_bytes: 100 * 1024 * 1024,\n",
    )

    replace_once(
        source,
        "                max_compression_ratio: 100,\n"
        "            },\n"
        "        }\n"
        "    }\n\n"
        "    fn index_record() -> RegistryIndexRecordV1 {\n",
        "                max_compression_ratio: 100,\n"
        "            },\n"
        "            root_signatures: vec![RegistryRootSignatureV1 {\n"
        "                key_id: \"root-2026-01\".to_owned(),\n"
        "                algorithm: \"ed25519\".to_owned(),\n"
        "                signature: \"AbCdEfGhIjKlMnOpQrStUvWxYz_01234\".to_owned(),\n"
        "            }],\n"
        "        }\n"
        "    }\n\n"
        "    fn index_record() -> RegistryIndexRecordV1 {\n",
    )

    hardening_tests = r'''    #[test]
    fn discovery_is_root_signed_versioned_and_capability_consistent() {
        let discovery = discovery();
        let payload = discovery
            .signing_payload_bytes()
            .expect("unsigned discovery payload canonicalizes");
        let payload_text = String::from_utf8(payload).expect("canonical JSON is UTF-8");
        assert!(!payload_text.contains("root_signatures"));
        discovery.validate().expect("signed discovery validates");

        let mut unsigned = discovery.clone();
        unsigned.root_signatures.clear();
        assert!(unsigned.validate().is_err());
        unsigned
            .signing_payload_bytes()
            .expect("publisher can canonicalize before signing");

        let mut unchained = discovery.clone();
        unchained.version = 2;
        assert!(unchained.validate().is_err());

        let mut inconsistent = discovery.clone();
        inconsistent.capabilities.publish = false;
        assert!(inconsistent.validate().is_err());

        let mut no_anonymous = discovery.clone();
        no_anonymous
            .auth
            .retain(|descriptor| descriptor.mode != RegistryAuthModeV1::AnonymousRead);
        assert!(no_anonymous.validate().is_err());
    }

    #[test]
    fn checkpoint_genesis_timestamp_and_signing_payload_are_strict() {
        let mut checkpoint: RegistryCheckpointV1 =
            serde_json::from_str(include_str!("../fixtures/registry-v1/checkpoint.json"))
                .expect("checkpoint fixture parses");
        checkpoint.validate().expect("checkpoint validates");

        checkpoint.signature.clear();
        checkpoint
            .signing_payload_bytes()
            .expect("publisher can canonicalize before signing");
        assert!(checkpoint.validate().is_err());

        checkpoint.signature = "AbCdEfGhIjKlMnOpQrStUvWxYz_01234".to_owned();
        checkpoint.previous_checkpoint_sha256 = Some(SHA_A.to_owned());
        assert!(checkpoint.validate().is_err());

        checkpoint.previous_checkpoint_sha256 = None;
        checkpoint.generated_at = "2026-8-07T00:00:00Z".to_owned();
        assert!(checkpoint.validate().is_err());
    }

    #[test]
    fn golden_fixture_chain_is_digest_consistent() {
        use sha2::{Digest as _, Sha256};

        let snapshot: RegistryIndexSnapshotV1 = serde_json::from_str(include_str!(
            "../fixtures/registry-v1/index-snapshot.json"
        ))
        .expect("snapshot fixture parses");
        let checkpoint: RegistryCheckpointV1 =
            serde_json::from_str(include_str!("../fixtures/registry-v1/checkpoint.json"))
                .expect("checkpoint fixture parses");
        let snapshot_digest = hex::encode(Sha256::digest(
            snapshot
                .canonical_json_bytes()
                .expect("snapshot canonicalizes"),
        ));
        assert_eq!(checkpoint.index_root_sha256, snapshot_digest);

        let widget_bytes = include_bytes!("../fixtures/registry-v1/snapshot/index/acme/widget");
        let widget_entry = snapshot
            .entries
            .iter()
            .find(|entry| entry.path == "index/acme/widget")
            .expect("widget index entry exists");
        assert_eq!(widget_entry.size, widget_bytes.len() as u64);
        assert_eq!(widget_entry.sha256, hex::encode(Sha256::digest(widget_bytes)));

        let archive: RegistryArchiveManifestV1 = serde_json::from_str(include_str!(
            "../fixtures/registry-v1/archive-manifest.json"
        ))
        .expect("archive fixture parses");
        let archive_manifest_digest = hex::encode(Sha256::digest(
            archive
                .canonical_json_bytes()
                .expect("archive manifest canonicalizes"),
        ));
        let first_record: RegistryIndexRecordV1 = serde_json::from_str(
            include_str!("../fixtures/registry-v1/index.ndjson")
                .lines()
                .next()
                .expect("index fixture has a record"),
        )
        .expect("first index record parses");
        assert_eq!(first_record.archive.manifest_sha256, archive_manifest_digest);
        assert_eq!(first_record.archive.sha256, archive.archive_sha256);
        assert_eq!(first_record.checkpoint_sequence, checkpoint.sequence);
        discovery()
            .authorize_current_checkpoint_metadata(&checkpoint)
            .expect("accepted discovery authorizes current checkpoint metadata");
    }

'''
    replace_once(
        source,
        "    #[test]\n"
        "    fn lifecycle_reason_is_explicit_and_version_identity_stays_burned() {\n",
        hardening_tests
        + "    #[test]\n"
        + "    fn lifecycle_reason_is_explicit_and_version_identity_stays_burned() {\n",
    )

    discovery_path = Path("fixtures/registry-v1/discovery.json")
    discovery = json.loads(discovery_path.read_text(encoding="utf-8"))
    discovery.update(
        {
            "version": 1,
            "generated_at": "2026-08-07T00:00:00Z",
            "expires_at": "2026-08-08T00:00:00Z",
            "accepted_digest_algorithms": ["sha256"],
            "accepted_archive_formats": ["tar-zstd"],
            "root_signatures": [
                {
                    "key_id": "root-2026-01",
                    "algorithm": "ed25519",
                    "signature": "AbCdEfGhIjKlMnOpQrStUvWxYz_01234",
                }
            ],
        }
    )
    auth = discovery.setdefault("auth", [])
    if not any(item.get("mode") == "anonymous-read" for item in auth):
        auth.insert(0, {"mode": "anonymous-read"})
    discovery_path.write_text(
        json.dumps(discovery, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )

    archive_path = Path("fixtures/registry-v1/archive-manifest.json")
    archive = json.loads(archive_path.read_text(encoding="utf-8"))
    archive_manifest_sha256 = hashlib.sha256(canonical_json_bytes(archive)).hexdigest()

    index_path = Path("fixtures/registry-v1/index.ndjson")
    records = [
        json.loads(line)
        for line in index_path.read_text(encoding="utf-8").splitlines()
        if line
    ]
    records[0]["archive"]["manifest_sha256"] = archive_manifest_sha256
    index_bytes = b"".join(
        canonical_json_bytes(record) + b"\n" for record in records
    )
    index_path.write_bytes(index_bytes)

    snapshot_index_path = Path("fixtures/registry-v1/snapshot/index/acme/widget")
    snapshot_index_path.parent.mkdir(parents=True, exist_ok=True)
    snapshot_index_path.write_bytes(index_bytes)
    snapshot = {
        "schema": "zpkg.registry-index-snapshot/v1",
        "registry_id": "zpkg-registry:0123456789abcdef0123456789abcdef",
        "sequence": 1,
        "entries": [
            {
                "path": "index/acme/widget",
                "sha256": hashlib.sha256(index_bytes).hexdigest(),
                "size": len(index_bytes),
            }
        ],
    }
    snapshot_path = Path("fixtures/registry-v1/index-snapshot.json")
    snapshot_path.write_text(
        json.dumps(snapshot, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )

    checkpoint_path = Path("fixtures/registry-v1/checkpoint.json")
    checkpoint = json.loads(checkpoint_path.read_text(encoding="utf-8"))
    checkpoint["index_root_sha256"] = hashlib.sha256(
        canonical_json_bytes(snapshot)
    ).hexdigest()
    checkpoint.pop("previous_checkpoint_sha256", None)
    checkpoint_path.write_text(
        json.dumps(checkpoint, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )

    replace_once(
        "docs/registry-protocol-v1.md",
        "- exact discovery schema and supported protocol versions;\n"
        "- immutable `registry_id` and canonical HTTPS URL;\n",
        "- exact discovery schema, monotonically chained version, generation/expiry timestamps, and supported protocol versions;\n"
        "- immutable `registry_id` and canonical HTTPS URL;\n",
    )
    replace_once(
        "docs/registry-protocol-v1.md",
        "- metadata-signing keys and rotation state; and\n"
        "- archive, expanded-size, file-count, path-length, and compression-ratio limits.\n",
        "- online metadata-signing keys and rotation state delegated by one or more recovery/root signatures;\n"
        "- accepted digest/compression formats; and\n"
        "- archive, expanded-size, file-count, path-length, and compression-ratio limits.\n",
    )
    replace_once(
        "docs/registry-protocol-v1.md",
        "The canonical URL excludes credentials, query strings, fragments, and a trailing slash. The client records the trust tuple only after an explicit user action or administrator-pinned policy.\n",
        "The canonical URL excludes credentials, query strings, fragments, and a trailing slash. Endpoint templates contain each required placeholder exactly once, contain no unknown placeholders or percent-encoded escapes, and remain same-origin relative paths. The client records the trust tuple only after an explicit user action or administrator-pinned policy.\n\nDiscovery version 1 has no predecessor; every later version carries the SHA-256 of the complete prior signed canonical discovery bytes. Locally enrolled recovery/root keys verify `root_signatures` over canonical discovery payload bytes with the signature list omitted. This delegates the active online checkpoint keys without trusting keys merely because the same HTTPS origin asserted them. Root-key replacement remains an explicit out-of-band recovery ceremony in v1.\n",
    )
    replace_once(
        "docs/registry-protocol-v1.md",
        "## Signed freshness checkpoints\n",
        "## Canonical signing and digest bytes\n\nAll signed or content-addressed v1 JSON uses one RFC-8785-compatible restricted profile: UTF-8, lexicographically sorted ASCII member names, normalized set-like arrays, integer numbers only, no insignificant whitespace, and absent optional members omitted rather than encoded as `null`. Golden vectors are byte-for-byte portable across Rust and non-Rust clients. Timestamps use exactly `YYYY-MM-DDTHH:MM:SSZ` with a valid UTC calendar second; this fixed form makes lexical ordering equivalent to chronological ordering.\n\n## Signed freshness checkpoints\n",
    )
    replace_once(
        "docs/registry-protocol-v1.md",
        "- previous signed-checkpoint digest for every sequence after 1;\n",
        "- no predecessor for sequence 1 and the previous signed-checkpoint digest for every later sequence;\n",
    )


if __name__ == "__main__":
    main()
