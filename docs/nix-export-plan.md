# `zed.nix-export-plan/v1`

`NixExportPlan` is the execution-independent wire contract between Zed package
planning, standalone-flake generation, realization, provenance recording, and
the non-Rust clients generated from `schemas/nix-export-plan.json`.

The plan contains only public, immutable evidence:

- Zed organization, package, version, and optional polyglot target;
- package class (`data` or `prebuilt-bin`);
- resolved artifact-only Nix attribute, systems, and outputs;
- exact immutable artifact format, SHA-256, byte size, and canonical filename;
- SHA-256 of exact `.zpkg.toml` and `.zpkg.lock` bytes;
- sorted executable name/path mappings;
- an explicitly empty v1 dependency graph; and
- strict policy evidence.

It has no fields for a registry URL, token, authentication endpoint, Supabase
key, workspace path, output path, temporary directory, username, hostname,
timestamp, cache key, command, or mutable version requirement. Unknown JSON
fields fail deserialization.

## Canonicalization

`NixExportPlan::canonical_json_bytes` clones and normalizes the plan before
validation and compact JSON serialization. It sorts systems, outputs, and the
reserved dependency vector. Maps use `BTreeMap`, so executable ordering is
stable.

Validation requires:

- schema exactly `zed.nix-export-plan/v1`;
- a valid public Zed package identity;
- artifact-only export mode;
- explicit valid systems and outputs;
- canonical artifact filename `<org>-<name>-<version>.<format>`;
- lowercase 64-character SHA-256 values;
- a safe artifact-relative path for every executable;
- class/inventory agreement (`data` has no bins, `prebuilt-bin` has bins);
- no dependency edges in strict v1; and
- publishable `strict-v1` policy evidence.

## Versioning boundary

Adding optional evidence within the same semantics may be considered for a
minor-compatible reader update, but changing package classes, dependency
semantics, source-build behavior, path rules, policy requirements, or execution
meaning requires a new major schema such as `zed.nix-export-plan/v2`. Unknown
major versions fail closed.

The plan is not a completed `zed.nix-adapter/v1` record. A completed adapter
record additionally binds a generated standalone-flake inventory and realized
Nix output evidence. Planning must not fabricate those later-stage values.
