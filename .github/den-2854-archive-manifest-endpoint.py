#!/usr/bin/env python3
"""One-use follow-up patch for archive-manifest retrieval in protocol v1."""

from __future__ import annotations

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


def main() -> None:
    source = "src/registry_protocol_v1.rs"

    replace_once(
        source,
        "    pub snapshot_manifest_template: String,\n"
        "    pub package_template: String,\n"
        "    pub checkpoint: String,\n",
        "    pub snapshot_manifest_template: String,\n"
        "    pub package_template: String,\n"
        "    pub archive_manifest_template: String,\n"
        "    pub checkpoint: String,\n",
    )

    replace_once(
        source,
        '            &["{org}", "{name}", "{version}"],\n'
        "        )?;\n"
        '        validate_endpoint("endpoints.checkpoint", &self.checkpoint)?;\n',
        '            &["{org}", "{name}", "{version}"],\n'
        "        )?;\n"
        "        validate_endpoint_template(\n"
        '            "endpoints.archive_manifest_template",\n'
        "            &self.archive_manifest_template,\n"
        '            &["{org}", "{name}", "{version}"],\n'
        "        )?;\n"
        '        validate_endpoint("endpoints.checkpoint", &self.checkpoint)?;\n',
    )

    replace_once(
        source,
        '                package_template: "/pkgs/{org}/{name}/{version}.tar.zst".to_owned(),\n'
        '                checkpoint: "/checkpoint.json".to_owned(),\n',
        '                package_template: "/pkgs/{org}/{name}/{version}.tar.zst".to_owned(),\n'
        '                archive_manifest_template: "/pkgs/{org}/{name}/{version}.manifest.json"\n'
        "                    .to_owned(),\n"
        '                checkpoint: "/checkpoint.json".to_owned(),\n',
    )

    replace_once(
        source,
        '        discovery.endpoints.snapshot_manifest_template = "/manifest.json".to_owned();\n'
        "        assert!(discovery.validate().is_err());\n"
        "    }\n",
        '        discovery.endpoints.snapshot_manifest_template = "/manifest.json".to_owned();\n'
        "        assert!(discovery.validate().is_err());\n\n"
        "        let mut discovery = discovery();\n"
        '        discovery.endpoints.archive_manifest_template = "/manifest.json".to_owned();\n'
        "        assert!(discovery.validate().is_err());\n"
        "    }\n",
    )

    discovery_path = Path("fixtures/registry-v1/discovery.json")
    discovery = json.loads(discovery_path.read_text(encoding="utf-8"))
    discovery["endpoints"]["archive_manifest_template"] = (
        "/pkgs/{org}/{name}/{version}.manifest.json"
    )
    discovery_path.write_text(json.dumps(discovery, indent=2) + "\n", encoding="utf-8")

    replace_once(
        "docs/registry-protocol-v1.md",
        '  "package_template": "/pkgs/{org}/{name}/{version}.tar.zst",\n'
        '  "checkpoint": "/checkpoint.json"\n',
        '  "package_template": "/pkgs/{org}/{name}/{version}.tar.zst",\n'
        '  "archive_manifest_template": "/pkgs/{org}/{name}/{version}.manifest.json",\n'
        '  "checkpoint": "/checkpoint.json"\n',
    )

    replace_once(
        "docs/registry-protocol-v1.md",
        "Protocol v1 accepts deterministic `tar.zst` package archives. `RegistryArchiveManifestV1` lists entries in strict bytewise path order and binds the exact archive SHA-256.\n",
        "Protocol v1 accepts deterministic `tar.zst` package archives. The archive is fetched through `package_template`; its canonical sidecar `RegistryArchiveManifestV1` is fetched through `archive_manifest_template`, lists entries in strict bytewise path order, and binds the exact archive SHA-256. The client verifies the sidecar digest from the signed index record before trusting its contents, then verifies the archive digest and extracted entries.\n",
    )


if __name__ == "__main__":
    main()
