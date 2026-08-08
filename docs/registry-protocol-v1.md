# Registry protocol v1 RFC

Status: proposed  
Tracking: Linear DEN-2854; GitHub issue #45

## Purpose

Registry protocol v1 gives zed-pkg one explicit trust and wire contract for:

- the canonical public registry at `registry.zpkg.net`;
- private dependencies on the canonical registry;
- named self-hosted registries;
- static public mirrors/exports served from a filesystem, object storage, or CDN; and
- future proxy transports that preserve an upstream registry's identity and signatures.

This RFC is contract-only. It performs no network, credential, database, object-storage, or deployment operation.

## Design principles

1. A local alias such as `corp` is configuration, not trust identity.
2. A URL is a transport location, not sufficient package provenance.
3. Public reads remain ordinary immutable files where possible.
4. Signed, monotonic freshness metadata prevents a stale or malicious mirror from freezing a client indefinitely.
5. Package version bytes are immutable. Yank, security revocation, and legal tombstone change lifecycle metadata, never the bytes bound to a version identity.
6. Human identity and package-registry authorization are separate audiences.
7. Unknown additive fields are tolerated only within an understood schema version; an unsupported breaking protocol version fails closed.

## Registry identity and enrollment

Every registry generates one immutable ID in this form:

```text
zpkg-registry:<32 lowercase hexadecimal characters>
```

`GET /.well-known/zpkg-registry.json` returns `RegistryDiscoveryV1`, including:

- immutable `registry_id`;
- canonical HTTPS URL;
- supported protocol versions;
- relative endpoint templates;
- additive capabilities;
- authentication modes and, where applicable, OIDC issuer/audience;
- metadata-signing keys and rotation state; and
- archive/file/path/compression limits.

The client records `(registry_id, canonical_url, accepted key IDs/trust root)` after an explicit user action or administrator policy. Repointing an existing local alias to another `registry_id` is a trust change and requires explicit approval. A lockfile stores the registry identity, not merely the alias.

The canonical URL excludes credentials, query strings, fragments, and a trailing slash. Endpoint templates are relative paths and cannot escape the registry origin.

## Sparse index

The authoritative v1 index is one NDJSON stream per package:

```http
GET /index/{org}/{name}
```

Each `RegistryIndexRecordV1` binds:

- registry identity;
- canonical lowercase organization and package name;
- strict SemVer version without build metadata;
- immutable archive and archive-manifest SHA-256 values;
- exact archive size and format;
- dependencies with explicit registry identity;
- target and feature metadata;
- lifecycle state and reason; and
- the signed checkpoint sequence that covers the record.

Per-package NDJSON remains authoritative because it is inspectable, incrementally cacheable, atomically replaceable, and servable without an application process. A future SQLite snapshot may accelerate cold-start resolution, but it is derivative and must be signed/verified against the same checkpoint.

## Signed freshness checkpoints

`GET /checkpoint.json` returns `RegistryCheckpointV1`:

- monotonically increasing sequence;
- generation and expiry timestamps;
- root digest covering the current sparse-index state;
- previous-checkpoint digest for every sequence after 1;
- signing-key ID; and
- Ed25519 signature over the canonical payload excluding the signature field.

Clients persist the highest accepted sequence and checkpoint digest for each registry identity. They reject:

- a lower sequence than previously accepted;
- the same sequence with different signed content;
- a broken previous-checkpoint link;
- an expired checkpoint outside the configured offline grace policy;
- a signing key not enrolled through discovery/key rotation; or
- metadata whose registry identity differs from the lock/config trust record.

A static mirror may transport discovery, checkpoint, index, manifest, and package files, but it cannot replace the upstream `registry_id` or resign the upstream namespace. A separately operated registry uses a different identity even if it mirrors the same packages.

### Minimum v1 signing roles

- An offline/recovery root controls accepted metadata-signing keys and registry-identity recovery.
- An online Ed25519 metadata key signs checkpoints.
- Publication credentials do not act as root or metadata-signing keys.

Full TUF delegation and target roles are deferred. The v1 checkpoint chain still must provide key rotation, expiry, rollback/freeze detection, and documented recovery.

## Canonical package coordinates

Organization, package, target, and feature tokens use lowercase ASCII letters, digits, hyphen, underscore, and period. They begin with a lowercase letter and end with an alphanumeric character. Servers and clients enforce the same rule before network resolution.

A global organization claim is a policy-layer operation backed by verifiable control, such as a GitHub organization administrator flow or DNS challenge. First-come-only namespace ownership is not sufficient for the canonical registry.

## Canonical archives

Protocol v1 accepts deterministic `tar.zst` package archives. `RegistryArchiveManifestV1` lists entries in strict bytewise path order and binds the exact archive SHA-256.

Allowed entry types:

- regular file;
- directory; and
- relative symlink whose target is itself a safe relative path.

Rejected by construction or extraction validation:

- absolute paths;
- empty, `.`, or `..` components;
- backslash path separators;
- device nodes, sockets, FIFOs, and hard links;
- duplicate or noncanonically ordered paths;
- symlinks escaping the package root;
- modes outside the portable `0o000..0o777` range;
- archive/file/path/count limits above discovery policy; and
- decompression ratios above discovery policy.

The publisher creates canonical bytes; the server independently validates the archive and recomputes archive/manifest digests before accepting it.

## Publication and lifecycle

```http
PUT /api/v1/packages/{org}/{name}/{version}
```

`RegistryPublishRequestV1` is idempotent only when the already accepted archive and manifest digests are byte-identical. Reusing the same coordinate/version with different bytes returns `immutable-version-conflict`.

Lifecycle states:

- `active`: eligible for new resolution;
- `yanked`: excluded from new resolution by default; locked installs may continue;
- `security-revoked`: clients warn or block according to policy;
- `legal-tombstoned`: bytes are unavailable and locked installs receive an explicit permanent diagnostic such as HTTP 410/451.

Every non-active state includes a nonempty reason. The coordinate/version remains permanently burned and cannot be republished with different bytes.

## Multiple registries and mirrors

A manifest may name a configured registry alias, but the lock records the resolved registry identity, canonical origin, package digest, manifest digest, and checkpoint identity.

V1 supports strict source replacement only when explicitly configured. It does not try another registry when a package is absent. Transparent fallback is rejected because it creates dependency-confusion and trust ambiguity.

An approved mirror may change the transport endpoint while preserving:

- upstream `registry_id`;
- upstream signed checkpoint and index metadata;
- package coordinate/version; and
- exact archive/manifest digests.

Mirror freshness and highest upstream checkpoint are observable.

## Authentication and private packages

Public static reads may advertise `anonymous-read`. Other supported modes include static scoped tokens, OIDC authorization-code/PKCE, device authorization for humans, and workload identity for CI.

For the canonical service:

- shared-auth performs web signup/login and human identity ceremonies;
- the registry validates/exchanges that assertion and issues a registry-audience, scoped, expiring, revocable credential;
- the CLI stores human refresh material through an OS keychain or credential helper;
- unattended CI never waits on device authorization and uses short-lived pre-provisioned or workload-identity credentials; and
- tokens never enter manifests, lockfiles, command-line flags, URLs, ordinary config, diagnostics, or crash logs.

Private index/API responses use `Cache-Control: private, no-store`. Large private package downloads may use short-lived object-storage URLs bound to an already authorized object. Signed URLs and authorization headers are redacted from logs and analytics.

A public package on the canonical registry cannot depend on private packages or on packages that exist only in an unrelated self-hosted registry. Self-hosted packages may declare explicit cross-registry dependencies; each origin remains pinned.

## Self-hosted profiles

The protocol supports these deployment profiles without changing client trust semantics:

1. **Static public export** — discovery, signed checkpoint, NDJSON index, manifests, and archives on a filesystem, S3-compatible bucket, Cloudflare R2, nginx, or CDN.
2. **Small writable registry** — `zpkg-registryd` with filesystem storage and scoped static tokens.
3. **Object-storage registry** — `zpkg-registryd` with S3/R2, conditional writes, versioning where available, generic OIDC, and audit logs.
4. **Kubernetes registry** — Helm/manifests, external object storage, replicated stateless API, signing-key separation, metrics, backup/restore, and disaster-recovery runbooks.

Object-storage configuration is provided through secret/environment references. Account IDs, access keys, secret keys, bearer tokens, signed URLs, and production bucket names are never committed to this interface repository.

Operators must document:

- registry-ID and offline-root backup;
- online signing-key rotation/revocation;
- object versioning and retention;
- checkpoint/index atomic publication;
- backup/restore verification;
- filesystem-to-object-store migration without changing registry identity;
- audit retention; and
- garbage collection that never removes bytes still reachable by an active/yanked locked version.

## Protocol evolution

All top-level DTOs contain an exact versioned `schema` string. V1 implementations:

- reject an unknown breaking schema version;
- preserve immutable identity/digest semantics;
- tolerate additive discovery capabilities they do not use;
- ignore optional additive JSON fields only when their parser policy explicitly permits them; and
- never reinterpret an existing field.

The Rust DTOs currently use `deny_unknown_fields` so the initial implementation fails closed while the extension mechanism is finalized. A later RFC may introduce an explicit `extensions` map rather than silently weakening validation.

## Conformance suite

The contract tests and golden fixtures cover:

- discovery round-trip and active-key requirement;
- alias/URL substitution rejection;
- canonical coordinate and SemVer validation;
- sparse index records with explicit dependency origins;
- lifecycle reason requirements;
- checkpoint sequence/link/signature shape;
- archive traversal, unsafe symlink, order, digest, and size failures;
- changed bytes for the same version; and
- deterministic canonical JSON used by signing/lock provenance.

Cross-repository E2E must additionally exercise rollback/freeze persistence, key rotation/recovery, token revocation, private-cache isolation, legal tombstones, static object-storage serving, and alias-remap rejection before GA.
