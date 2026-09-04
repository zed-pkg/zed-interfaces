# Public validation contracts

Only client-safe contracts belong here. `RequestMeta`, `PageQuery`, and `ProblemDetails` are independently authored in JSON Schema and TypeSpec as peer, top-level authorities. Neither source may overwrite the other; semantic differences stop release.

`zed-lib-core` implements these contracts natively and owns separate server-only definitions. `TrustedActor`, `ServerRequestContext`, and `InternalCommand` must never appear here or in `zed-clients` artifacts.

`route-bindings.v1.json` may reference only stable operation IDs from `ORESoftware/api-docs`. It begins empty rather than guessing route signatures. Bindings must be reviewed with the corresponding digest-bound api-docs route change.
