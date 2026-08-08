#!/usr/bin/env python3
"""One-use exact-source patcher for DEN-2854 immutable index snapshots.

The caller deletes this script in the same commit as the materialized source,
fixtures, and generated schemas. Every replacement is exact and fails if the
branch no longer has the reviewed source shape.
"""

from __future__ import annotations

import json
from pathlib import Path


def replace_once(path: str, old: str, new: str) -> None:
    file = Path(path)
    text = file.read_text(encoding="utf-8")
    count = text.count(old)
    if count != 1:
        raise SystemExit(
            f"{path}: expected one replacement, found {count}: {old[:80]!r}"
        )
    file.write_text(text.replace(old, new, 1), encoding="utf-8")


def main() -> None:
    source = "src/registry_protocol_v1.rs"

    replace_once(
        source,
        'pub const REGISTRY_INDEX_RECORD_SCHEMA_V1: &str = "zpkg.registry-index-record/v1";\n',
        'pub const REGISTRY_INDEX_RECORD_SCHEMA_V1: &str = "zpkg.registry-index-record/v1";\n'
        '/// Immutable sparse-index snapshot-manifest schema.\n'
        'pub const REGISTRY_INDEX_SNAPSHOT_SCHEMA_V1: &str = "zpkg.registry-index-snapshot/v1";\n',
    )

    replace_once(
        source,
        "pub struct RegistryEndpointsV1 {\n"
        "    pub sparse_index_template: String,\n"
        "    pub package_template: String,\n",
        "pub struct RegistryEndpointsV1 {\n"
        "    pub sparse_index_template: String,\n"
        "    pub snapshot_manifest_template: String,\n"
        "    pub package_template: String,\n",
    )

    replace_once(
        source,
        '            &["{org}", "{name}"],\n'
        "        )?;\n"
        "        validate_endpoint_template(\n"
        '            "endpoints.package_template",\n',
        '            &["{snapshot}", "{org}", "{name}"],\n'
        "        )?;\n"
        "        validate_endpoint_template(\n"
        '            "endpoints.snapshot_manifest_template",\n'
        "            &self.snapshot_manifest_template,\n"
        '            &["{snapshot}"],\n'
        "        )?;\n"
        "        validate_endpoint_template(\n"
        '            "endpoints.package_template",\n',
    )

    snapshot_types = r'''
/// One immutable sparse-index object authenticated by a snapshot manifest.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistryIndexSnapshotEntryV1 {
    pub path: String,
    pub sha256: String,
    pub size: u64,
}

impl RegistryIndexSnapshotEntryV1 {
    fn validate(&self, field: &str) -> Result<(), RegistryProtocolV1Error> {
        validate_safe_relative_path(&format!("{field}.path"), &self.path)?;
        let mut parts = self.path.split('/');
        match (parts.next(), parts.next(), parts.next(), parts.next()) {
            (Some("index"), Some(org), Some(name), None) => {
                validate_coordinate_component(&format!("{field}.path.org"), org)?;
                validate_coordinate_component(&format!("{field}.path.name"), name)?;
            }
            _ => {
                return Err(RegistryProtocolV1Error::InvalidValue {
                    field: format!("{field}.path"),
                    value: self.path.clone(),
                });
            }
        }
        validate_sha256(&format!("{field}.sha256"), &self.sha256)?;
        if self.size == 0 {
            return Err(RegistryProtocolV1Error::ZeroValue {
                field: format!("{field}.size"),
            });
        }
        Ok(())
    }
}

/// Canonical manifest for every per-package index in one immutable snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RegistryIndexSnapshotV1 {
    pub schema: String,
    pub registry_id: String,
    pub sequence: u64,
    pub entries: Vec<RegistryIndexSnapshotEntryV1>,
}

impl RegistryIndexSnapshotV1 {
    pub const SCHEMA_V1: &'static str = REGISTRY_INDEX_SNAPSHOT_SCHEMA_V1;

    pub fn validate(&self) -> Result<(), RegistryProtocolV1Error> {
        validate_schema("schema", &self.schema, Self::SCHEMA_V1)?;
        validate_registry_id(&self.registry_id)?;
        if self.sequence == 0 {
            return Err(RegistryProtocolV1Error::ZeroValue {
                field: "sequence".to_owned(),
            });
        }
        if self.entries.is_empty() {
            return Err(RegistryProtocolV1Error::MissingField {
                field: "entries".to_owned(),
            });
        }

        let mut previous: Option<&str> = None;
        for (index, entry) in self.entries.iter().enumerate() {
            entry.validate(&format!("entries[{index}]"))?;
            if previous.is_some_and(|path| path >= entry.path.as_str()) {
                return Err(RegistryProtocolV1Error::NonCanonicalOrder {
                    field: "entries.path".to_owned(),
                });
            }
            previous = Some(&entry.path);
        }
        Ok(())
    }

    /// Canonical bytes whose SHA-256 is selected by a signed checkpoint.
    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>, RegistryProtocolV1Error> {
        self.validate()?;
        serde_json::to_vec(self).map_err(RegistryProtocolV1Error::Serialization)
    }
}

'''
    replace_once(
        source,
        "/// Signed, monotonically advancing freshness checkpoint.\n",
        snapshot_types + "/// Signed, monotonically advancing freshness checkpoint.\n",
    )

    replace_once(
        source,
        "    pub expires_at: String,\n    pub index_root_sha256: String,\n",
        "    pub expires_at: String,\n"
        "    /// SHA-256 of canonical `RegistryIndexSnapshotV1` bytes. The same\n"
        "    /// lowercase digest is substituted into the `{snapshot}` endpoint token.\n"
        "    pub index_root_sha256: String,\n",
    )

    replace_once(
        source,
        "    /// Bytes covered by `signature`. The signature itself is excluded.\n",
        "    /// Immutable snapshot token selected by this checkpoint.\n"
        "    #[must_use]\n"
        "    pub fn snapshot_id(&self) -> &str {\n"
        "        &self.index_root_sha256\n"
        "    }\n\n"
        "    /// Bytes covered by `signature`. The signature itself is excluded.\n",
    )

    replace_once(
        source,
        '                sparse_index_template: "/index/{org}/{name}".to_owned(),\n'
        '                package_template: "/pkgs/{org}/{name}/{version}.tar.zst".to_owned(),\n',
        '                sparse_index_template: "/snapshots/{snapshot}/index/{org}/{name}"\n'
        "                    .to_owned(),\n"
        '                snapshot_manifest_template: "/snapshots/{snapshot}/manifest.json"\n'
        "                    .to_owned(),\n"
        '                package_template: "/pkgs/{org}/{name}/{version}.tar.zst".to_owned(),\n',
    )

    snapshot_helper = r'''
    fn index_snapshot() -> RegistryIndexSnapshotV1 {
        RegistryIndexSnapshotV1 {
            schema: REGISTRY_INDEX_SNAPSHOT_SCHEMA_V1.to_owned(),
            registry_id: REGISTRY_ID.to_owned(),
            sequence: 7,
            entries: vec![
                RegistryIndexSnapshotEntryV1 {
                    path: "index/acme/shared".to_owned(),
                    sha256: SHA_A.to_owned(),
                    size: 21,
                },
                RegistryIndexSnapshotEntryV1 {
                    path: "index/acme/widget".to_owned(),
                    sha256: SHA_B.to_owned(),
                    size: 42,
                },
            ],
        }
    }

'''
    replace_once(
        source,
        "    fn archive_manifest() -> RegistryArchiveManifestV1 {\n",
        snapshot_helper + "    fn archive_manifest() -> RegistryArchiveManifestV1 {\n",
    )

    replace_once(
        source,
        '        index_record().validate().expect("index validates");\n'
        '        archive_manifest().validate().expect("archive validates");\n',
        '        index_record().validate().expect("index validates");\n'
        '        index_snapshot().validate().expect("snapshot validates");\n'
        '        archive_manifest().validate().expect("archive validates");\n',
    )

    snapshot_tests = r'''
    #[test]
    fn static_read_templates_require_the_immutable_snapshot_token() {
        let mut discovery = discovery();
        discovery.endpoints.sparse_index_template = "/index/{org}/{name}".to_owned();
        assert!(discovery.validate().is_err());

        let mut discovery = discovery();
        discovery.endpoints.snapshot_manifest_template = "/manifest.json".to_owned();
        assert!(discovery.validate().is_err());
    }

    #[test]
    fn snapshot_manifest_rejects_non_index_paths_and_noncanonical_order() {
        let mut snapshot = index_snapshot();
        snapshot.entries[0].path = "packages/acme/shared".to_owned();
        assert!(snapshot.validate().is_err());

        let mut snapshot = index_snapshot();
        snapshot.entries.reverse();
        assert!(snapshot.validate().is_err());

        let mut snapshot = index_snapshot();
        snapshot.entries[1].path = snapshot.entries[0].path.clone();
        assert!(snapshot.validate().is_err());
    }

'''
    replace_once(
        source,
        "    #[test]\n"
        "    fn lifecycle_reason_is_explicit_and_version_identity_stays_burned() {\n",
        snapshot_tests
        + "    #[test]\n"
        + "    fn lifecycle_reason_is_explicit_and_version_identity_stays_burned() {\n",
    )

    replace_once(
        source,
        '        assert!(checkpoint.validate().is_err());\n'
        "    }\n\n"
        "    #[test]\n"
        "    fn archive_rejects_traversal_links_and_noncanonical_order() {\n",
        '        assert_eq!(checkpoint.snapshot_id(), SHA_A);\n'
        '        assert!(checkpoint.validate().is_err());\n'
        "    }\n\n"
        "    #[test]\n"
        "    fn archive_rejects_traversal_links_and_noncanonical_order() {\n",
    )

    replace_once(
        source,
        "        let checkpoint: RegistryCheckpointV1 =\n"
        '            serde_json::from_str(include_str!("../fixtures/registry-v1/checkpoint.json"))\n'
        '                .expect("checkpoint fixture parses");\n'
        '        checkpoint.validate().expect("checkpoint fixture validates");\n\n'
        "        let archive: RegistryArchiveManifestV1 = serde_json::from_str(include_str!(\n",
        "        let snapshot: RegistryIndexSnapshotV1 = serde_json::from_str(include_str!(\n"
        '            "../fixtures/registry-v1/index-snapshot.json"\n'
        "        ))\n"
        '        .expect("snapshot fixture parses");\n'
        '        snapshot.validate().expect("snapshot fixture validates");\n\n'
        "        let checkpoint: RegistryCheckpointV1 =\n"
        '            serde_json::from_str(include_str!("../fixtures/registry-v1/checkpoint.json"))\n'
        '                .expect("checkpoint fixture parses");\n'
        '        checkpoint.validate().expect("checkpoint fixture validates");\n\n'
        "        let archive: RegistryArchiveManifestV1 = serde_json::from_str(include_str!(\n",
    )

    replace_once(
        "src/lib.rs",
        "    REGISTRY_DISCOVERY_SCHEMA_V1, REGISTRY_INDEX_RECORD_SCHEMA_V1,\n"
        "    REGISTRY_PROTOCOL_ERROR_SCHEMA_V1, REGISTRY_PROTOCOL_V1, REGISTRY_PUBLISH_REQUEST_SCHEMA_V1,\n",
        "    REGISTRY_DISCOVERY_SCHEMA_V1, REGISTRY_INDEX_RECORD_SCHEMA_V1,\n"
        "    REGISTRY_INDEX_SNAPSHOT_SCHEMA_V1, REGISTRY_PROTOCOL_ERROR_SCHEMA_V1,\n"
        "    REGISTRY_PROTOCOL_V1, REGISTRY_PUBLISH_REQUEST_SCHEMA_V1,\n",
    )
    replace_once(
        "src/lib.rs",
        "    RegistryDiscoveryV1, RegistryEndpointsV1, RegistryIndexRecordV1, RegistryLifecycleStateV1,\n",
        "    RegistryDiscoveryV1, RegistryEndpointsV1, RegistryIndexRecordV1,\n"
        "    RegistryIndexSnapshotEntryV1, RegistryIndexSnapshotV1, RegistryLifecycleStateV1,\n",
    )

    replace_once(
        "examples/generate_schemas.rs",
        '    write::<zed_interfaces::RegistryIndexRecordV1>(dir, "registry-index-record-v1");\n'
        '    write::<zed_interfaces::RegistryCheckpointV1>(dir, "registry-checkpoint-v1");\n',
        '    write::<zed_interfaces::RegistryIndexRecordV1>(dir, "registry-index-record-v1");\n'
        '    write::<zed_interfaces::RegistryIndexSnapshotV1>(dir, "registry-index-snapshot-v1");\n'
        '    write::<zed_interfaces::RegistryCheckpointV1>(dir, "registry-checkpoint-v1");\n',
    )

    discovery_path = Path("fixtures/registry-v1/discovery.json")
    discovery = json.loads(discovery_path.read_text(encoding="utf-8"))
    endpoints = discovery["endpoints"]
    endpoints["sparse_index_template"] = "/snapshots/{snapshot}/index/{org}/{name}"
    endpoints["snapshot_manifest_template"] = "/snapshots/{snapshot}/manifest.json"
    discovery_path.write_text(json.dumps(discovery, indent=2) + "\n", encoding="utf-8")

    snapshot_fixture = {
        "schema": "zpkg.registry-index-snapshot/v1",
        "registry_id": "zpkg-registry:0123456789abcdef0123456789abcdef",
        "sequence": 1,
        "entries": [
            {
                "path": "index/acme/shared",
                "sha256": "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
                "size": 123,
            },
            {
                "path": "index/acme/widget",
                "sha256": "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
                "size": 456,
            },
        ],
    }
    Path("fixtures/registry-v1/index-snapshot.json").write_text(
        json.dumps(snapshot_fixture, indent=2) + "\n", encoding="utf-8"
    )


if __name__ == "__main__":
    main()
