// Independently authored TypeSpec authority for immutable desktop application artifacts.
// Generated JSON Schema is comparison evidence only; authored.schema.json is a peer authority.
import "@typespec/json-schema";
using TypeSpec.JsonSchema;

namespace ZedDesktopApplicationArtifact;

enum DesktopReleaseChannelV1 {
  stable,
  beta,
  nightly,
}

enum DesktopTargetOsV1 {
  macos,
  windows,
  linux,
}

enum DesktopTargetArchV1 {
  x86_64,
  aarch64,
  universal,
}

enum DesktopArtifactFormatV1 {
  macosPkg: "macos_pkg",
  macosDmg: "macos_dmg",
  windowsExe: "windows_exe",
  linuxSnap: "linux_snap",
}

enum DesktopSigningStateV1 {
  unsigned,
  signed,
}

enum DesktopNotarizationStateV1 {
  notApplicable: "not_applicable",
  notNotarized: "not_notarized",
  notarized,
}

@id("DesktopApplicationArtifactV1")
model DesktopApplicationArtifactV1 {
  contract_version: "zed.desktop-application-artifact/v1";

  @minLength(1)
  @maxLength(63)
  @pattern("^[a-z][a-z0-9-]{0,62}$")
  org: string;

  @minLength(1)
  @maxLength(128)
  @pattern("^[a-z][a-z0-9._-]{0,127}$")
  app: string;

  @minLength(1)
  @maxLength(128)
  version: string;

  channel: DesktopReleaseChannelV1;
  target_os: DesktopTargetOsV1;
  target_arch: DesktopTargetArchV1;
  artifact_format: DesktopArtifactFormatV1;

  @minLength(64)
  @maxLength(64)
  @pattern("^[a-f0-9]{64}$")
  artifact_sha256: string;

  @minValue(1)
  @maxValue(4294967295)
  artifact_size: uint32;

  @minLength(1)
  @maxLength(255)
  file_name: string;

  @minLength(9)
  @maxLength(512)
  @pattern("^https://[^\\s/@]+(?:/[^\\s]*)?$")
  source_repository: string;

  @minLength(40)
  @maxLength(40)
  @pattern("^[a-f0-9]{40}$")
  source_revision: string;

  @minLength(64)
  @maxLength(64)
  @pattern("^[a-f0-9]{64}$")
  build_receipt_sha256: string;

  @minLength(1)
  @maxLength(64)
  minimum_os_version?: string;

  signing_state: DesktopSigningStateV1;

  @minLength(1)
  @maxLength(256)
  publisher_identity?: string;

  notarization_state: DesktopNotarizationStateV1;

  @minLength(1)
  @maxLength(512)
  cdn_object_path: string;
}
