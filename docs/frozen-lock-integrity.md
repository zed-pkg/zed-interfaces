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

The public Rust field remains `Option<String>` so existing lock builders can be
migrated without an immediate source break. Parsing, JSON Schema validation,
and serialization still require the value: `None` is a construction-time
intermediate state, not a valid committed lockfile.

## Duplicate identity

A lockfile may contain only one entry for an `org/name` identity. Resolution
must use `Lockfile::upsert` when replacing a package version; hand-authored
duplicate tables are rejected rather than resolved by order.

## Migration

Regenerate an older or incomplete lockfile with a non-frozen resolution command
that can retrieve the registry artifact metadata and immutable source revision.
Do not repair a frozen lock by guessing an archive format, size, checksum, or
revision.

Consumers should validate with `Lockfile::parse` before beginning any install
transaction and should emit committed lockfiles only through
`Lockfile::to_toml_string`.
