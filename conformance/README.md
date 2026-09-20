# Conformance

`contracts/` remains the structural and wire-format authority for zed-pkg interfaces. `conformance/` is the shared, implementation-neutral behavioral boundary consumed by Zed package lifecycles, Git pre-push admission, and CI.

This repository starts in **scaffold-only** coverage. `conformance/cases/bootstrap.v1.json` is policy metadata, not a behavioral test and not evidence of Rust/Dart/TypeScript parity.

Run `node conformance/check.mjs`. Promotion to behavioral coverage requires real `ores.conformance.case/v1` cases, explicit required participants, and exact contract/corpus/spec digest-bound evidence. Runtime-specific goldens and generated evidence are never authority.
