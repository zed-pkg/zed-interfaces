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

When both `Accept` and `format=` are present and disagree, the server returns `406 Not Acceptable`. Download filenames are server-generated from validated package coordinates or the resolution digest; caller-supplied path separators are never reflected.

## Canonicalization and identity

Every document carries:

```json
{
  "schema": "zpkg/dependency-graph/v1",
  "graph_digest": "sha256:<64 lowercase hex>"
}
```

The semantic `graph_digest` is SHA-256 over canonical JSON of the normalized typed document with `graph_digest` omitted. Object keys are lexicographically ordered, set-like arrays use their normative order, exact duplicates are removed, optional absent members are omitted, and floating-point numbers are forbidden by the v1 model.

The `x-zpkg-graph-digest` response header carries that semantic identity across JSON, YAML, and TOML. A strong HTTP `ETag` hashes the actual encoded response bytes and is therefore representation-specific. A JSON ETag must not be reused for YAML or TOML.

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

## Projections and limits

Target, feature, dependency-kind, and depth filtering produces a new explicit projection. A projected graph carries:

- `completeness = "projected"`;
- `parent_graph_digest`;
- a non-empty canonical projection specification;
- its own `graph_digest`.

A complete graph carries neither parent nor projection metadata. Server safety limits return an explicit error; they never silently truncate a document while claiming completeness.

## Privacy and cache behavior

Public immutable resolution artifacts may be stored in R2/S3 and served through static or edge caches. Private resolution artifacts are authorization-gated, non-enumerable, and never publicly cacheable. A denied response must not expose private package names, node/edge counts, registry paths, object keys, signed URLs, or credential material.

There is no partial-redaction mode in v1. Returning a graph with hidden nodes would change semantics and can leak topology. The caller either receives the authorized complete/projected graph or an indistinguishable denial.

Historical backfill is allowed only when the original lock and immutable registry checkpoint provenance are available. Old manifests must not be resolved against today's index and presented as historical truth.

## Validation gates

The Rust contract provides:

- structural validation for declared and resolved views;
- exact-node and edge-reference validation;
- complete/projected metadata invariants;
- canonical lowercase `sha256:` validation;
- deterministic normalization and semantic digest verification;
- round-trip, ordering, missing-edge, and projection-negative tests.

The first certification runs in `zed-pkg-test/zed-interfaces`. Production promotion to `zed-pkg/zed-interfaces` should pin the certified test commit and retain the same schema bytes.
