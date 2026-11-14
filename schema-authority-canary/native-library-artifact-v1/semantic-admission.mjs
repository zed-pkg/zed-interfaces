import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));

function readFixture(relativePath) {
  return JSON.parse(readFileSync(resolve(root, relativePath), "utf8"));
}

function simpleLoaderName(value) {
  return typeof value === "string" && value.length > 0 && !/[\\/:]/.test(value) && !value.includes("..");
}

function macLoaderName(value) {
  if (typeof value !== "string" || value.length === 0 || value.includes("..") || value.includes("\\")) {
    return false;
  }
  if (simpleLoaderName(value)) {
    return value.endsWith(".dylib");
  }
  return ["@rpath/", "@loader_path/", "@executable_path/"].some((prefix) =>
    value.startsWith(prefix) && simpleLoaderName(value.slice(prefix.length)) && value.endsWith(".dylib"),
  );
}

function validLoaderName(targetOs, value) {
  if (targetOs === "windows") {
    return simpleLoaderName(value) && value.toLowerCase().endsWith(".dll");
  }
  if (targetOs === "linux") {
    return simpleLoaderName(value) && /\.so(?:\.[0-9A-Za-z._+-]+)?$/.test(value);
  }
  if (targetOs === "macos") {
    return macLoaderName(value);
  }
  return false;
}

function semanticErrors(document) {
  const errors = [];
  const formatForOs = {
    windows: "pe",
    linux: "elf",
    macos: "mach_o",
  };
  if (formatForOs[document.target_os] !== document.binary_format) {
    errors.push("platform_binary_format_mismatch");
  }

  if (!simpleLoaderName(document.file_name)) {
    errors.push("file_name_must_be_basename");
  } else if (document.target_os === "windows" && !document.file_name.toLowerCase().endsWith(".dll")) {
    errors.push("windows_file_must_be_dll");
  } else if (document.target_os === "linux" && !/\.so(?:\.[0-9A-Za-z._+-]+)?$/.test(document.file_name)) {
    errors.push("linux_file_must_be_shared_object");
  } else if (document.target_os === "macos" && !document.file_name.endsWith(".dylib")) {
    errors.push("macos_file_must_be_dylib");
  }

  if (!validLoaderName(document.target_os, document.runtime_name)) {
    errors.push("invalid_runtime_loader_name");
  }

  const imports = Array.isArray(document.direct_imports) ? document.direct_imports : [];
  if (new Set(imports).size !== imports.length) {
    errors.push("duplicate_direct_import");
  }
  for (const imported of imports) {
    if (typeof imported !== "string" || imported.length === 0 || imported.length > 255) {
      errors.push("invalid_direct_import_length");
      continue;
    }
    if (!validLoaderName(document.target_os, imported)) {
      errors.push("invalid_direct_import_name");
    }
  }

  if (document.provenance_ref !== undefined && /\s|:\/\//.test(document.provenance_ref)) {
    errors.push("provenance_ref_must_be_opaque_credential_free_identity");
  }

  return errors;
}

for (const fixture of [
  "instances/NativeLibraryArtifactV1/valid/windows-dll.json",
  "instances/NativeLibraryArtifactV1/valid/linux-so.json",
  "instances/NativeLibraryArtifactV1/valid/macos-dylib.json",
]) {
  assert.deepEqual(semanticErrors(readFixture(fixture)), [], `${fixture} must be admitted`);
}

assert.ok(
  semanticErrors(readFixture("instances/NativeLibraryArtifactV1/invalid/windows-elf.json"))
    .includes("platform_binary_format_mismatch"),
);

const duplicateImport = readFixture("instances/NativeLibraryArtifactV1/valid/windows-dll.json");
duplicateImport.direct_imports.push("kernel32.dll");
assert.ok(semanticErrors(duplicateImport).includes("duplicate_direct_import"));

const traversal = readFixture("instances/NativeLibraryArtifactV1/valid/macos-dylib.json");
traversal.runtime_name = "@rpath/../libores_crypto.dylib";
assert.ok(semanticErrors(traversal).includes("invalid_runtime_loader_name"));

console.log("NativeLibraryArtifactV1 semantic admission fixtures passed");
