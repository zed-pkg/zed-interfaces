# Zed binary artifacts v1

Status: implementation draft for `zed-interfaces` and `zed-cli`.

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

`.zpkg.toml` is the same package manifest used for source packages. It is a sibling of the binary payload at the `pkg/` root and its `[bin]` table is authoritative for command names and paths:

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

## ZIP profile

Binary v1 accepts ZIP only.

Required rules:

1. Every entry is beneath `pkg/`.
2. Exactly one `pkg/.zpkg.toml` and one `pkg/.zpkg-binary.json` exist.
3. All payload entries are regular files or directories. Symlinks, hard links, devices, sockets, and FIFOs are rejected.
4. Paths are UTF-8, relative, slash-separated, and contain no empty, `.`, `..`, drive-prefix, backslash, NUL, or control component.
5. Duplicate names and portable case-fold collisions are rejected before extraction.
6. The descriptor's file paths are relative to `pkg/`; it lists every regular file except itself and no extras are allowed.
7. Every `[bin]` path exists, is listed in the descriptor, and has `executable: true`.
8. ZIP timestamps and entry order are deterministic. POSIX executable modes are preserved, while descriptor executable intent covers Windows-produced ZIPs.
9. Only supported compression methods are accepted. Encrypted entries and self-extracting prefixes are rejected.
10. The verifier enforces archive-byte, expanded-byte, file-count, path-length, and compression-ratio limits before promotion.

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

## Signing and provenance

The inner descriptor is integrity metadata, not an independent trust root. Registry signatures or an in-toto/DSSE statement should cover:

- registry identity;
- release identity;
- target/platform and format;
- archive SHA-256 and byte size;
- descriptor SHA-256;
- source repository, tag, and commit;
- builder identity and workflow reference when available;
- SBOM/provenance attachment digests.

Code signing and notarization are payload properties. Zed must not rewrite signed executable bytes after hashing. macOS notarization, Windows Authenticode, and Linux package signatures can be recorded as attestations without replacing Zed's archive integrity checks.

## Compatibility

- Existing source `tar.gz` packages are unchanged.
- Existing ZIP extraction remains magic-byte based and can consume v1 binary archives.
- Older clients that do not understand `.zpkg-binary.json` can still see `.zpkg.toml`, but registries should advertise a binary-artifact capability so old resolvers do not select a platform-specific artifact accidentally.
- Nix/Flox/Devbox/mise/asdf integrations consume the exact archive SHA-256 and selected target; Nix may additionally record its NAR hash without replacing the Zed digest.
