# Agent instructions

## Scope and hierarchy

- These instructions apply to the whole `zed-pkg/zed-interfaces` repository unless a deeper lowercase `agents.md` adds narrower rules.
- Before editing, resolve the current working directory and load every readable ancestor `agents.md` from the filesystem root to the working directory. Do not search siblings. Resolve symlinks, deduplicate resolved files, and report unreadable or cyclic instruction files.
- `.claude/CLAUDE.md`, `.gemini/GEMINI.md`, and `.openai/AGENTS.md` are pointers only. Never duplicate instructions in tool-specific files.

## Repository role

This crate is the shared contract boundary for Zed manifests, lockfiles, registry models, language targets, paths, version requirements, synchronization, and VCS metadata. Changes here affect the CLI, API server, web server, sync engine, and generated clients.

## Working rules

- Preserve serialization compatibility and deterministic ordering unless an explicitly versioned migration says otherwise.
- Treat public Rust types, JSON schemas, TOML fields, lockfile formats, and path conventions as cross-repository APIs.
- Add round-trip and negative tests for every parser or schema change; do not weaken validation to make one consumer pass.
- Update interface consumers and monorepo pins in contract-first order when a change spans repositories.
- Keep the crate free of service-specific network, credential, database, and deployment policy.
- Never commit registry tokens, cloud credentials, generated secrets, or production environment files.
- Keep `Cargo.lock` and `flake.lock` committed; do not allow CI to update either lock implicitly.
- Pin GitHub Actions by immutable commit SHA and keep workflow permissions read-only unless a documented write is required.
- Resolve conflicts by preserving serialization compatibility, validation strength, generated-schema determinism, and consumer expectations rather than selecting an entire side.

## Reproducible validation

Use the pinned shell rather than mutable host toolchains:

```sh
nix develop -c agent-check
```

Focused stages are available while iterating:

```sh
nix develop -c agent-check format
nix develop -c agent-check lint
nix develop -c agent-check test
nix develop -c agent-check schemas
```

The default command runs Nix/workflow preflight, rustfmt, Clippy with warnings denied, unit tests, doctests, schema generation, and a clean-tree schema drift check.

## Validation

The pinned `agents policy` workflow validates this hierarchy and the three tool pointers. Run `nix develop -c agent-check` before requesting review.
