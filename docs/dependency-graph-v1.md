# Dependency graph protocol v1

Status: implementation draft for DEN-2864 / zed-pkg/zed-interfaces#46.

This contract deliberately separates **declared requirements** from an **exact resolved graph**. An immutable package version has one declaration set, but it can resolve differently for different targets, feature selections, resolver versions, registry checkpoints, and lock decisions.

## Endpoints

```text
GET|HEAD /v1/packages/{org}/{name}/versions/{version}/dependency-graph?view=declared
GET|HEAD /v1/resolutions/{resolution_digest}/dependency-graph
```

The declared endpoint returns direct unresolved requirements from the immutable package manifest. The resolution endpoint returns a content-addressed exact graph emitted from the same resolver state that produced the lockfile. The global registry does not accept credentials for arbitrary third-party registries and does not synchronously re-resolve a caller's private cross-registry graph; `zed graph --local` owns that flow.

## Representations

Authoritative, lossless representations:

| Format | Extension | Media type |
|---|---|---|
| JSON | `.json` | `application/vnd.zpkg.dependency-graph.v1+json` |
| YAML | `.yaml` | `application/vnd.zpkg.dependency-graph.v1+yaml` |
| TOML | `.toml` | `application/vnd.zpkg.dependency-graph.v1+toml` |

DOT and Mermaid are convenience renderings only. They are not digest inputs and must not be treated as interchange authorities.

JSON is the stored semantic authority. YAML must use a JSON-compatible safe subset with no custom tags, aliases, anchors, or merge keys. TOML must use normalized arrays of nodes and edges and omit absent optional members rather than inventing sentinel values.

When both `Accept` and `format=` are present and disagree, the server returns `406 Not Acceptable`. Download filenames are server-generated from validated package coordinates or the resolution digest and delivered in a `Content-Disposition` attachment header with a fixed character set; caller-supplied path separators, control characters, quotes, and non-ASCII bytes are never reflected.

Both routes support conditional requests per representation: `GET` or `HEAD` with `If-None-Match` matching the representation's strong `ETag` returns `304 Not Modified` carrying the same `ETag`, `x-zpkg-graph-digest`, and `Cache-Control` metadata with no body. RFC 9110 requires weak comparison for `If-None-Match`, so a presented `W/"tag"` matches an emitted `"tag"`; the opaque tag still remains representation-specific, and a JSON validator never produces a `304` for a YAML or TOML request.

Public successful and `304` responses carry `Vary: Accept`. This remains necessary for immutable graphs because the same canonical route selects different representation bytes from `Accept`; a shared cache must not reuse JSON for a YAML-only request. Authorized private responses add `Authorization` to `Vary` and use `Cache-Control: private, no-store`.

## Canonicalization and identity

Every document carries:

```json
{
  "schema": "zpkg/dependency-graph/v1",
  "graph_digest": "sha256:<64 lowercase hex>"
}
```

The semantic `graph_digest` is SHA-256 over canonical JSON of the normalized typed document with `graph_digest` omitted. The canonical form is an RFC 8785 (JCS)-compatible subset: object member names are ASCII by construction and sorted bytewise ascending, insignificant whitespace is forbidden, strings use standard JSON escaping with lowercase hex `\u00xx` escapes for control characters and raw UTF-8 elsewhere, and only integer numbers are representable, so the JCS floating-point serialization rules are never exercised. The model contains no volatile timestamp members and none may join the digest preimage in v1.

### Digest participation and normative order

Every serialized member participates in the digest except `graph_digest` itself. Absent optional members are omitted entirely; explicit `null` is not a second spelling of absence and is not canonical. Struct-field comparisons below are field-by-field in the listed order, and string comparisons are bytewise over UTF-8 content; version strings therefore order bytewise, not by semver — normative order is an ordering convention only and carries no compatibility meaning.

| Collection | Sort key | Exact duplicates |
|---|---|---|
| `dependencies` (declared view) | `(registry_id, org, name, requirement, kind, optional, default_features, features, target)` | removed |
| `roots` | `(registry_id, org, name, version)` | removed |
| `nodes` | `(id, artifact_digest, features)` | **not removed** — any duplicate node `id`, exact or conflicting, is a validation error |
| `edges` | `(from, to, kind, requirement, target, optional, features)` | removed |
| every `features` list | bytewise string order | removed |
| `provenance.enabled_features` | bytewise string order | removed |
| `provenance.registry_snapshots` | `(registry_id, checkpoint_digest)` | removed — one registry with two differing checkpoints is a validation error |
| `projection.kinds` | kind order `runtime < build < development < peer < tooling` | removed |

The `x-zpkg-graph-digest` response header carries that semantic identity across JSON, YAML, and TOML. A strong HTTP `ETag` hashes the actual encoded response bytes and is therefore representation-specific. A JSON ETag must not be reused for YAML or TOML.

### Verification requirements

Lenient typed parsing does not authenticate a document: an injected unknown member survives deserialization and the digest still verifies, and an explicit `null` is silently read as absence. Consumers verifying a received canonical JSON artifact must use byte-exact verification — parse, verify `graph_digest`, re-serialize to canonical bytes, and require equality with the received bytes (`DependencyGraphDocument::parse_verified_canonical` in the Rust contract). Byte-exact verification rejects unknown members, explicit `null`, duplicate members, non-normative collection order, insignificant whitespace, and non-integer number formats. YAML and TOML representations verify by decoding to the typed model and re-deriving the canonical JSON digest; their transport integrity is the representation-specific `ETag`.

## Declared view

The declared view contains one exact root package identity and unresolved dependency records:

- immutable `registry_id`, org, package name, and root version;
- target registry identity and package coordinates;
- version requirement;
- dependency kind;
- optional/default-feature flags;
- requested features and target predicate.

A declared dependency is not forced into a fake exact node. That prevents the API from claiming one universal resolution for an exact root version.

## Resolved view

Every resolved node uses a structured identity containing `registry_id`, org, name, and exact version. Registry identity is intrinsic rather than a local alias, so changing a user's alias cannot reinterpret a lock or graph.

Resolution provenance is mandatory:

- resolver version;
- target triple/runtime target;
- enabled feature set;
- immutable registry IDs and checkpoint digests;
- lock digest.

The graph is emitted from the resolver state that produced the lock. The API must not independently re-resolve metadata and then label the result as the graph for an existing lock.

Dependency cycles are representable and valid in a resolved graph. Normalization, validation, and digesting are set-based — node identity, edge endpoint reference, and normative order — rather than traversal-based, so they terminate deterministically on cyclic input. The committed `cycle` golden fixture pins this behavior.

## Projections and limits

Target, feature, dependency-kind, and depth filtering produces a new explicit projection. A projected graph carries:

- `completeness = "projected"`;
- `parent_graph_digest`;
- a non-empty canonical projection specification;
- its own `graph_digest`.

A complete graph carries neither parent nor projection metadata. Server safety limits return an explicit error; they never silently truncate a document while claiming completeness.

Limits are advertised through discovery rather than hard-coded by clients. The v1 default advertised limits, exported as constants by the Rust contract and mirrored in the discovery document, are:

| Limit | Default |
|---|---|
| `max_nodes` | 50,000 |
| `max_edges` | 500,000 |
| `max_projection_depth` | 1,000 |
| `max_encoded_bytes` | 33,554,432 (32 MiB) |

Exceeding a graph limit returns `422` with code `graph_limit_exceeded`; an encoded representation over the byte limit returns `413` with code `graph_representation_too_large`. A limit failure is never a silently truncated document, and request processing is bounded by server-side wall-clock budgets that fail the request rather than degrade the output.

## Privacy and cache behavior

Public immutable resolution artifacts may be stored in R2/S3 and served through static or edge caches. Private resolution artifacts are authorization-gated, non-enumerable, and never publicly cacheable. A denied response must not expose private package names, node/edge counts, registry paths, object keys, signed URLs, or credential material.

There is no partial-redaction mode in v1. Returning a graph with hidden nodes would change semantics and can leak topology. The caller either receives the authorized complete/projected graph or an indistinguishable denial. The denial status is `404 Not Found` for missing, nonexistent, and unauthorized-private alike; these routes define no `401` or `403`, and denied responses are never publicly cacheable.

Historical backfill is allowed only when the original lock and immutable registry checkpoint provenance are available. Old manifests must not be resolved against today's index and presented as historical truth.

## Compatibility and schema evolution

The `schema` member is the exact string `zpkg/dependency-graph/v1`; validators reject any other value, including unknown major or minor variants. v1 admits no additive members: every serialized member participates in the digest, so a document carrying members outside this contract is not a v1 document and byte-exact verifiers reject it. Any member addition, removal, or semantic change ships as a new schema string with its own golden fixtures.

Non-verifying consumers may ignore unknown members when reading — the lenient Rust parser does — but no unknown member is authenticated by `graph_digest`, and nothing may be relayed as verified v1 content unless byte-exact verification passed.

## Validation gates

The Rust contract provides:

- structural validation for declared and resolved views;
- exact-node and edge-reference validation;
- complete/projected metadata invariants;
- canonical lowercase `sha256:` validation;
- deterministic normalization and semantic digest verification;
- byte-exact canonical verification (`parse_verified_canonical`) rejecting unknown members, explicit `null`, duplicate members, non-normative order, and insignificant whitespace;
- golden conformance vectors under `fixtures/dependency-graph-v1/golden/` — declared, diamond, duplicate-registries, cycle, optional-feature, target-predicate, and projected — with pinned canonical bytes and digests, regenerated only by `generate_schemas` and enforced byte-for-byte by unit tests on every platform;
- round-trip, ordering, missing-edge, and projection-negative tests.

The first certification runs in `zed-pkg-test/zed-interfaces`. Production promotion to `zed-pkg/zed-interfaces` should pin the certified test commit and retain the same schema bytes.
