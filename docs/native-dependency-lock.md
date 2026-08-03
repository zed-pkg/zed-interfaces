# Native dependency requirement and exact-lock contract

`NativeDependencyLock` records how a dependency declaration from npm or Cargo
was translated into Zed's shared SemVer algebra and which exact immutable
artifact satisfied it.

The source registry is part of the requirement identity. Identical text is not
assumed to have identical semantics:

| Declaration | npm | Cargo |
| --- | --- | --- |
| `1.2.3` | exact `=1.2.3` | default caret `^1.2.3` |
| `1.2` | x-range `1.2.*` | default caret `^1.2` |
| `1` | x-range `1.*` | default caret `^1` |
| `^0.2.3` | caret | caret |
| `~1.2.3` | patch-compatible | patch-compatible |

The original declaration remains in the record for auditability. The canonical
requirement is recomputed during validation, so editing it independently causes
a fail-closed drift error.

## Frozen identity

A requirement is not a frozen dependency. A valid v1 lock also requires:

- one exact strict-SemVer package version;
- a native package name valid for the selected registry;
- lowercase, nonzero SHA-256 of the exact artifact bytes;
- a nonzero artifact size; and
- the artifact archive format.

The resolved version must satisfy the recomputed canonical requirement. SemVer
build metadata is rejected for declarations, candidates, and exact lock
versions because it does not participate in precedence and cannot safely name
different artifacts.

## Supported npm subset

Version 1 supports exact versions, partial/x-ranges, `x`/`X`/`*` wildcards,
caret and tilde ranges, explicit comparators, whitespace-separated comparator
intersections, and explicit prerelease requirements. A full bare version is
made explicitly exact.

Logical unions, hyphen ranges, dist-tags, aliases, workspace requirements,
local paths, Git sources, and URL sources are rejected rather than approximated
or silently converted to opaque strings. Cargo-style comma input is also
rejected for npm in strict v1.

## Supported Cargo subset

Version 1 supports Cargo's default-caret bare requirements, explicit caret,
tilde, exact and inequality comparators, `*` wildcards, comma-separated
comparator intersections, and explicit prerelease requirements.

Npm-style `x` wildcards, whitespace-only comparator intersections, unions,
source protocols, and mutable or non-SemVer selectors are rejected.

## Resolution boundary

`NativeDependencyLock::resolve` validates every supplied candidate, rejects
duplicate exact versions, and selects the highest satisfying version independent
of input order. Registry policy such as filtering yanked releases remains the
caller's responsibility; the shared contract operates only on eligible
candidates.

Frozen restore consumes the exact lock result. It does not ask npm, Cargo, Nix,
Flox, Devbox, mise, or asdf to reinterpret or re-resolve the declaration.

## Relationship to publication records

`NativeRegistryAdapterRecord` binds Zed publication identity to one native
publication family, including platform package selection and immutable archive
identity. `NativeDependencyLock` binds one native dependency declaration to one
exact member and artifact. They share registry and artifact types but remain
separate versioned contracts.
