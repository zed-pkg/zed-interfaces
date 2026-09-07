# Peer-authority consumer-verification canary

Tracking: DEN-3828; ORESoftware/typespec-json-schema-validator#20 and merged verifier PR #31.

The two source files are independently maintained peer authorities. Neither is generated from or preferred over the other. The pinned workflow compiles TypeSpec, compares generated Schema B with independently authored Draft 2020-12 JSON Schema, runs differential probes, and emits a receipt and Contract IR. The canonical verifier then rebuilds the expected IR from explicit current paths and requires exactly both canary declarations.

Absent, malformed, stale, tampered or incomplete evidence blocks admission; copied green fields are insufficient. Generated witness, receipt, SARIF and IR files are disposable CI artifacts under the ignored `.typespec-json-schema-validator/` directory, never editable authorities.

This scope is separate from the legacy Rust/manifest/lockfile generation pipeline and the peer-authority `validation/` pipeline. It does not certify their contracts, private persistence semantics, ORM annotations, runtime targets, or generated output. Those native gates remain unchanged, and public/server scope separation must still be proven independently.
