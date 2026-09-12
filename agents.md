# Agent instructions

## Scope and hierarchy

- Follow the canonical policy in `ORESoftware/my-ai/AGENTS.md`; these repository rules narrow that policy for `zed-pkg/zed-interfaces`.
- These instructions apply to the whole repository unless a deeper lowercase `agents.md` adds narrower rules.
- Before editing, resolve the current working directory and load every readable ancestor `agents.md` from the filesystem root to the working directory. Do not search siblings. Resolve symlinks, deduplicate resolved files, and report unreadable or cyclic instruction files.
- `.claude/CLAUDE.md`, `.gemini/GEMINI.md`, and `.openai/AGENTS.md` are pointers only. Never duplicate instructions in tool-specific files.

## Repository role

This repository is the shared contract boundary for Zed manifests, lockfiles, registry models, language targets, paths, version requirements, synchronization, VCS metadata, public validation, and private server persistence shapes. Changes here affect the CLI, API server, web server, sync engine, ORM core, lambdas, and generated clients.

Implementations belong in `zed-pkg/zed-lib-core`; executable database entities and repositories belong only in private `zed-pkg/zed-orm-core`. Keep this repository to types, validation, serialization, annotations, and generated contract artifacts.

Two contract pipelines coexist during migration:

1. The legacy manifest/lockfile pipeline keeps hand-written Rust in `src/rust`, generated JSON Schemas in `schemas/`, and generated Dart/TypeScript slices in `src/dart` and `src/ts`.
2. The parity-gated validation pipeline keeps independently authored TypeSpec and JSON Schema Draft 2020-12 authorities under `validation/`. Neither authority is generated from Rust or from the other. `generated/final/` is derivative output and may change only after semantic parity succeeds.

Do not silently translate one authority into the other and call the result agreement.

## Working rules

- For the legacy pipeline, never hand-edit generated files. Regenerate both hops (`cargo run --locked --example generate_schemas`, then `npm run codegen`) in the same commit as the Rust source change.
- For `validation/`, edit both peer authorities intentionally, run the pinned `ORESoftware/api-docs` comparator, and commit candidate signatures, final runtime types, and the parity receipt together.
- Server-scoped models must never appear in browser or edge runtime exports. Private persistence models may describe shape and annotations here, but executable Diesel/SeaORM code stays in `zed-orm-core`.
- JSON Schema is the runtime-validation authority for JSON instances; TypeSpec is a peer design/code-generation authority. Their normalized structural semantics and ORM annotations must agree before downstream generation or migration promotion.
- Every server persistence model must carry matching JSON `x-orm` metadata and TypeSpec `// @orm` annotations. The annotation checker must reject unknown columns, invalid identifiers, duplicate database-object names, foreign-key arity drift, and one-sided model coverage.
- Every file in legacy `schemas/` must be classified in `schemas/index.json`. A new schema is front-end-facing only if a browser or Flutter client decodes it directly; toolchain and on-disk formats stay `"targets": []`.
- Do not add `dart format --set-exit-if-changed` to CI: the generator owns formatting so generated output stays independent of the installed Dart SDK.
- Keep `.zpkg.toml` valid against this crate's own manifest model.
- Preserve serialization compatibility and deterministic ordering unless an explicitly versioned migration says otherwise.
- Treat public Rust types, JSON schemas, TypeSpec models, TOML fields, lockfile formats, generated receipts, and path conventions as cross-repository APIs.
- Add round-trip and negative tests for every parser, schema, annotation, or generator change; do not weaken validation to make one consumer pass.
- Update consumers and monorepo pins in contract-first order.
- Keep the crate free of service-specific network, credential, database-connection, and deployment policy.
- Never commit registry tokens, cloud credentials, generated secrets, or production environment files.
- Keep `Cargo.lock` and `flake.lock` committed; do not allow CI to update either lock implicitly.
- Pin GitHub Actions and external generators by immutable commit SHA. Keep workflow permissions read-only unless a documented write is required.
- Resolve conflicts semantically by preserving compatibility, validation strength, source independence, generated determinism, visibility boundaries, and consumer expectations rather than selecting an entire side.

## Functional design rules

- Prefer explicit inputs, explicit outputs, immutable values, pure transformations, typed errors, explicit state transitions, and composition.
- Push filesystem, process, network, clock, randomness, database, and deployment effects to narrow adapter boundaries.
- Model illegal states so they are excluded by types where practical, and use exhaustive pattern matching for finite state and error domains.
- Keep Rust, TypeScript, and Dart modules focused; do not accumulate unrelated behavior in a single entrypoint or oversized module.
- Use object-oriented state only when lifecycle ownership or performance requires it, while keeping state transitions explicit and testable.
- Favor reusable utilities and moderate deduplication without hiding domain distinctions behind premature abstraction.
- Apply formal or exhaustive checking to state machines, generators, caches, lockfiles, compatibility rules, and failure modes.

## Reproducible validation

Use the pinned shell for the legacy pipeline:

```sh
nix develop -c agent-check
```

Focused legacy stages are available while iterating:

```sh
nix develop -c agent-check format
nix develop -c agent-check lint
nix develop -c agent-check test
nix develop -c agent-check schemas
```

For the peer-authority pipeline, run:

```sh
node validation/parity/orm-annotation-check.mjs --self-test
node validation/parity/orm-annotation-check.mjs --check
node .deps/api-docs/tools/validation-parity/parity-tool.mjs --self-test
node .deps/api-docs/tools/validation-parity/parity-tool.mjs --check
```

The default agent check covers Nix/workflow preflight, rustfmt, Clippy with warnings denied, unit tests, doctests, legacy schema generation, and a clean-tree drift check. The validation workflow independently checks TypeSpec/JSON Schema semantics, ORM annotations, generated runtime visibility, and receipts.

## Repository-local Git worktrees

- Create or use a Git worktree only when the human operator explicitly authorizes it for the current task. Concurrency or a dirty checkout is not permission by itself.
- Put every authorized worktree at `<repository-root>/tmp/worktrees/<name>`; from the repository root, use `./tmp/worktrees/<name>`. Never place worktrees beside repositories or organization directories.
- Keep `tmp`, `temp`, `tmp/worktrees`, and `temp/worktrees` ignored in the repository-root `.gitignore`. Do not commit files from those directories.
- Relocate or remove a worktree only when the operator explicitly requests it. Before removal, preserve and publish intended changes, verify its commit is represented on the target branch, and confirm there are no tracked, untracked, ignored-sensitive, or in-use files that must survive. Remove it with `git worktree remove <path>` without `--force`; never delete a worktree directory with `rm`.
