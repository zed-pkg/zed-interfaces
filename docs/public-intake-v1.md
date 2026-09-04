# Public commercial intake v1

The hand-written Rust model in `src/rust/public_intake.rs` remains the runtime validation and serialization authority. Its generated JSON Schemas remain independent closed-object validators and continue to generate the browser TypeScript and Flutter/Dart slices.

`contracts/public-intake/v1/main.tsp` and `proto/zed_public_intake_v1.proto` are additional semantic representations for API/RPC tooling. Neither replaces Rust, JSON Schema, Diesel/SeaORM persistence models, nor the generated client slices. The dependency-free parity test fails when request fields, enum wire values, endpoint paths, or Protobuf field allocations drift.

## Stable public routes

- `POST /v1/pre-interest` — individual or organization interest registration; it never creates a quote, account, session, role, or entitlement.
- `POST /v1/quote-requests` — organization pricing intake; it remains a distinct intent.

The Cloudflare edge derives `sourceHost`; callers do not get to select an arbitrary host. Both routes return the same PII-free accepted envelope for new and replayed requests.

## Protobuf evolution

Field tags in the v1 messages are permanently allocated. Removed fields must be reserved rather than reused. New fields append tags and require simultaneous changes to Rust, JSON Schema, TypeSpec, generated clients, fixtures, and the semantic-shadow test.
