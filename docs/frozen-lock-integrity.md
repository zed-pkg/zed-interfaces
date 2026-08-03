# Frozen lock integrity contract

A frozen Zed install treats every `[[package]]` entry in `.zpkg.lock` as an
immutable artifact identity. It must reject an incomplete or ambiguous entry
before creating a staging directory, downloading an archive, or changing the
active installation.

## Required package fields

Each locked package must contain all of the following:

- non-empty `org`, `name`, and resolved `version`;
- a canonical 64-character lowercase hexadecimal `sha256` that is not the
  all-zero digest;
- a nonzero artifact `size`;
- an explicit `format` (`tar.gz` or `zip`), never an inferred default;
- non-empty `vcs_tag` and an explicit immutable `vcs_commit`;
- a non-empty registry `source`.

`vcs_commit` is named for compatibility with the existing wire format, but it
may identify an immutable revision from Git, Mercurial, Fossil, Pijul, or
another VCS. Moving names such as `HEAD`, `main`, `master`, `trunk`, `latest`,
`refs/heads/*`, and `heads/*` are invalid.

The public Rust field remains `Option<String>` so existing lock builders and
legacy registry responses can be migrated without an immediate source break.
Parsing and JSON Schema validation still require the value to be explicitly
present in every committed lockfile.

`Lockfile::to_toml_string` is the one compatibility boundary: when an in-memory
package has no stronger source revision, the writer emits
`artifact-sha256:<sha256>`. This is not a guessed Git commit. It is an explicit,
content-addressed revision that identifies the exact published archive bytes.
The writer derives it only from a canonical, nonzero artifact digest and leaves
the caller's in-memory structure unchanged. Empty, malformed, all-zero, or
mutable revisions still fail.

This fallback supports packages published by older registries or through an
explicit VCS-check bypass while preserving the stronger invariant that every
serialized and frozen lock carries immutable provenance. Publishers with a
verified source revision continue to retain that exact revision unchanged.

## Schema consumers

`schemas/lockfile.json` places every package identity field—including
`format` and `vcs_commit`—in the object-level `required` array. API clients and
editors should validate against that checked-in schema instead of deriving
requiredness from the Rust `Option` representation.

## Duplicate identity

A lockfile may contain only one entry for an `org/name` identity. Resolution
must use `Lockfile::upsert` when replacing a package version; hand-authored
duplicate tables are rejected rather than resolved by order.

## Migration

Regenerate an older or incomplete lockfile with a non-frozen resolution command
that can retrieve the registry artifact metadata. The canonical writer may use
the exact artifact digest as the immutable revision only when no stronger
source revision exists. Do not repair a frozen lock by guessing an archive
format, size, checksum, tag, source, or revision.

Consumers should validate with `Lockfile::parse` before beginning any install
transaction and should emit committed lockfiles only through
`Lockfile::to_toml_string`.
