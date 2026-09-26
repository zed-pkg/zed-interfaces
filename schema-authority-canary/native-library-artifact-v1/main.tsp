// Independently authored TypeSpec authority for content-addressed native shared libraries.
// Generated JSON Schema is comparison evidence only; authored.schema.json is a peer authority.
import "@typespec/json-schema";
using TypeSpec.JsonSchema;

namespace ZedNativeLibraryArtifact;

enum NativeLibraryOsV1 {
  windows,
  linux,
  macos,
}

enum NativeLibraryArchV1 {
  x86_64,
  aarch64,
}

enum NativeLibraryFormatV1 {
  pe,
  elf,
  machO: "mach_o",
}

@id("NativeLibraryArtifactV1")
model NativeLibraryArtifactV1 {
  contract_version: "zed.native-library-artifact/v1";

  @minLength(1)
  @maxLength(128)
  @pattern("^[a-z][a-z0-9._+-]{0,127}$")
  library_id: string;

  target_os: NativeLibraryOsV1;
  target_arch: NativeLibraryArchV1;
  binary_format: NativeLibraryFormatV1;

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

  @minLength(1)
  @maxLength(255)
  runtime_name: string;

  @minLength(1)
  @maxLength(128)
  abi_version?: string;

  @maxItems(128)
  direct_imports: string[];

  @minLength(1)
  @maxLength(256)
  provenance_ref?: string;
}
