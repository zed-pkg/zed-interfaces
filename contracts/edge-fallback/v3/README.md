# Edge fallback capability v3

V3 is the bounded-outage candidate for authenticated private fallback. It
extends v2 with the minimum signed authentication context the edge needs to
decide whether an already-authorized session may continue briefly while Shared
Auth or the Zed origin is unavailable.

The peer wire authorities remain independently authored:

- `main.tsp` — TypeSpec authority.
- `authored.schema.json` — JSON Schema Draft 2020-12 authority.

Neither is generated from the other and neither has precedence.

## Difference from v2

V2 carries canonical session lineage (`sid` and `parent_jti`) but leaves
assurance and revocation freshness entirely outside the capability. V3 adds:

- `assurance` — the authenticated assurance level, currently 1 or 2;
- `session_epoch` — the canonical session epoch observed at issuance;
- `policy_epoch` — the canonical policy epoch observed at issuance;
- `revocation_checked_at` — the authoritative Unix-second checkpoint at which
  revocation/session validity was last checked.

These fields are provenance, not a new authorization source. Zed must authorize
the exact package/provider/resource before minting the capability.

## Bounded outage admission

The reference Rust projection combines a v3 capability with trusted local
runtime facts. Admission requires all of the following:

1. the v3 signature/issuer/audience/JTI/resource contract has already passed;
2. `now` is within `[nbf, exp)` and the capability remains within the
   five-minute contract lifetime;
3. capability age is within the configured outage policy;
4. `assurance` meets the configured minimum;
5. `revocation_checked_at` is not newer than issuance and remains within the
   configured freshness bound;
6. the verifier's **local trusted JWKS snapshot** remains within its configured
   freshness bound;
7. the locally observed outage duration remains within its configured bound.

Unknown `kid` values fail closed at the JOSE layer. Key freshness is
deliberately verifier-local: a bearer capability is not allowed to self-assert
that its own signing key or JWKS snapshot is fresh.

A Shared Auth outage never turns an expired capability, stale revocation
checkpoint, stale keyset, insufficient-assurance session, or unknown signing key
into authority.

## Security invariants

All v2 restrictions remain:

- audience is exactly `zed-edge-fallback`;
- grants are read-only and limited to 1..16;
- each grant binds one canonical Zed package to one provider resource;
- `credential_ref` is an opaque broker lookup identity, never a credential;
- bearer tokens, cookies, PATs, npm/Cargo tokens, SSH keys and authorization
  headers are forbidden;
- unknown fields fail closed.

The session and policy epochs are signed continuity markers. During an outage
the edge may compare them with locally trusted minima, but it must never infer a
newer epoch or treat equality of email/domain/profile data as identity.

## Activation

V2 remains the currently documented live broker contract. V3 must not become a
production fallback merely because these files merge. Activation additionally
requires the issuer, Cloudflare verifier, credential broker and outage E2E suite
to implement the same admission semantics, plus key-rotation/replay and
private-cache negative controls.
