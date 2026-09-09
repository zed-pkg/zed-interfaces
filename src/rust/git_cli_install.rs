//! Cross-runtime receipt contract for revision-pinned Git CLI installation.
//!
//! The wire shape is independently authored in TypeSpec and JSON Schema under
//! `schema-authority-canary/git-cli-install-v1/` and compared by the pinned
//! ORESoftware/typespec-json-schema-validator gate. This Rust DTO is a runtime
//! projection of that accepted peer-authority contract; it is not allowed to
//! redefine the wire shape locally in zed-cli.

use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize};

pub const GIT_CLI_INSTALL_RECEIPT_SCHEMA_VERSION_V1: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GitCliInstallReceiptV1 {
    pub schema_version: u32,
    pub source: String,
    pub revision: String,
    pub binary: String,
    pub installed_path: String,
    pub sha256: String,
    pub manifest: String,
    /// Required on the wire but nullable when a CLI has no flags contract.
    #[serde(deserialize_with = "deserialize_required_nullable_string")]
    pub flags_contract: Option<String>,
}

impl GitCliInstallReceiptV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != GIT_CLI_INSTALL_RECEIPT_SCHEMA_VERSION_V1 {
            return Err(format!(
                "unsupported Git CLI install receipt schema version {}",
                self.schema_version
            ));
        }
        if self.source.trim().is_empty() {
            return Err("Git CLI install receipt source must not be empty".to_owned());
        }
        if !valid_revision(&self.revision) {
            return Err(
                "Git CLI install receipt revision must be a full 40- or 64-character hexadecimal commit id"
                    .to_owned(),
            );
        }
        if !valid_binary_name(&self.binary) {
            return Err(
                "Git CLI install receipt binary must contain only ASCII letters, digits, '-' and '_'"
                    .to_owned(),
            );
        }
        if self.installed_path.trim().is_empty() {
            return Err("Git CLI install receipt installed_path must not be empty".to_owned());
        }
        if self.sha256.len() != 64
            || !self
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            return Err(
                "Git CLI install receipt sha256 must be 64 lowercase hexadecimal characters"
                    .to_owned(),
            );
        }
        if self.manifest.trim().is_empty() {
            return Err("Git CLI install receipt manifest must not be empty".to_owned());
        }
        if self
            .flags_contract
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err("Git CLI install receipt flags_contract must be null or non-empty".to_owned());
        }
        Ok(())
    }
}

fn deserialize_required_nullable_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer)
}

fn valid_revision(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_binary_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn fixture() -> GitCliInstallReceiptV1 {
        GitCliInstallReceiptV1 {
            schema_version: GIT_CLI_INSTALL_RECEIPT_SCHEMA_VERSION_V1,
            source: "https://github.com/ORESoftware/ores-cli.git".to_owned(),
            revision: "387bce152d9572c014710d68062f979c3614276d".to_owned(),
            binary: "ores-cli".to_owned(),
            installed_path: "/home/alex/.local/bin/ores-cli".to_owned(),
            sha256: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
                .to_owned(),
            manifest: ".zpkg.toml".to_owned(),
            flags_contract: Some(".cli-flags.toml".to_owned()),
        }
    }

    #[test]
    fn canonical_fixture_validates() {
        fixture().validate().unwrap();
    }

    #[test]
    fn serialized_wire_names_match_peer_authorities() {
        assert_eq!(
            serde_json::to_value(fixture()).unwrap(),
            json!({
                "schema_version": 1,
                "source": "https://github.com/ORESoftware/ores-cli.git",
                "revision": "387bce152d9572c014710d68062f979c3614276d",
                "binary": "ores-cli",
                "installed_path": "/home/alex/.local/bin/ores-cli",
                "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                "manifest": ".zpkg.toml",
                "flags_contract": ".cli-flags.toml"
            })
        );
    }

    #[test]
    fn unknown_fields_fail_closed() {
        let mut value = serde_json::to_value(fixture()).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("credential".to_owned(), json!("must-never-be-admitted"));
        assert!(serde_json::from_value::<GitCliInstallReceiptV1>(value).is_err());
    }

    #[test]
    fn missing_required_nullable_flags_contract_fails_closed() {
        let mut value = serde_json::to_value(fixture()).unwrap();
        value.as_object_mut().unwrap().remove("flags_contract");
        assert!(serde_json::from_value::<GitCliInstallReceiptV1>(value).is_err());
    }

    #[test]
    fn null_flags_contract_is_admitted() {
        let mut value = serde_json::to_value(fixture()).unwrap();
        value
            .as_object_mut()
            .unwrap()
            .insert("flags_contract".to_owned(), serde_json::Value::Null);
        let receipt = serde_json::from_value::<GitCliInstallReceiptV1>(value).unwrap();
        assert_eq!(receipt.flags_contract, None);
        receipt.validate().unwrap();
    }

    #[test]
    fn semantic_identity_and_digest_checks_fail_closed() {
        let mutations: [fn(&mut GitCliInstallReceiptV1); 4] = [
            |receipt| receipt.schema_version = 2,
            |receipt| receipt.revision = "main".to_owned(),
            |receipt| receipt.binary = "../ores-cli".to_owned(),
            |receipt| receipt.sha256 = "ABCDEF".repeat(10),
        ];
        for mutate in mutations {
            let mut receipt = fixture();
            mutate(&mut receipt);
            assert!(receipt.validate().is_err());
        }
    }
}
