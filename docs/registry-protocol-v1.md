# Registry protocol v1 RFC

Status: proposed  
Tracking: Linear DEN-2854; GitHub issue #45  
Static-profile evidence: `zed-pkg-test/zed-pkg-e2e#115`

## Purpose

Registry protocol v1 gives zed-pkg one explicit trust and wire contract for:

- the canonical public registry at `registry.zpkg.net`;
- private dependencies on the canonical registry;
- named self-hosted registries;
- public static exports served from a filesystem, object storage, or CDN; and
- approved mirrors that preserve an upstream registry's identity, signatures, and package digests.

The load-bearing property is that the complete **public read path is ordinary immutable files**. A server is required for publishing, lifecycle mutations, organization administration, private-package authorization, search, and audit APIs, but not for resolving public packages.

This RFC is contract-only. It performs no network, credential, database, object-storage, DNS, or deployment operation.

## Decision summary

1. A local alias such as `corp` is configuration, not trust identity.
2. A URL is a transport location, not sufficient package provenance.
3. `registry_id` is immutable and independent of rotatable signing keys.
4. A stable signed checkpoint selects an **immutable index snapshot**. Mutable `/index/**` paths are not a conforming static publication scheme.
5. Package version bytes are immutable. Yank, security revocation, and legal tombstone change lifecycle metadata, never the bytes bound to the version identity.
6. Per-package NDJSON remains the authoritative sparse index; an optional SQLite snapshot may only be a derivative accelerator.
7. Transparent cross-registry fallback is rejected. Mirrors and source replacement are explicit.
8. Human identity and package-registry authorization use separate audiences and credentials.
9. Unsupported breaking schema versions, metadata rollback, expired metadata outside policy, identity changes, and digest mismatches fail closed.

## Registry identity and trust enrollment

Every registry generates one cryptographically random immutable identifier:

```text
zpkg-registry:<32 lowercase hexadecimal characters>
```

The 128-bit value is generated once with a CSPRNG and backed up with the registry's recovery material. It is deliberately **not** the hash of an online or offline signing key. Binding identity directly to a key would either change package origin during routine key rotation or require treating a compromised key as permanent identity. Trust enrollment instead pins the tuple:

```text
(registry_id, canonical origin, accepted recovery/root key fingerprints)
```

A controlled key rotation preserves `registry_id`, is authorized by the enrolled recovery/root policy, and is announced through signed metadata. Compromise recovery may require an explicit out-of-band re-enrollment ceremony, but never silently changes a locked package's registry identity.

A lockfile records at minimum:

- immutable registry identity;
- canonical package coordinate and version;
- archive and archive-manifest digests;
- accepted checkpoint identity/sequence according to lock policy; and
- explicit dependency registry identities.

Repointing a local alias or mirror URL to another `registry_id` is a trust change that requires explicit approval. An alias is never sufficient lock provenance.

## Discovery

`GET /.well-known/zpkg-registry.json` returns `RegistryDiscoveryV1`, including:

- exact discovery schema and supported protocol versions;
- immutable `registry_id` and canonical HTTPS URL;
- relative endpoint templates;
- additive read, publish, yank, private-package, static-export, and mirror capabilities;
- supported authentication modes and OIDC issuer/audience metadata where applicable;
- metadata-signing keys and rotation state; and
- archive, expanded-size, file-count, path-length, and compression-ratio limits.

The required static-read templates are conceptually:

```json
{
  "sparse_index_template": "/snapshots/{snapshot}/index/{org}/{name}",
  "snapshot_manifest_template": "/snapshots/{snapshot}/manifest.json",
  "package_template": "/pkgs/{org}/{name}/{version}.tar.zst",
  "checkpoint": "/checkpoint.json"
}
```

`{snapshot}` is the lower-case SHA-256 value in `RegistryCheckpointV1.index_root_sha256`. Endpoint templates are relative paths, contain no credentials/query/fragment, cannot escape the registry origin, and must contain every required placeholder.

The canonical URL excludes credentials, query strings, fragments, and a trailing slash. The client records the trust tuple only after an explicit user action or administrator-pinned policy.

## Immutable sparse-index snapshots

### Per-package NDJSON

The authoritative v1 index is one NDJSON stream per package under the immutable snapshot selected by the checkpoint:

```http
GET /snapshots/{snapshot}/index/{org}/{name}
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
- the checkpoint sequence that covers the record.

Producers enforce canonical organization/package segments before publication. The wrong-organization-segment defect class must fail during export, not be left for a client to infer.

Per-package NDJSON remains authoritative because it is inspectable, incrementally fetchable, cacheable by immutable URL, and servable without an application process. A future SQLite snapshot may accelerate cold resolution, but it is derivative and must verify against the same signed checkpoint and immutable snapshot manifest.

### Snapshot manifest

`GET /snapshots/{snapshot}/manifest.json` returns `RegistryIndexSnapshotV1`:

- exact schema and registry identity;
- checkpoint sequence;
- a strictly path-sorted, duplicate-free list of `index/<org>/<name>` objects; and
- each object's SHA-256 and byte size.

The canonical JSON bytes of this manifest hash to `{snapshot}` and therefore to `RegistryCheckpointV1.index_root_sha256`.

A client resolving one package:

1. verifies the signed checkpoint and monotonic/freshness policy;
2. fetches the snapshot manifest selected by `index_root_sha256`;
3. verifies the manifest byte digest equals `index_root_sha256`;
4. finds `index/<org>/<name>` in the manifest;
5. fetches that immutable NDJSON object through the discovery template;
6. verifies its byte size and SHA-256; and
7. validates each record, including registry identity and checkpoint sequence.

An absent manifest entry is the authoritative package-not-found result for that snapshot. A static host may also return 404 for the absent object, but the signed manifest is the trust decision.

## Signed freshness checkpoints

`GET /checkpoint.json` returns `RegistryCheckpointV1`:

- monotonically increasing sequence;
- generation and expiry timestamps;
- `index_root_sha256`, which selects and authenticates the immutable index snapshot;
- previous signed-checkpoint digest for every sequence after 1;
- signing-key ID; and
- Ed25519 signature over canonical payload bytes excluding the signature field.

Clients persist the highest accepted sequence and checkpoint digest for each registry identity. They reject:

- a lower sequence than previously accepted;
- the same sequence with different signed content;
- a broken previous-checkpoint link;
- an expired checkpoint outside the configured offline grace policy;
- a signing key not accepted through the enrolled root/recovery policy; or
- metadata whose registry identity differs from the configured/locked trust record.

### Atomic static publication

Object stores provide atomicity per object, not across a mutable collection. Merely uploading mutable `/index/**` objects and then updating `/checkpoint.json` does **not** guarantee a reader holding the previous checkpoint can still fetch the previous index.

A conforming writer therefore publishes in this order:

1. construct canonical archives and immutable package objects;
2. construct every per-package NDJSON index for the new state;
3. construct the canonical `RegistryIndexSnapshotV1` manifest;
4. derive `snapshot = sha256(canonical snapshot-manifest bytes)`;
5. upload all index objects and the manifest under the new immutable `snapshots/{snapshot}/` prefix;
6. verify object size/digest/metadata through the direct object API;
7. upload any new immutable package objects;
8. publish the newly signed stable `/checkpoint.json` **last**; and
9. verify the stable checkpoint directly, then through any custom-domain/CDN path.

Objects under an existing snapshot prefix are never overwritten or deleted while retained checkpoints or supported lockfiles can reference them. An old checkpoint always addresses the old immutable prefix; a new checkpoint addresses the new prefix. Readers therefore see a complete previous or complete next snapshot rather than a mixed mutable index.

The stable checkpoint should use `Cache-Control: no-cache` or an equivalently reviewed, tightly bounded revalidation policy. Discovery also requires revalidation because capabilities and accepted keys can change. Immutable snapshot objects and immutable package bytes may use long-lived caching, for example:

```text
Cache-Control: public, max-age=31536000, immutable
```

Checkpoint expiry and monotonic sequence remain the security freshness mechanism; CDN purge is only convergence acceleration.

## Minimum signing roles

- An offline/recovery root controls accepted metadata-signing keys, registry-identity recovery policy, and key replacement.
- An online Ed25519 metadata key signs checkpoints.
- Publication credentials cannot act as recovery/root or metadata-signing keys.
- Key rotation is authorized by the enrolled recovery/root policy and recorded in auditable metadata while preserving `registry_id`.

Full TUF delegation and target roles are deferred. The v1 checkpoint chain and trust enrollment still provide expiry, rollback/freeze detection, key rotation, and documented compromise recovery.

## Canonical package coordinates

Organization, package, target, and feature tokens use lowercase ASCII letters, digits, hyphen, underscore, and period. They begin with a lowercase letter and end with an alphanumeric character. Producers, servers, and clients enforce the same contract before network resolution.

A global organization claim requires verifiable control, such as a GitHub organization administrator flow or DNS challenge, plus reserved/confusable-name and dispute/transfer/recovery policy. First-come-only ownership is insufficient for the canonical registry.

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

Canonical exporters use sorted entries, normalized uid/gid/uname/gname, `SOURCE_DATE_EPOCH`, an explicit portable mode policy, and a pinned compression implementation/level. Executability is taken from reviewed package metadata, not accidental host filesystem mode. The publisher creates canonical bytes; the server independently validates the archive and recomputes archive/manifest digests before accepting it.

## Publication and lifecycle

```http
PUT /api/v1/packages/{org}/{name}/{version}
```

`RegistryPublishRequestV1` is idempotent only when the already accepted archive and manifest digests are byte-identical. Concurrent same-bytes publication may converge successfully; the same coordinate/version with different bytes returns `immutable-version-conflict`.

Lifecycle states:

- `active`: eligible for new resolution;
- `yanked`: excluded from new resolution by default; immutable bytes remain available for locked installs;
- `security-revoked`: clients warn or block according to policy while the version remains permanently named; and
- `legal-tombstoned`: bytes are unavailable and locked installs receive an explicit permanent diagnostic such as HTTP 410/451.

Every non-active state includes a nonempty reason/category safe to disclose. The coordinate/version remains permanently burned and cannot be republished with different bytes.

## Multiple registries and mirrors

A manifest may name a configured registry alias, but the lock records the resolved registry identity, canonical origin, package digest, manifest digest, and checkpoint identity.

V1 supports strict source replacement only when explicitly configured. It does not try another registry when a package is absent. Transparent fallback is rejected because it creates dependency-confusion and trust ambiguity.

An approved mirror may change the transport endpoint while preserving:

- upstream `registry_id` and enrolled key chain;
- upstream signed checkpoint and snapshot manifest;
- package coordinate/version; and
- exact index, archive, and archive-manifest digests.

A separately operated registry has a distinct `registry_id` even when it imports identical bytes. Mirror freshness and highest accepted upstream checkpoint are observable.

## Authentication and private packages

Public static reads may advertise `anonymous-read`. Other supported modes include static scoped tokens, OIDC authorization-code/PKCE, device authorization for humans, and workload identity for CI.

For the canonical service:

- shared-auth performs web signup/login and human identity ceremonies;
- the registry validates/exchanges that assertion and issues a registry-audience, scoped, expiring, revocable credential;
- the CLI stores human refresh material through an OS keychain or credential helper;
- unattended CI never waits on device authorization and uses short-lived pre-provisioned or workload-identity credentials; and
- tokens never enter manifests, lockfiles, command-line flags, URLs, ordinary config, diagnostics, or crash logs.

For package/index/archive reads, an unauthenticated or unauthorized private coordinate returns 404 so package existence is not disclosed. Authentication endpoints may still return standards-compliant 401 responses, and authenticated administration attempts may return 403 where existence is already known.

Private index/API responses use `Cache-Control: private, no-store`. Large private package downloads may use short-lived object-storage URLs bound to an already authorized immutable object. Redirects and object responses preserve the private cache contract, and signed URLs plus authorization headers are redacted from logs and analytics.

A public package on the canonical registry cannot depend on private packages or on packages that exist only in an unrelated self-hosted registry. A private global package may depend on authorized public/private packages in the same global registry. Self-hosted packages may declare explicit cross-registry dependencies; each origin remains pinned.

## Self-hosted profiles

The protocol supports these deployment profiles without changing client trust semantics:

1. **Static public export** — revalidated discovery/checkpoint, immutable snapshot manifests/indexes, and immutable archives on a filesystem, S3-compatible bucket, Cloudflare R2, nginx, or CDN.
2. **Small writable registry** — `zpkg-registryd` with filesystem storage and scoped static tokens.
3. **Object-storage registry** — `zpkg-registryd` with S3/R2, conditional writes, object versioning where available, generic OIDC, and audit logs.
4. **Kubernetes registry** — Helm/manifests, external object storage, replicated stateless API, signing-key separation, metrics, backup/restore, and disaster-recovery runbooks.

Object-storage configuration is provided through secret/environment references. Account IDs, access keys, secret keys, bearer tokens, signed URLs, and production bucket names are never committed to this interface repository.

Operators document:

- registry-ID and recovery/root backup;
- online signing-key rotation/revocation;
- snapshot/checkpoint retention policy;
- object versioning and lifecycle interaction;
- checkpoint-last publication and direct-origin verification;
- backup/restore verification;
- filesystem-to-object-store migration without changing registry identity;
- audit retention; and
- garbage collection that never removes bytes still reachable by retained checkpoints or supported lockfiles.

## R2/static-host evidence and operational implications

`zed-pkg-test/zed-pkg-e2e#115` demonstrates a deterministic unsigned v0 static tree, local conformance checks, mutation red tests, and a live read from a dedicated non-production R2 bucket. The evidence established useful implementation facts:

- `r2.dev` is a rate-limited development surface and may reject generic script user agents; zed clients and conformance tools send an identifying `User-Agent` such as `zed/<version>`;
- enabling a public development domain has a propagation window, so provisioning verifies with bounded retry before declaring it ready;
- object `Cache-Control` metadata is observable through the R2 public read path; and
- deterministic `tar.zst` output requires normalized tar metadata, deterministic ordering, `SOURCE_DATE_EPOCH`, and a pinned compression level/toolchain.

These are operational findings, not reasons to weaken trust checks. Production static hosting uses a reviewed custom domain, explicit cache/WAF policy, direct-origin verification, short-lived prefix-scoped credentials, and checkpoint-last immutable snapshot publication. The v0 fixture must be upgraded to the signed v1 snapshot layout before it becomes a release gate.

## Protocol evolution

All top-level DTOs contain an exact versioned `schema` string. V1 implementations:

- reject an unknown breaking schema version;
- preserve immutable identity/digest semantics;
- tolerate additive discovery capabilities they do not use;
- ignore optional additive JSON fields only when their parser policy explicitly permits them; and
- never reinterpret an existing field.

The Rust DTOs currently use `deny_unknown_fields` so the initial implementation fails closed while the extension mechanism is finalized. A later RFC may introduce an explicit `extensions` map rather than silently weakening validation.

## Conformance suite

The interface contract tests and golden fixtures cover:

- discovery round-trip and active-key requirement;
- required immutable-snapshot endpoint placeholders;
- alias/URL substitution rejection;
- canonical coordinate and SemVer validation;
- snapshot-manifest path ordering, digest/size shape, and registry/sequence binding;
- sparse index records with explicit dependency origins;
- lifecycle reason requirements;
- checkpoint sequence/link/signature shape;
- archive traversal, unsafe symlink, order, digest, and size failures;
- changed bytes for the same version; and
- deterministic canonical JSON used by signing/lock provenance.

Cross-repository E2E additionally exercises:

- checkpoint-last publication while readers race the transition;
- old-checkpoint/old-snapshot and new-checkpoint/new-snapshot consistency;
- rollback/freeze persistence and key rotation/recovery;
- token revocation and private-cache isolation;
- legal tombstones and locked-package diagnostics;
- static object-storage serving and cache headers;
- wrong-organization export rejection;
- absent-package hard miss;
- alias-remap rejection; and
- mutation tests proving every validation class can turn red.

## Reconciliation of the parallel RFC draft

The closed doc-only PR #49 and live fixture PR #115 were reviewed rather than duplicated.

Accepted here:

- static public reads as plain files;
- producer-side canonical organization enforcement;
- explicit identifying user agent;
- deterministic archive evidence;
- private-read 404 behavior;
- checkpoint-last publication;
- global-public dependency restrictions; and
- the empirical R2 propagation/cache-header findings.

Corrected:

- uploading mutable index paths before a checkpoint is not collection-atomic; v1 uses immutable snapshot prefixes selected by the checkpoint;
- a root digest alone cannot authenticate a sparse file without a manifest/proof, so v1 adds `RegistryIndexSnapshotV1`; and
- fixed mode `0644` for every entry would destroy intentional executability, so canonical modes come from reviewed package metadata.

Rejected:

- deriving `registry_id` directly from a signing-key fingerprint, because routine/compromise key rotation must preserve registry/package identity; and
- transparent mirror fallthrough or first-come-only global organization ownership.
