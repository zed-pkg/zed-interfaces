# Native registry publication contract

`NativeRegistryAdapterRecord` binds one strict-SemVer Zed package target to the
exact immutable archives published to npm or a Cargo-compatible registry.

The contract deliberately separates three identities:

- **API compatibility:** `MAJOR.MINOR.PATCH[-prerelease]`;
- **platform:** explicit `os`, `arch`, and optional `libc` selectors; and
- **bytes:** lowercase SHA-256 plus archive size and format.

SemVer build metadata is rejected at this boundary. It does not participate in
SemVer precedence, and Cargo registry indexes explicitly treat versions that
differ only in build metadata as one version. Architecture-specific artifacts
therefore use distinct package names or manager-native platform selectors while
sharing one strict version.

A publication family may contain:

- one portable package;
- one generic meta package whose platform edges reference packages in the same
  record; and
- one package for each unique platform selector.

Validation fails closed on version drift, duplicate package identities,
duplicate platforms, dangling or mismatched meta-package edges, malformed
native package names, zero-byte artifacts, uppercase or malformed digests, and
unsupported schema versions. `canonical_json_bytes()` validates first and then
sorts publications and platform edges for deterministic signing and lockfile
provenance.
