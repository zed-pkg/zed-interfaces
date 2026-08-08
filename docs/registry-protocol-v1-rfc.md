# RFC (draft): Zed registry protocol v1 — identity, sparse index, signed checkpoints, canonical archives

Status: **draft for review** · Linear: DEN-2854 (project *zpkg Self-Hosted
Registries and Private Deps*) · anchor: zed-interfaces#45
Evidence fixture: `zed-pkg-test/zed-pkg-e2e` PR #115 (`static-registry-protocol/`).

This RFC defines the wire contract that lets one client implementation resolve
packages from the global registry (zpkg.net), from self-hosted daemons
(`zpkg-registryd`), and from **dumb static hosting** (R2/S3/nginx/GitHub
Pages) interchangeably. It is deliberately transport-lean: the load-bearing
constraint is that **the public read path is plain files**. Only publish,
yank/lifecycle, and private-package authorization require an active server.

Relationship to existing surfaces:

- The current `zed-api-server` HTTP registry and the client's `Registry`
  abstraction (`file://` directory registries + HTTPS) stay as-is; this
  protocol is the convergence target both serve. The API server MAY continue
  to expose richer endpoints (search, audit) that static profiles omit —
  capability presence is discovered, never assumed.
- `docs/native-registry-contract.md` governs publication of Zed targets to
  *external* managers (npm/Cargo). This RFC reuses its idioms: strict SemVer
  (build metadata rejected), lowercase SHA-256 digests, canonical bytes for
  signing, fail-closed validation.

## 1. Registry identity

- A registry is identified by `registry_id`, defined as the lowercase hex
  SHA-256 of its **trust-root public key** (Ed25519, 32 bytes). Not by URL.
- URLs, aliases, mirrors, and client config names are mutable transport
  details; `registry_id` is not. Lockfiles pin `(registry_id, cksum)` per
  resolved package, so repointing an alias or mirror at a different registry
  cannot silently change resolution (adversarial case in DEN-2861).
- Trust-root custody, online/offline key separation, rotation, and compromise
  recovery are operational policy (DEN-2857 guide); the protocol only requires
  that rotations be announced via a checkpoint chained to the previous key
  (open question 5).

## 2. Discovery

`GET {base}/.well-known/zpkg-registry.json`:

```json
{
  "schema_version": 1,
  "registry_id": "<64 hex>",
  "endpoints": { "index": "/index", "pkgs": "/pkgs", "checkpoint": "/checkpoint.json" },
  "auth_modes": ["none"],
  "publish_supported": false,
  "capabilities": []
}
```

- `auth_modes`: `none` | `token` | `oidc` (any combination).
- `publish_supported: false` is definitive for static exports.
- Unknown fields MUST be ignored (additive evolution; see §8).
- Empirical notes baked into requirements: dot-prefixed paths serve correctly
  on R2 public domains; freshly provisioned public domains have a propagation
  window of transient 403s, so provisioning tooling MUST verify-with-retry
  before declaring a registry live.

## 3. Sparse index

`GET {base}/index/{org}/{name}` → NDJSON, one JSON object per line, ascending
strict-SemVer order, unique versions:

```json
{"version":"1.1.0","deps":[{"name":"org/name","req":"^1.0","registry":null}],
 "cksum":"sha256:<64 hex lowercase>","size":1234,"yanked":false}
```

- **Org normalization is a MUST for exporters, not only clients**: the org
  segment is the publisher's lowercase canonical form; producers MUST fail
  builds on non-canonical orgs (the wrong-org-segment class dies at export
  time — enforced in the evidence fixture's generator).
- `deps[].registry`: `null`/absent = same registry; a value names a client
  config alias only inside private/self-hosted graphs. Packages published to
  the **global** registry MUST have all deps resolvable from the global
  registry (no dependency confusion via leaked private names).
- `yanked: true` versions MUST be excluded from resolution while their
  archives remain fetchable (locked consumers keep working). Lifecycle states
  beyond yank (security-revoke, legal tombstone) are v1.1 material (open
  question 4).
- SemVer build metadata is rejected, matching the native-registry contract.

## 4. Canonical archives

`GET {base}/pkgs/{org}/{name}/{version}.tar.zst`

- Content-addressed: the index line's `cksum`/`size` are authoritative;
  clients MUST verify before unpacking and MUST enforce a hard size ceiling
  independent of the advisory `size`.
- Published bytes for a version are immutable; changed bytes for an existing
  version are a protocol violation (registries MUST reject; clients MUST fail
  closed on mismatch).
- Canonical production recipe (normative for exporters that claim
  reproducibility, proven byte-stable in the fixture): ustar format, entries
  sorted by path, `mtime`/uid/gid zeroed from `SOURCE_DATE_EPOCH`, fixed
  0644 mode, empty uname/gname, then zstd at a pinned level.
- Cache semantics (verified to survive verbatim through R2 object metadata):
  `pkgs/**` → `Cache-Control: public, max-age=31536000, immutable`;
  index/discovery/checkpoint → short TTL + `stale-while-revalidate`.

## 5. Checkpoints

`GET {base}/checkpoint.json`:

```json
{"schema_version":1,"registry_id":"<64 hex>","seq":42,
 "files":[{"path":"index/org/name","sha256":"<64 hex>","size":210}],
 "tree_sha256":"<64 hex>","signature":"ed25519:<base64>"}
```

- `seq` is strictly monotonic per registry. `files` lists every index,
  discovery, and package object; `tree_sha256` = SHA-256 over the canonical
  concatenation of `"{path} {sha256}\n"` sorted by path.
- `signature` is Ed25519 over the canonical JSON bytes of the checkpoint with
  the `signature` field null (canonicalization per the existing
  `canonical_json_bytes()` discipline: validate, then deterministic ordering).
- **Atomic publish rule**: writers upload objects first, indexes second,
  checkpoint last. Readers that pin a checkpoint see either the previous
  complete tree or the next complete tree, never a mix — this is the
  static-profile substitute for transactions (AC in DEN-2857).
- **Freshness/replay**: clients MUST reject a checkpoint whose `seq` is lower
  than one previously observed for the same `registry_id` (rollback), and MAY
  enforce a staleness bound (policy-configurable) to detect freeze attacks
  (adversarial cases in DEN-2861).

## 6. Authorization profiles

- `none`: everything above is public objects. This is the whole story for
  public static hosting.
- `token`: opaque bearer tokens, env-only on clients
  (`ZPKG_TOKEN_<REGISTRY>`); scopes `read` / `publish` / `admin`.
- `oidc`: interactive web + device-authorization flows against an issuer
  (shared-auth for zpkg.net), exchanged server-side for opaque scoped registry
  tokens (DEN-2859).
- Private packages: unauthorized index or archive requests MUST return **404,
  not 403** (no existence leak — AC in DEN-2858). Private read paths are
  necessarily served by a daemon; large downloads SHOULD redirect to
  short-lived signed object URLs. Signed-URL responses MUST carry the same
  cache-control contract as direct streams (defect class: zed-api-server.rs
  #18; sibling #17 covers idempotent same-bytes republish under object-store
  write races).

## 7. Client requirements

- Send an identifying `User-Agent` (`zed/<version>`); language-default agents
  are empirically blocked by real static hosts (r2.dev 403s `Python-urllib`).
- Verify archive digests always; verify checkpoint signatures when the
  registry advertises one; treat signature/digest mismatch and version-byte
  drift as hard failures, never warnings.
- Pin `(registry_id, cksum)` in lockfiles; re-resolution across mirrors or
  alias changes MUST NOT change locked origins.
- Publish-time: enforce the global-deps rule (§3) before upload.

## 8. Versioning and evolution

- `schema_version` gates both discovery and checkpoint. Evolution is
  additive-only within a major; unknown fields are ignored; removals or
  semantic changes bump the major and MUST be dual-published during
  migration windows.
- Static profile conformance = the fixture's checker plus its mutation red
  tests (tamper archive/index, drop checkpoint entry, resurrect yanked, wrong
  schema — each MUST fail a conforming verifier).

## 9. Open questions for review

1. Index transport: per-package NDJSON files (this draft) vs a periodically
   published single-file snapshot (faster cold resolve, worse cacheability) —
   or both, with the snapshot as an optional capability?
2. Mirrors: strict `replace-with` only (this draft) vs transparent
   proxy-with-fallthrough (rejected here as a dependency-confusion vector) —
   confirm.
3. Global org verification: GitHub-org ownership proof vs first-come +
   dispute process (policy issue DEN-2860, but the RFC must reserve fields).
4. Lifecycle beyond yank: `security_revoked` and legal tombstone (bytes
   removed, version burned) — representation in index lines vs a separate
   lifecycle file.
5. Trust-root rotation: chained checkpoints vs out-of-band re-enrollment;
   offline-root/online-signing split.
6. Should `registry_id` bind to the trust root (this draft) or to a separate
   registry certificate so roots can rotate without identity change?

## Appendix A — evidence

The static profile of §§2–5 (unsigned checkpoint) runs today:
`zed-pkg-test/zed-pkg-e2e` PR #115 — deterministic generator, conformance
checker, five mutation red tests, and a full PASS served from R2 public
hosting (`zed-pkg-static-registry-e2e` bucket) with the §4 cache headers
verified in responses. Operational findings from that run are folded into
§§2, 4, 7.
