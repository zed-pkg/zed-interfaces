# Compiler-backed production public validation

Tracking: DEN-3828 / DEN-3600; ORESoftware/typespec-json-schema-validator#20.

This gate checks the existing production `RequestMeta`, `PageQuery` and
`ProblemDetails` definitions plus their named `PublicValidationContract` union.
It is not the separate repository-local `schema-authority-canary`.

TypeSpec and independently authored Draft 2020-12 JSON Schema remain peers.
The aggregate TypeSpec alias becomes an emitted named union; the peer schema
records the matching union without changing the accepted shapes of the three
existing models. Explicit `additionalProperties: false` extensions and disabled
emitter-wide sealing preserve the authored closure semantics. Nothing is ignored
by mapping policy: the downstream verifier requires exactly all four declarations
and a complete scope. The public union is an exclusive union of disjoint closed
objects, as the authored root already specifies.

`checkPublicParity()` compiles with the pinned upstream toolchain and canonical
flags2env CLI, executes both schema lanes over synthesized probes and the 34
recorded cases in `cases.json`, then invokes the upstream consumer verifier over
actual current paths. The case file is materialized into the upstream validator's
`Declaration/valid|invalid` layout in a fresh temporary directory. Its raw digest
is included in the returned evidence; missing declarations/verdicts, duplicate
IDs, unknown cases and unsafe paths fail before compilation. Every call owns and
removes its temporary files, so no earlier green receipt can be reused.

The returned frozen evidence includes the real receipt, approved IR and recorded
corpus for downstream implementation conformance. It is a checked-revision result,
not a signed publisher assertion or an indefinitely reusable promotion token.

```sh
# Provision .deps/typespec-json-schema-validator at the commit in the module.
npm ci --prefix .deps/typespec-json-schema-validator
node validation/compiler/public-parity.mjs
node --test validation/compiler/public-parity.test.mjs
```

The existing validation workflow now requires this gate before checking generated
runtime outputs. Legacy Rust/schema generation, server/private contracts, ORM
annotations, and browser/edge isolation checks remain intact. This slice does not
certify the private server schema lane or claim universal equivalence from a
finite corpus. Zed runtime implementations must consume these cases in their own
native tests; structural agreement alone is not runtime conformance.
