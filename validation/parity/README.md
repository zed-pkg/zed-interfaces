# Validation authority parity and scope layout

TypeSpec and JSON Schema/OpenAPI are independent, peer, human-authored authorities. Neither is generated from the other. `ORESoftware/api-docs` provides the pinned semantic comparator; this repository owns its authorities, generated definitions, and parity receipt.

## Required flow

1. Edit the TypeSpec and JSON Schema authorities independently.
2. Generate a candidate signature and TypeScript, Rust, Go, and Gleam definitions from each authority.
3. Compare semantic signatures and generated candidates. Any discrepancy is a stop-and-evaluate condition.
4. Only after agreement, write `generated/final/**` and `parity-receipt.v2.json` to Git.
5. Require producer CI plus independent consumer certification from a `*-test` organization before release.

## Scope folders

Every model belongs to exactly one scope. `isomorphic` is safe everywhere; `client` is client-only; `edge` is edge-only; `server` is private/server-only. New non-isomorphic sources belong under `validation/authorities/<scope>/` with separate `.json` and `.tsp` files. Generated candidates and finals preserve the same scope. Browser and edge TypeScript entrypoints cannot export server scope. Node.js, Deno, and Bun entrypoints remain distinct even when they currently re-export identical isomorphic types.

Runtime validators live in the companion `*-lib-core`: Zod (TypeScript), Garde (Rust), `go-playground/validator/v10` (Go), and Gleam decoders. Public `*-clients` import those public validation SDK entrypoints; clients must not copy schemas or import server validators.

Route/HTTP signatures use stable `operationId` values from `ORESoftware/api-docs`. Their binding document is digest-bound into the parity receipt.
