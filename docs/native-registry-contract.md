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

- at most one portable package;
- at most one generic meta package, with at least one platform edge; and
- one package for each unique platform selector.

When a meta package is present it must select every platform publication in the
record exactly once. Platform-only families remain valid when consumers select
native packages directly without a generic wrapper.

Validation fails closed on version drift, duplicate package identities,
duplicate platforms, dangling or mismatched meta-package edges, malformed
native package names, zero-byte artifacts, uppercase or malformed digests, and
unsupported schema versions. `canonical_json_bytes()` validates first and then
sorts publications and platform edges for deterministic signing and lockfile
provenance.

The contract is transport-neutral. It performs no npm, Cargo-registry, OCI, or
Zed-registry operation and consumes no credential. Independent certification in
`zed-pkg-test/zed-pkg-e2e` regenerates the schema, compiles an external consumer
offline, and pins the exact `zed-interfaces` product commit before promotion.
