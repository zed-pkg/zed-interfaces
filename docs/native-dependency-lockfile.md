# Native dependency provenance in `.zpkg.lock`

Lockfile version 1 may contain zero or more `[[native-dependency]]` tables. Each
table is a complete `NativeDependencyLock`: source registry, original native
requirement, deterministic canonical requirement, exact resolved version, and
immutable artifact identity.

Existing lockfiles remain valid because the field is additive and defaults to
an empty list.

## TOML shape

```toml
version = 1

[[native-dependency]]
schema = "zed.native-dependency-lock/v1"

[native-dependency.requirement]
registry = "npm"
declared = "^1.2.3"
canonical = "^1.2.3"

[native-dependency.package]
name = "@fiducia/core"
version = "1.9.0"

[native-dependency.artifact]
sha256 = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
size = 1024
format = "tar.gz"
```

The lockfile owns exact restore identity. Nix, Flox, Devbox, mise, asdf,
container export, and future npm/Cargo adapters consume the frozen package
version and artifact hash; they do not reinterpret `declared` or rerun native
range resolution.

## Identity and duplicates

Native lock uniqueness is `(registry, package.name)`. Npm and Cargo entries with
the same textual name may coexist because their naming and requirement semantics
are independent. Two entries for the same registry and package name are rejected
while parsing or writing rather than resolved by array order.

Protocol aliases and workspace, path, Git, or URL sources are not represented by
`NativeDependencyLock` v1 and cannot create hidden duplicate identities.

## Validation

`Lockfile::parse`, `Lockfile::to_toml_string`, and
`Lockfile::upsert_native_dependency` call `NativeDependencyLock::validate`.
That validation:

- recomputes the source-aware canonical requirement;
- rejects requirement-receipt drift;
- checks that the exact resolved version satisfies the requirement;
- validates npm or Cargo package naming;
- rejects SemVer build metadata; and
- validates lowercase nonzero SHA-256, nonzero size, and archive format.

The lockfile layer then rejects duplicate native keys.

## Determinism

Before serialization, the lockfile normalizes:

1. ordinary Zed packages by `(org, name)`;
2. native dependencies by `(registry, package.name)`; and
3. Nix adapters by their existing package, direction, system, and output key.

Insertion order therefore does not change emitted TOML.

## Public helpers

```rust
lockfile.find_native_dependency(NativeRegistry::Npm, "@fiducia/core");
lockfile.upsert_native_dependency(exact_lock)?;
```

Upsert validates the new entry, replaces only the same native identity, and
restores deterministic ordering. A Cargo package with the same textual name is
not replaced by an npm entry.

## Compatibility

The serialized change is additive within lockfile version 1. The Rust `Lockfile`
struct gains a public `native_dependencies` vector, so downstream crates that
construct it with struct literals must add `native_dependencies: Vec::new()` or
prefer `Lockfile::default()` plus the public upsert methods. The independent
canary compiles the current downstream CLI against the exact interface commit to
make this source-level change explicit rather than assuming compatibility.
