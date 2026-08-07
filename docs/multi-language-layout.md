# Multi-language layout

`zed-interfaces` used to be a single Rust crate at the repository root. It is
now one repository holding three language slices of the same contract, each
published as its own zed-package:

```
zed-interfaces/
  schemas/                 JSON Schema — the cross-language source of truth
    index.json             which schemas are front-end-facing (see below)
  codegen/generate.mjs     schemas/ -> src/dart, src/ts
  src/
    rust/                  the hand-written crate (Cargo.toml lives here)
      lib.rs, manifest.rs, lockfile.rs, …
      tests/, examples/
    dart/                  generated Dart package (pubspec.yaml, lib/*.dart)
    ts/                    generated TypeScript package (package.json, *.ts)
  Cargo.toml               virtual workspace, members = ["src/rust"]
  .zpkg.toml               one package section, four targets
```

## Direction of truth

```
src/rust/*.rs
    │  cargo run --locked --example generate_schemas
    ▼
schemas/*.json
    │  node codegen/generate.mjs   (front-end-facing subset only)
    ▼
src/dart/lib/*.dart      src/ts/*.ts
```

Rust is written by hand. Everything downstream is generated and must never be
hand-edited — CI fails on drift in both hops (`schemas/` via `git diff`, the
slices via `node codegen/generate.mjs --check`).

Regenerate both hops with:

```sh
cargo run --locked --example generate_schemas
npm run codegen
```

## Which schemas become Dart and TypeScript

Not all of them, and that is the point. `schemas/index.json` classifies every
file; the generator refuses to run when a schema is missing from it, so a new
type cannot be silently skipped or silently shipped.

The rule: **a schema is front-end-facing if and only if a browser or Flutter
client decodes it directly off the registry HTTP API or the sync stream.**

* Front-end (`"targets": ["dart", "ts"]`) — the registry read/write DTOs
  (`package-metadata`, `version-metadata`, search, publish, yank, org claim,
  audit) and the sync envelope and its policy enums.
* Rust-only (`"targets": []`) — the toolchain and on-disk formats: `manifest`,
  `lockfile`, environment plans and locks, the nix/oci/native adapter records,
  `publish-meta`. These are consumed by `zed-cli` and the servers. Generating
  them would push 20+ transitive types into every front-end bundle for a
  consumer that does not exist.

Flipping a schema from Rust-only to front-end-facing is a one-line change to
`index.json` plus `npm run codegen`.

## Why the crate manifest moved into `src/rust/`

`zed-interfaces` defines the polyglot manifest model, so its own `.zpkg.toml`
has to obey it. Two rules in `manifest.rs` decide the layout:

1. Every target needs an isolated source root — two targets may not share a
   `dir`, so the Rust slice cannot own `dir = "."` next to
   `[targets.repository]`.
2. A target with `dir = "."` may not carry a `[targets.*.native]` route ("the
   whole-repository target cannot publish to a native registry"), and root
   `[publish.native]` is only valid for a *single-language* package.

Publishing the crate to crates.io while shipping Dart and TypeScript as their
own packages therefore requires the crate to live in a subdirectory.
`src/rust/Cargo.toml` is that subdirectory, and the repository root became a
virtual workspace so `zed-interfaces = { git = "…" }` consumers still resolve.

`src/rust/tests/own_manifest.rs` asserts all of this against the real
`.zpkg.toml`, so the manifest cannot drift from the model it defines.

### Consumer impact

Path dependencies must point one level deeper:

```toml
# before
zed-interfaces = { path = "../zed-interfaces" }
# after
zed-interfaces = { path = "../zed-interfaces/src/rust" }
```

Git dependencies (`zed-cli`, `zed-api-server.rs`) need no change — Cargo finds
the package through the workspace members — but they will pick this up when
their pinned `rev` is bumped.

## Targets

| target       | dir        | adapter | native registry               |
| ------------ | ---------- | ------- | ----------------------------- |
| `repository` | `.`        | —       | — (whole tree, for tooling)   |
| `rust`       | `src/rust` | `rust`  | crates.io `zed-interfaces`    |
| `dart`       | `src/dart` | `dart`  | pub.dev `zed_interfaces`      |
| `typescript` | `src/ts`   | `node`  | npm `@zed-pkg/zed-interfaces` |

A consumer selects a slice (`zed install zed-pkg/zed-interfaces --target dart`)
and gets only that language's bytes.

## Formatting of generated Dart

The generated Dart is not run through `dart format`. The formatter's output
changed between Dart 3.6 and 3.7 (the "tall" style), so formatting at
generation time would make the committed slice depend on whichever SDK the
author had installed, and the CI drift check would flip for reasons unrelated
to the contract. The generator emits stable, already-readable Dart instead, and
CI runs `dart analyze --fatal-infos` for correctness. Do not add
`dart format --set-exit-if-changed` to CI.

## Implementations live in `zed-lib`

This repository is types and validation. Behavior that composes those types —
resolution, planning, policy — belongs in
[`zed-lib`](https://github.com/zed-pkg/zed-lib), which depends on this package.
Existing implementation bodies here (`version.rs`, `excludes.rs`,
`language.rs`, …) stay put for now and move one module at a time; see the
migration tickets in Linear (`github.com/zed-pkg`).
