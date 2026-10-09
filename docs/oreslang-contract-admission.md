# Oreslang language conformance admission — draft

This repository `zed-pkg/zed-interfaces` owns or witnesses the following existing
contract roots: `contracts/`, `schemas/`, `schema-authority-canary/`, `conformance/`, `codegen/`.

**Unadmitted candidate**: do not count Oreslang among supported implementations,
change a published support matrix, or mark a conformance test as passed.

## Gates required for actual support

1. Select exact independently authored TypeSpec and JSON Schema Draft 2020-12
   authority files, record their immutable Git revisions and source closure;
   resolve peer discrepancies **semantically**, never by allowing one lane to
   overwrite the other. Keep generated files evidence-only.
2. Validate current-input parity and Contract IR through
   `ORESoftware/typespec-json-schema-validator`, not a new validator;
   use `ORESoftware/ores-contracts` additionally for persistence-bearing
   contract families, but do not invent persistence tables for value contracts.
3. Compile actual Oreslang declarations and runtime adapter on GraalVM/JVM;
   preserve package dependency graph, provenance and registry/runtime boundary semantics.
4. Run *positive and negative* language-native fixtures for ingress and egress,
   with null-vs-absent, enum/union, numeric boundary, UTF-8, unknown-field,
   and invalid-shape coverage; compare results to currently admitted languages.
5. Produce TJSV current-source-bound runtime and language-boundary evidence:
   compiler identity, exact commit SHA, SHA-256 digests, fixture input digests
   and per-case outcomes. Missing or stale evidence stops admission.
6. JS-browser and WASM-browser are *future separate runtime identities*:
   require transpiler, browser sandbox and independent conformance CI before
   support or package publication may be claimed.
7. Maintain all existing `governance/` or `contract-admission/` mirrors and
   participant matrices, changing them atomically when executable Oreslang
   tests exist; do not register an unimplemented mandatory participant.

Move this PR out of draft **only after** the working language adapter and
exact-head compiled conformance evidence have been added and reviewed.
