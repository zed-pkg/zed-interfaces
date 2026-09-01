use std::{collections::BTreeSet, fs, path::Path};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{native_host::NativeRegistryHost, registry::RegistrySignatureV1};

pub const MIRROR_DISCOVERY_SCHEMA: &str = "zed.registry.mirror-discovery.v1";
pub const MIRROR_CAPABILITIES_SCHEMA: &str = "zed.registry.mirror-capabilities.v1";
pub const MIRROR_HEALTH_SCHEMA: &str = "zed.registry.mirror-health.v1";
pub const DEFAULT_INDEX_PREFIX: &str = "index/";
pub const DEFAULT_ARTIFACT_PREFIX: &str = "artifacts/";
pub const DEFAULT_RAW_PREFIX: &str = "zpkg/";
pub const GITHUB_HOST: &str = "github.com";

#[derive(Debug, thiserror::Error)]
pub enum MirrorError {
    #[error("{0}")]
    Message(String),
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MirrorKindV1 {
    ZedRegistry,
    #[default]
    ObjectStore,
    Directory,
    GithubRelease,
    GithubRaw,
}

impl MirrorKindV1 {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ZedRegistry => "zed-registry",
            Self::ObjectStore => "object-store",
            Self::Directory => "directory",
            Self::GithubRelease => "github-release",
            Self::GithubRaw => "github-raw",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MirrorServesV1 {
    #[serde(default = "default_true")]
    pub artifacts: bool,
    #[serde(default = "default_true")]
    pub metadata: bool,
    #[serde(default = "default_true")]
    pub index: bool,
}

impl Default for MirrorServesV1 {
    fn default() -> Self {
        Self {
            artifacts: true,
            metadata: true,
            index: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MirrorDescriptorV1 {
    pub schema_version: u32,
    pub mirror_id: String,
    pub display_name: String,
    pub base_url: String,
    #[serde(default)]
    pub kind: MirrorKindV1,
    #[serde(default)]
    pub serves: MirrorServesV1,
    #[serde(default)]
    pub native_formats: BTreeSet<NativeRegistryHost>,
    #[serde(default)]
    pub priority: u16,
    #[serde(default)]
    pub public_key: Option<String>,
}

impl MirrorDescriptorV1 {
    pub fn validate(&self) -> Result<(), MirrorError> {
        require_schema_version(self.schema_version)?;
        require_id(&self.mirror_id, "mirror_id")?;
        require_non_empty(&self.display_name, "display_name")?;
        require_absolute_base_url(&self.base_url, self.kind)?;
        if !self.serves.artifacts && !self.serves.metadata && !self.serves.index {
            return Err(message("mirror must serve at least one capability"));
        }
        if self.priority > 10_000 {
            return Err(message("priority must be <= 10000"));
        }
        if self.public_key.as_deref().is_some_and(str::is_empty) {
            return Err(message("public_key must be omitted or non-empty"));
        }
        Ok(())
    }

    pub fn supports(&self, host: NativeRegistryHost) -> bool {
        self.native_formats.contains(&host)
    }

    pub fn metadata_base(&self) -> String {
        join_url(&self.base_url, DEFAULT_INDEX_PREFIX)
    }

    pub fn artifact_base(&self) -> String {
        join_url(&self.base_url, DEFAULT_ARTIFACT_PREFIX)
    }

    pub fn raw_base(&self) -> String {
        join_url(&self.base_url, DEFAULT_RAW_PREFIX)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MirrorDiscoveryV1 {
    pub schema: String,
    pub generated_at: String,
    pub mirrors: Vec<MirrorDescriptorV1>,
    pub signatures: Vec<RegistrySignatureV1>,
}

impl MirrorDiscoveryV1 {
    pub fn validate(&self) -> Result<(), MirrorError> {
        if self.schema != MIRROR_DISCOVERY_SCHEMA {
            return Err(message(format!(
                "unsupported mirror discovery schema {}; expected {MIRROR_DISCOVERY_SCHEMA}",
                self.schema
            )));
        }
        require_timestamp(&self.generated_at, "generated_at")?;
        if self.mirrors.is_empty() {
            return Err(message("mirror discovery must contain at least one mirror"));
        }
        require_unique_ids(self.mirrors.iter().map(|mirror| mirror.mirror_id.as_str()))?;
        for mirror in &self.mirrors {
            mirror.validate()?;
        }
        if self.signatures.is_empty() {
            return Err(message("mirror discovery requires at least one signature"));
        }
        for signature in &self.signatures {
            signature.validate()?;
        }
        Ok(())
    }

    pub fn canonical_payload(&self) -> Result<Vec<u8>, MirrorError> {
        let mut unsigned = self.clone();
        unsigned.signatures.clear();
        unsigned
            .mirrors
            .sort_by(|left, right| left.mirror_id.cmp(&right.mirror_id));
        let mut value = serde_json::to_value(unsigned)
            .map_err(|error| message(format!("cannot serialize mirror discovery: {error}")))?;
        sort_json(&mut value);
        serde_json::to_vec(&value)
            .map_err(|error| message(format!("cannot encode mirror discovery: {error}")))
    }

    pub fn canonical_sha256(&self) -> Result<String, MirrorError> {
        Ok(hex::encode(Sha256::digest(self.canonical_payload()?)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MirrorCapabilitiesV1 {
    pub schema: String,
    pub mirror_id: String,
    pub generated_at: String,
    pub serves: MirrorServesV1,
    pub native_formats: BTreeSet<NativeRegistryHost>,
    pub artifact_base: String,
    pub metadata_base: String,
    pub raw_base: String,
}

impl MirrorCapabilitiesV1 {
    pub fn validate(&self) -> Result<(), MirrorError> {
        if self.schema != MIRROR_CAPABILITIES_SCHEMA {
            return Err(message(format!(
                "unsupported mirror capabilities schema {}; expected {MIRROR_CAPABILITIES_SCHEMA}",
                self.schema
            )));
        }
        require_id(&self.mirror_id, "mirror_id")?;
        require_timestamp(&self.generated_at, "generated_at")?;
        require_absolute_base_url(&self.artifact_base, MirrorKindV1::ObjectStore)?;
        require_absolute_base_url(&self.metadata_base, MirrorKindV1::ObjectStore)?;
        require_absolute_base_url(&self.raw_base, MirrorKindV1::ObjectStore)?;
        if !self.serves.artifacts && !self.serves.metadata && !self.serves.index {
            return Err(message("capabilities must advertise at least one service"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MirrorHealthV1 {
    pub schema: String,
    pub mirror_id: String,
    pub checked_at: String,
    pub healthy: bool,
    pub latency_ms: u64,
    #[serde(default)]
    pub failures: Vec<String>,
}

impl MirrorHealthV1 {
    pub fn validate(&self) -> Result<(), MirrorError> {
        if self.schema != MIRROR_HEALTH_SCHEMA {
            return Err(message(format!(
                "unsupported mirror health schema {}; expected {MIRROR_HEALTH_SCHEMA}",
                self.schema
            )));
        }
        require_id(&self.mirror_id, "mirror_id")?;
        require_timestamp(&self.checked_at, "checked_at")?;
        if self.healthy && !self.failures.is_empty() {
            return Err(message("healthy mirror reports cannot contain failures"));
        }
        if !self.healthy && self.failures.is_empty() {
            return Err(message("unhealthy mirror reports require at least one failure"));
        }
        Ok(())
    }
}

pub fn write_discovery(path: &Path, discovery: &MirrorDiscoveryV1) -> Result<(), MirrorError> {
    discovery.validate()?;
    let mut value = serde_json::to_value(discovery)
        .map_err(|error| message(format!("cannot serialize mirror discovery: {error}")))?;
    sort_json(&mut value);
    let mut bytes = serde_json::to_vec_pretty(&value)
        .map_err(|error| message(format!("cannot encode mirror discovery: {error}")))?;
    bytes.push(b'\n');
    fs::write(path, bytes)
        .map_err(|error| message(format!("cannot write {}: {error}", path.display())))
}

pub fn read_discovery(path: &Path) -> Result<MirrorDiscoveryV1, MirrorError> {
    let bytes = fs::read(path)
        .map_err(|error| message(format!("cannot read {}: {error}", path.display())))?;
    let discovery: MirrorDiscoveryV1 = serde_json::from_slice(&bytes)
        .map_err(|error| message(format!("cannot decode {}: {error}", path.display())))?;
    discovery.validate()?;
    Ok(discovery)
}

pub fn artifact_url(base: &str, org: &str, name: &str, version: &str, file: &str) -> String {
    let key = artifact_key(org, name, version, file);
    join_url(base, &key)
}

pub fn artifact_key(org: &str, name: &str, version: &str, file: &str) -> String {
    format!(
        "{DEFAULT_ARTIFACT_PREFIX}{org}/{name}/{version}/{file}",
        org = encode_segment(org),
        name = encode_segment(name),
        version = encode_segment(version),
        file = encode_segment(file),
    )
}

pub fn metadata_url(base: &str, org: &str, name: &str) -> String {
    let key = metadata_key(org, name);
    join_url(base, &key)
}

pub fn metadata_key(org: &str, name: &str) -> String {
    format!(
        "{DEFAULT_INDEX_PREFIX}{org}/{name}.json",
        org = encode_segment(org),
        name = encode_segment(name),
    )
}

pub fn github_release_url(repo: &str, tag: &str, asset: &str) -> Result<String, MirrorError> {
    let (owner, name) = parse_github_repo(repo)?;
    Ok(format!(
        "https://github.com/{owner}/{name}/releases/download/{tag}/{asset}",
        owner = encode_segment(&owner),
        name = encode_segment(&name),
        tag = encode_segment(tag),
        asset = encode_segment(asset),
    ))
}

pub fn github_raw_url(repo: &str, revision: &str, path: &str) -> Result<String, MirrorError> {
    let (owner, name) = parse_github_repo(repo)?;
    if revision.is_empty() {
        return Err(message("revision must not be empty"));
    }
    if path.is_empty() || path.starts_with('/') || path.contains("..") {
        return Err(message("path must be a safe relative path"));
    }
    Ok(format!(
        "https://raw.githubusercontent.com/{owner}/{name}/{revision}/{path}",
        owner = encode_segment(&owner),
        name = encode_segment(&name),
        revision = encode_segment(revision),
        path = path
            .split('/')
            .map(encode_segment)
            .collect::<Vec<_>>()
            .join("/"),
    ))
}

fn parse_github_repo(repo: &str) -> Result<(String, String), MirrorError> {
    let trimmed = repo.trim().trim_end_matches('/');
    let without_suffix = trimmed.strip_suffix(".git").unwrap_or(trimmed);
    let normalized = without_suffix
        .strip_prefix("https://github.com/")
        .or_else(|| without_suffix.strip_prefix("http://github.com/"))
        .or_else(|| without_suffix.strip_prefix("git@github.com:"))
        .unwrap_or(without_suffix);
    let mut parts = normalized.split('/');
    let owner = parts.next().unwrap_or_default();
    let name = parts.next().unwrap_or_default();
    if owner.is_empty() || name.is_empty() || parts.next().is_some() {
        return Err(message(format!(
            "invalid GitHub repository identity {repo:?}; expected owner/name"
        )));
    }
    Ok((owner.to_owned(), name.to_owned()))
}

fn require_schema_version(value: u32) -> Result<(), MirrorError> {
    if value != 1 {
        return Err(message(format!(
            "unsupported mirror descriptor schema version {value}; expected 1"
        )));
    }
    Ok(())
}

fn require_id(value: &str, field: &str) -> Result<(), MirrorError> {
    if value.is_empty() || value.len() > 128 {
        return Err(message(format!(
            "{field} must be between 1 and 128 characters"
        )));
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(message(format!(
            "{field} must contain only lowercase ASCII letters, digits, and hyphens"
        )));
    }
    Ok(())
}

fn require_non_empty(value: &str, field: &str) -> Result<(), MirrorError> {
    if value.trim().is_empty() {
        return Err(message(format!("{field} must not be empty")));
    }
    Ok(())
}

fn require_absolute_base_url(value: &str, kind: MirrorKindV1) -> Result<(), MirrorError> {
    if value.is_empty() {
        return Err(message("base URL must not be empty"));
    }
    if matches!(kind, MirrorKindV1::Directory) {
        let path = Path::new(value);
        if !path.is_absolute() {
            return Err(message("directory mirrors require an absolute path"));
        }
        return Ok(());
    }
    if !value.starts_with("https://") && !value.starts_with("http://127.0.0.1:") {
        return Err(message("mirror URL must use https:// or loopback http://"));
    }
    Ok(())
}

fn require_timestamp(value: &str, field: &str) -> Result<(), MirrorError> {
    if value.len() < 20 || !value.ends_with('Z') || !value.contains('T') {
        return Err(message(format!(
            "{field} must be an RFC3339 UTC timestamp"
        )));
    }
    Ok(())
}

fn require_unique_ids<'a>(values: impl Iterator<Item = &'a str>) -> Result<(), MirrorError> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(message(format!("duplicate mirror id {value}")));
        }
    }
    Ok(())
}

fn join_url(base: &str, suffix: &str) -> String {
    format!("{}/{}", base.trim_end_matches('/'), suffix.trim_start_matches('/'))
}

fn encode_segment(value: &str) -> String {
    value
        .bytes()
        .flat_map(|byte| {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
                vec![byte as char]
            } else {
                format!("%{byte:02X}").chars().collect()
            }
        })
        .collect()
}

fn sort_json(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(values) => {
            for item in values {
                sort_json(item);
            }
        }
        serde_json::Value::Object(object) => {
            let mut entries = object
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect::<Vec<_>>();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            object.clear();
            for (key, mut value) in entries {
                sort_json(&mut value);
                object.insert(key, value);
            }
        }
        _ => {}
    }
}

fn default_true() -> bool {
    true
}

fn message(message: impl Into<String>) -> MirrorError {
    MirrorError::Message(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor() -> MirrorDescriptorV1 {
        MirrorDescriptorV1 {
            schema_version: 1,
            mirror_id: "primary".to_owned(),
            display_name: "Primary".to_owned(),
            base_url: "https://mirror.example.test/".to_owned(),
            kind: MirrorKindV1::ObjectStore,
            serves: MirrorServesV1::default(),
            native_formats: BTreeSet::new(),
            priority: 10,
            public_key: None,
        }
    }

    #[test]
    fn descriptor_validation_is_fail_closed() {
        let mut value = descriptor();
        value.mirror_id = "UPPER".to_owned();
        assert!(value.validate().is_err());
        value = descriptor();
        value.serves = MirrorServesV1 {
            artifacts: false,
            metadata: false,
            index: false,
        };
        assert!(value.validate().is_err());
    }

    #[test]
    fn deterministic_discovery_payload_sorts_mirrors_and_keys() {
        let mut second = descriptor();
        second.mirror_id = "secondary".to_owned();
        let mut discovery = MirrorDiscoveryV1 {
            schema: MIRROR_DISCOVERY_SCHEMA.to_owned(),
            generated_at: "2026-08-29T00:00:00Z".to_owned(),
            mirrors: vec![second, descriptor()],
            signatures: vec![RegistrySignatureV1 {
                algorithm: crate::registry::REGISTRY_SIGNATURE_ALGORITHM.to_owned(),
                key_id: "publisher".to_owned(),
                signature: "ed25519:signature".to_owned(),
            }],
        };
        let digest_a = discovery.canonical_sha256().unwrap();
        discovery.mirrors.reverse();
        let digest_b = discovery.canonical_sha256().unwrap();
        assert_eq!(digest_a, digest_b);
    }

    #[test]
    fn artifact_and_metadata_keys_are_guessable() {
        assert_eq!(
            artifact_key("acme", "widget", "1.2.3", "package.tgz"),
            "artifacts/acme/widget/1.2.3/package.tgz"
        );
        assert_eq!(metadata_key("acme", "widget"), "index/acme/widget.json");
    }

    #[test]
    fn github_helpers_are_deterministic() {
        assert_eq!(
            github_release_url("acme/widget", "v1.2.3", "widget.tgz").unwrap(),
            "https://github.com/acme/widget/releases/download/v1.2.3/widget.tgz"
        );
        assert_eq!(
            github_raw_url("https://github.com/acme/widget.git", "abc123", "dist/widget.tgz")
                .unwrap(),
            "https://raw.githubusercontent.com/acme/widget/abc123/dist/widget.tgz"
        );
    }
}
