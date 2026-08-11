# Zed binary artifacts v1

Status: versioned interface contract; CLI and registry rollout remains capability-gated.

## Decision

A native executable is published as a deterministic ZIP artifact. Every ZIP has one canonical package root:

```text
pkg/
  .zpkg.toml
  .zpkg-binary.json
  bin/
    zed-tool
  lib/
    libzed_tool.so        # optional explicit runtime payload
  LICENSE                 # optional explicit payload
```

`.zpkg.toml` is the same package manifest used for source packages. It is a sibling of the `bin/` payload directory at the `pkg/` root, while each executable normally lives below `pkg/bin/`. This gives the archive exactly one manifest even when it exposes several commands or carries runtime libraries. Its `[bin]` table is authoritative for command names and package-root-relative paths:

```toml
[package]
org = "acme"
name = "zed-tool"
version = "1.2.3"

[package.repository]
vcs = "git"
url = "https://github.com/acme/zed-tool"

[bin]
zed-tool = "bin/zed-tool"
```

`.zpkg-binary.json` is generated, not authored. It binds the release identity, the selected artifact platform, every payload file's SHA-256 and size, executable intent, and optional VCS provenance. It excludes itself from its `files` array to avoid a circular digest. The registry/lockfile SHA-256 of the complete ZIP remains the outer trust anchor.

The verifier parses the exact `pkg/.zpkg.toml` bytes listed in the descriptor and requires:

- descriptor `package` equals manifest `package.org`, `package.name`, and `package.version`;
- descriptor `entrypoints` equals the complete manifest `[bin]` map, not merely a subset; and
- every `[bin]` path is a listed regular file marked executable.

The Rust interface exposes this cross-document check as `BinaryArtifactManifestV1::validate_against_manifest`. Descriptor-only validation is not sufficient for promotion or install.

## Identity model

Platform is never encoded in SemVer build metadata.

- Release identity: `org/name/version`
- Artifact identity: release identity + target/platform + format
- Blob identity: lowercase SHA-256 of the exact ZIP bytes

A release may eventually have many immutable binary artifacts, such as Linux x86-64 glibc, Linux ARM64 musl, macOS universal2, and Windows x86-64. Re-publishing the same artifact identity with the same bytes is idempotent; different bytes for an existing artifact identity are rejected.

The legacy `/v1/packages/{org}/{name}/versions/{version}` route currently stores one artifact per release. A compatibility publisher may upload one ZIP there, but multi-platform publication requires an artifact-qualified registry route and storage key. The intended route family is:

```text
PUT /v1/packages/{org}/{name}/versions/{version}/artifacts/{target}/{format}
GET /v1/packages/{org}/{name}/versions/{version}/artifacts
GET /v1/packages/{org}/{name}/versions/{version}/artifacts/{target}/{format}
```

The old version route remains a source-artifact/compatibility alias until clients negotiate the multi-artifact capability.

The path tokens are literal normalized tokens; clients do not put platform data in a query string or SemVer and do not invent a fallback path. Shared path builders are:

- `binary_artifacts_path(org, name, version)` for the collection; and
- `binary_artifact_path(org, name, version, target, format)` for exact `GET`/`PUT`.

An exact item `GET` or successful `PUT` response uses `BinaryArtifactMetadataV1`. It carries `org`, `name`, `version`, the full normalized platform (`target`, `os`, `arch`, optional `libc`/`abi`), `format`, ZIP SHA-256/size, descriptor SHA-256, immutable download URL, publication time, lifecycle state, optional source provenance, and digest-addressed evidence. `BinaryArtifactListResponseV1` is strictly ordered by `(platform.target, format)` and rejects duplicate identities.

## ZIP profile

Binary v1 accepts ZIP only.

Required rules:

1. Every entry is beneath `pkg/`.
2. Exactly one `pkg/.zpkg.toml` and one `pkg/.zpkg-binary.json` exist.
3. All payload entries are regular files or directories. Symlinks, hard links, devices, sockets, and FIFOs are rejected.
4. Paths are UTF-8, relative, slash-separated, at most 4,096 bytes overall, and at most 255 UTF-8 bytes per component. Empty, `.`, `..`, drive-prefix, backslash, NUL/control, trailing dot/space, Win32-reserved character, and Win32 device-name components are rejected.
5. Duplicate names and collisions after slash normalization plus Unicode lowercase conversion are rejected before extraction. This key uses Rust-style Unicode `to_lowercase()` only; it does not perform Unicode normalization or full Unicode case folding.
6. The descriptor's file paths are relative to `pkg/`; it lists every regular file except itself and no extras are allowed.
7. Every `[bin]` path exists, is listed in the descriptor, and has `executable: true`.
8. Canonical packers emit regular files only (no directory entries), ordered by the UTF-8 bytes of their complete archive paths. All local and central-directory timestamps are the DOS epoch `1980-01-01T00:00:00`; modes are exactly `0644` or `0755` from descriptor executable intent; and UID/GID, archive comments, file comments, and nonessential extra fields are absent.
9. Canonical packers use raw DEFLATE at fixed level 6. Verifiers accept only Stored or Deflated entries for interoperability. Encrypted entries, data-descriptor ambiguity, multi-disk archives, ZIP64 when ordinary ZIP limits suffice, and self-extracting prefixes are rejected. Reproducibility tests pin exact bytes for the supported writer implementation.
10. The verifier enforces archive-byte, expanded-byte, file-count, path-length, and compression-ratio limits before promotion.

The interoperable v1 verifier defaults are a 1 GiB archive, 2 GiB total expanded payload, 200,000 central-directory entries, a 1,000:1 per-file expansion ratio, a 4 MiB descriptor, a 4 MiB embedded manifest, a 4,096-byte relative path, and a 255-byte path component. A deployment may impose lower ceilings. A local diagnostic override does not relax the registry's acceptance policy and must not make an artifact portable by definition.

## Publish pipeline

The secure publication sequence is:

1. Parse and validate the authored `.zpkg.toml`.
2. Require at least one `[bin]` entry.
3. Select one normalized target/platform explicitly; host inference must be visible in command output.
4. Collect only declared entrypoints plus explicitly included runtime files/directories and selected legal files. Do not sweep an arbitrary build tree by default.
5. Reject symlinks and unsafe/portable-colliding paths.
6. Hash each payload file and generate `.zpkg-binary.json`.
7. Write a deterministic ZIP beneath `pkg/`.
8. Re-open and fully verify the produced ZIP using the same verifier used for downloads.
9. Compute the complete archive SHA-256 and size.
10. Verify VCS tag/commit provenance.
11. Upload metadata plus bytes. The server recomputes the SHA-256, re-verifies the ZIP profile, and commits artifact metadata transactionally.
12. Sign or attest the release/artifact/archive tuple when registry signing is enabled.

The multipart JSON half is `BinaryArtifactPublishMetaV1`. Before accepting bytes, a server validates its ordinary manifest, requires `[bin]`, checks normalized platform/format, verifies ZIP and descriptor digests, and validates every evidence reference against the ZIP digest.

Publication storage has a unique immutable key `(org, name, version, target, format)`. A repeated `PUT` succeeds idempotently only when platform, format, ZIP SHA-256/size, descriptor SHA-256, source provenance, and all attachment bindings equal the accepted record. Any difference returns an immutable-artifact conflict; it never overwrites the object or metadata row. Object storage uses conditional create, and metadata plus object visibility are committed so readers see either the previous complete release view or the next one.

No package hook, install script, or binary is executed while packing, uploading, downloading, inspecting, or verifying.

## Download and install pipeline

1. Resolve an artifact using the host target plus any explicit override.
2. Download to a temporary content-addressed path with a hard byte limit.
3. Verify the registry/lockfile archive SHA-256 before opening the ZIP.
4. Inspect the entire central directory without writing files.
5. Validate path, type, duplicate, collision, compression, count, and expanded-size limits.
6. Parse `.zpkg-binary.json` and `.zpkg.toml`; require matching package identity and `[bin]` maps.
7. Stream and hash every payload file; reject missing, extra, or changed bytes.
8. Extract into a private staging directory without following links.
9. Apply executable intent only to declared files.
10. Atomically promote the completed `pkg/` tree into the content-addressed store.

A failure leaves no partially promoted package and never mutates an existing store entry.

## Descriptor example

```json
{
  "schema": "zpkg.binary-artifact/v1",
  "package": {
    "org": "acme",
    "name": "zed-tool",
    "version": "1.2.3"
  },
  "platform": {
    "target": "x86_64-unknown-linux-gnu",
    "os": "linux",
    "arch": "x86_64",
    "libc": "gnu"
  },
  "format": "zip",
  "package_manifest": ".zpkg.toml",
  "expanded_size": 4123456,
  "files": [
    {
      "path": ".zpkg.toml",
      "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
      "size": 386,
      "executable": false
    },
    {
      "path": "bin/zed-tool",
      "sha256": "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
      "size": 4123070,
      "executable": true
    }
  ],
  "entrypoints": {
    "zed-tool": "bin/zed-tool"
  },
  "source": {
    "repository": "https://github.com/acme/zed-tool",
    "vcs_tag": "v1.2.3",
    "vcs_commit": "0123456789abcdef0123456789abcdef01234567"
  }
}
```

## Lock, signing, provenance, and SBOM binding

A frozen binary resolution uses `BinaryArtifactLockV1` and carries all information needed to reject platform substitution:

- release identity plus full normalized platform and format;
- exact ZIP SHA-256 and byte size;
- exact canonical `.zpkg-binary.json` SHA-256;
- canonical registry origin and optional immutable download URL;
- optional source repository, VCS tag, and immutable commit; and
- strictly ordered signature, attestation, provenance, and SBOM references.

Every `BinaryArtifactAttachmentV1` includes its kind, media type, SHA-256, byte size, immutable URL, and `subject_sha256`. The subject must equal the locked ZIP digest. This prevents evidence for one valid archive from being replayed for another. Absence is encoded by omitting optional members; canonical JSON does not use explicit `null`.

`BinaryArtifactLockV1` is deliberately standalone in this revision. Silently adding fields to `LockedPackage` in `.zpkg.lock` v1 would let an older client parse and then discard security-critical platform/evidence data on rewrite. A lockfile envelope may embed this record only with an explicit version bump; unsupported readers must fail closed and must never rewrite a newer lock. Until that integration lands, qualified binary installs must not pretend the generic v1 lock entry fully represents them.

The inner descriptor is integrity metadata, not an independent trust root. Registry signatures or an in-toto/DSSE statement should cover:

- registry identity;
- release identity;
- target/platform and format;
- archive SHA-256 and byte size;
- descriptor SHA-256;
- source repository, tag, and commit;
- builder identity and workflow reference when available;
- SBOM/provenance attachment digests.

The registry's signed index/checkpoint metadata binds the archive and descriptor digests. Detached evidence is supplementary and is itself content-addressed through the metadata and lock record. A verifier checks the registry signature first, then attachment digests and subject claims, then any kind-specific signature, DSSE/in-toto, SPDX, or CycloneDX policy.

Code signing and notarization are payload properties. Zed must not rewrite signed executable bytes after hashing. macOS notarization, Windows Authenticode, and Linux package signatures can be recorded as attestations without replacing Zed's archive integrity checks.

## Compatibility

- Existing source `tar.gz` packages are unchanged.
- Existing ZIP extraction remains magic-byte based and can consume v1 binary archives.
- Older clients that do not understand `.zpkg-binary.json` can still see `.zpkg.toml`, but registries should advertise a binary-artifact capability so old resolvers do not select a platform-specific artifact accidentally.
- During the transition, the deployed legacy version route explicitly supports one self-describing binary ZIP per release. It remains fail-closed and one-artifact-only: it never selects among targets, never encodes target data in SemVer, and rejects different immutable bytes for an existing release. Qualified publication begins only when artifact-variant persistence and routes are deployed. Older clients must preserve an unsupported newer lock byte-for-byte or refuse to write it.
- Nix/Flox/Devbox/mise/asdf integrations consume the exact archive SHA-256 and selected target; Nix may additionally record its NAR hash without replacing the Zed digest.
