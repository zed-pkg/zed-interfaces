# Public validation contracts

Only client-safe contracts belong here. `RequestMeta`, `PageQuery`, and `ProblemDetails` are independently authored in JSON Schema and TypeSpec as peer, top-level authorities. Neither source may overwrite the other; semantic differences stop release.

`zed-lib-core` implements these contracts natively and owns separate server-only definitions. `TrustedActor`, `ServerRequestContext`, and `InternalCommand` must never appear here or in `zed-clients` artifacts.

`route-bindings.v1.json` may reference only stable operation IDs from `ORESoftware/api-docs`. It begins empty rather than guessing route signatures. Bindings must be reviewed with the corresponding digest-bound api-docs route change.

## TJSV enforcement

CI additionally executes `ORESoftware/typespec-json-schema-validator` at immutable revision `2281843126ab644607b11cf8281d84f382d68dfc` against `validation/authorities/server/contracts.tsp` and `validation/authorities/server/contracts.json`. TJSV's TypeSpec-emitted JSON Schema is comparison evidence only; it cannot overwrite or outrank the authored JSON Schema. Structural or behavioral drift stops the workflow for evaluation and blocks promotion until reconciled.

This gate complements the existing `api-docs` parity and ORM-annotation checks; it does not replace them. A TJSV pass is scoped to the declarations and probes it evaluated and does not certify route bindings, runtime visibility, Zed package publication, registry authorization, persistence permissions, or generated language behavior by itself.
