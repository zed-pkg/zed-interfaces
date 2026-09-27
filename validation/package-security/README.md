# Package security peer authorities

`main.tsp` and `authored.schema.json` are independent peer authorities for portable package-security evidence. Neither is generated from the other.

This lane models evidence and admission receipts; it does not make a scanner or model score the publication authority.

## Trust boundary

- A `PackageRiskAssessment` is evidence. Publication remains a separate policy decision.
- `producer_kind` distinguishes deterministic scanners, heuristics, and model-derived findings. Consumers must not silently collapse them into one opaque score.
- Package identity is immutable across source, resolved revision, source digest, and built artifact digest.
- Registry package identities for npm, Cargo, Python, and Git retain the original portable artifact shape.
- Hex package identities are a distinct fail-closed variant. They additionally bind `registry_uri` and the lowercase 64-hex `outer_checksum` supplied by the Hex lock/registry metadata. A Hex assessment without that checksum is invalid rather than falling back to package name/version.
- For Hex, `outer_checksum` proves the registry's immutable package checksum; `artifact_digest` and `source_digest` remain independent Zed evidence for the exact downloaded/materialized artifact and source tree. Consumers must compare all applicable identities instead of treating the Hex checksum alone as a safety approval.
- `PackageDependencyClosure` binds a root artifact, every transitive resolved dependency identity, the exact dependency-lock digest, resolver identity/version, and a deterministic `closure_digest`. It is closure evidence, not deployment authority.
- Rebuild evidence binds the rebuilt artifact, declared file manifest, and dependency-lock identity to the same immutable artifact identity.
- Intake decisions bind exact artifact, source, assessment, rebuild, and policy digests. A stale assessment cannot authorize a different artifact.
- Approval receipts bind exact artifact, source, policy, risk, and rebuild receipt digests. Missing bindings fail admission.
- Raw credentials, secrets, bearer tokens, private source bodies, arbitrary package payloads, and unbounded model explanations are not portable evidence fields. The schemas are closed so unexpected payload material is rejected.
- `publishable` is a policy outcome, not proof that publication occurred. Runtime state transitions and deployment policy live in the infrastructure/service layers.

## Dependency closure digest

`zed-package-dependency-closure-v1` exists so consumers such as BeamScale can invalidate prior evidence when **any** transitive dependency changes, even if the root package name/version is unchanged.

Producers and consumers must recompute `closure_digest`; it is not trusted merely because it is present. The v1 digest is SHA-256 over RFC 8785 JCS serialization of the closure object **without** the `closure_digest` member. Before serialization:

1. normalize hexadecimal digests to lowercase;
2. sort `dependencies` by the tuple `(ecosystem, registry_uri-or-empty, package_name, package_version, resolved_revision, artifact_digest, source_digest, outer_checksum-or-empty)`;
3. reject duplicate tuples rather than silently deduplicating them;
4. require the root artifact to be separate from, and not repeated in, `dependencies`;
5. retain the exact `dependency_lock_digest`, `resolver_id`, and `resolver_version` in the signed/hashed payload.

A consumer must treat a closure as changed if the lock digest, resolver identity/version, root identity, any member identity, or dependency membership/order-normalized set changes. The schema caps the dependency list at 4096 entries; operational resolvers may impose tighter limits.

For Hex closures, each member carries the registry `outer_checksum` plus independent Zed artifact/source digests. A closure made only from package names and versions is insufficient for safety reuse.

## Repository boundary

These contracts are portable public validation shapes. They do not contain registry credentials, network policy, deployment topology, database connection details, or executable ORM models.

`zed-pkg/zed-infra` owns the operational quarantine/build/rebuild/promotion policy. `zed-pkg/zed-lib-core` owns implementation behavior, including closure canonicalization/digest verification. Private persistence belongs in `zed-pkg/zed-orm-core`.

Changes to this directory must preserve independently authored TypeSpec and Draft 2020-12 JSON Schema authorities and pass the dedicated TJSV peer-authority workflow before downstream generation or service adoption.
