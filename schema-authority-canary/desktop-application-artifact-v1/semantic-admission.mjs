import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));

function readFixture(relativePath) {
  return JSON.parse(readFileSync(resolve(root, relativePath), "utf8"));
}

function isBasename(value) {
  return typeof value === "string"
    && value.length > 0
    && !value.includes("/")
    && !value.includes("\\")
    && value !== "."
    && value !== "..";
}

function semanticErrors(document) {
  const errors = [];
  const formatsByOs = {
    macos: new Set(["macos_pkg", "macos_dmg"]),
    windows: new Set(["windows_exe"]),
    linux: new Set(["linux_snap"]),
  };
  if (!formatsByOs[document.target_os]?.has(document.artifact_format)) {
    errors.push("platform_artifact_format_mismatch");
  }

  if (document.target_arch === "universal" && document.target_os !== "macos") {
    errors.push("universal_arch_is_macos_only");
  }

  if (!isBasename(document.file_name)) {
    errors.push("file_name_must_be_basename");
  } else {
    const extensionByFormat = {
      macos_pkg: ".pkg",
      macos_dmg: ".dmg",
      windows_exe: ".exe",
      linux_snap: ".snap",
    };
    const expected = extensionByFormat[document.artifact_format];
    if (expected && !document.file_name.toLowerCase().endsWith(expected)) {
      errors.push("file_extension_mismatch");
    }
  }

  let repository;
  try {
    repository = new URL(document.source_repository);
  } catch {
    errors.push("invalid_source_repository");
  }
  if (repository) {
    if (repository.protocol !== "https:") {
      errors.push("source_repository_not_https");
    }
    if (repository.username || repository.password || repository.search || repository.hash || repository.port) {
      errors.push("source_repository_not_credential_free_identity");
    }
  }

  const hasPublisher = document.publisher_identity !== undefined;
  if (document.signing_state === "signed" && !hasPublisher) {
    errors.push("signed_artifact_requires_publisher_identity");
  }
  if (document.signing_state === "unsigned" && hasPublisher) {
    errors.push("unsigned_artifact_forbids_publisher_identity");
  }

  if (document.target_os === "macos") {
    if (document.notarization_state === "not_applicable") {
      errors.push("macos_requires_explicit_notarization_state");
    }
    if (document.notarization_state === "notarized" && document.signing_state !== "signed") {
      errors.push("notarized_artifact_must_be_signed");
    }
  } else if (document.notarization_state !== "not_applicable") {
    errors.push("notarization_is_macos_only");
  }

  if (document.minimum_os_version !== undefined
      && !/^[0-9][0-9A-Za-z._+-]{0,63}$/.test(document.minimum_os_version)) {
    errors.push("invalid_minimum_os_version");
  }

  const expectedCdnPath = `apps/artifacts/${document.artifact_sha256}/${document.file_name}`;
  if (document.cdn_object_path !== expectedCdnPath) {
    errors.push("cdn_object_path_must_be_content_addressed");
  }

  return errors;
}

for (const fixture of [
  "instances/DesktopApplicationArtifactV1/valid/macos-pkg.json",
  "instances/DesktopApplicationArtifactV1/valid/windows-exe.json",
  "instances/DesktopApplicationArtifactV1/valid/linux-snap.json",
]) {
  assert.deepEqual(semanticErrors(readFixture(fixture)), [], `${fixture} must be admitted`);
}

const invalid = readFixture(
  "instances/DesktopApplicationArtifactV1/invalid/windows-universal-notarized.json",
);
const invalidErrors = semanticErrors(invalid);
assert.ok(invalidErrors.includes("universal_arch_is_macos_only"));
assert.ok(invalidErrors.includes("notarization_is_macos_only"));

const badPath = readFixture("instances/DesktopApplicationArtifactV1/valid/windows-exe.json");
badPath.cdn_object_path = `apps/channels/stable/${badPath.file_name}`;
assert.ok(semanticErrors(badPath).includes("cdn_object_path_must_be_content_addressed"));

const unsignedPublisher = readFixture("instances/DesktopApplicationArtifactV1/valid/linux-snap.json");
unsignedPublisher.publisher_identity = "unexpected";
assert.ok(semanticErrors(unsignedPublisher).includes("unsigned_artifact_forbids_publisher_identity"));

console.log("DesktopApplicationArtifactV1 semantic admission fixtures passed");
