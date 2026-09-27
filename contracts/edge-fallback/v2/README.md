# Edge fallback capability v2

V2 is the live authenticated-fallback contract. It extends the v1 resource
capability with signed Shared Auth lineage so the Cloudflare edge and provider
credential broker can preserve revocation/session provenance during an origin
outage without receiving the caller's bearer token.

The peer authorities are independent:

- `main.tsp` — TypeSpec authority.
- `authored.schema.json` — JSON Schema Draft 2020-12 authority.

Neither is generated from the other and neither has precedence.

## Difference from v1

V1 proved an exact read-only package/provider/resource grant but did not carry
the session lineage required by the broker contract. V2 adds two required,
bounded, non-secret claims:

- `sid` — the canonical Shared Auth session id already validated by Zed.
- `parent_jti` — the delegated Shared Auth parent-token id already validated
  by Zed.

The edge capability's own `jti` remains the capability id. These three values
are deliberately distinct and MUST NOT be substituted for one another.

## Security invariants

All v1 resource restrictions remain:

- audience is exactly `zed-edge-fallback`;
- grants are read-only and limited to 1..16;
- each grant binds one canonical Zed package to one provider resource;
- `credential_ref` identifies a broker lookup and is never a credential;
- unknown fields fail closed.

Additionally, v2 requires signed `sid` and `parent_jti`. They are lineage
identifiers only: bearer tokens, cookies, PATs, npm/Cargo tokens, SSH keys, and
authorization headers are forbidden from this contract.

Runtime policy still owns cryptographic JOSE-header rules, maximum five-minute
TTL, exact temporal ordering, replay/freshness checks, key rotation, destination
URL confinement, provider credential exchange, and cache policy.

## Migration

- v1 remains valid for compatibility and tests, but MUST NOT be used to enable
  live private fallback through the credential broker.
- issuers should mint v2 for live authenticated fallback.
- edge verifiers may accept v1 only for non-broker compatibility paths; the
  private-provider broker path must require v2 lineage.
