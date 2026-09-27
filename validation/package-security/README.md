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
- Rebuild evidence binds the rebuilt artifact, declared file manifest, and dependency-lock identity to the same immutable artifact identity.
- Intake decisions bind exact artifact, source, assessment, rebuild, and policy digests. A stale assessment cannot authorize a different artifact.
- Approval receipts bind exact artifact, source, policy, risk, and rebuild receipt digests. Missing bindings fail admission.
- Raw credentials, secrets, bearer tokens, private source bodies, arbitrary package payloads, and unbounded model explanations are not portable evidence fields. The schemas are closed so unexpected payload material is rejected.
- `publishable` is a policy outcome, not proof that publication occurred. Runtime state transitions and deployment policy live in the infrastructure/service layers.

## Repository boundary

These contracts are portable public validation shapes. They do not contain registry credentials, network policy, deployment topology, database connection details, or executable ORM models.

`zed-pkg/zed-infra` owns the operational quarantine/build/rebuild/promotion policy. `zed-pkg/zed-lib-core` owns implementation behavior. Private persistence belongs in `zed-pkg/zed-orm-core`.

Changes to this directory must preserve independently authored TypeSpec and Draft 2020-12 JSON Schema authorities and pass the dedicated TJSV peer-authority workflow before downstream generation or service adoption.
