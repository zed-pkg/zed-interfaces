# Edge fallback capability v1

This directory owns the peer-authority wire contract for the short-lived,
read-only capability that permits authenticated package fallback at the
Cloudflare edge.

The two authored authorities are intentionally independent:

- `main.tsp` — TypeSpec authority.
- `authored.schema.json` — JSON Schema Draft 2020-12 authority.

Neither file is generated from the other and neither has precedence. CI uses
`ORESoftware/typespec-json-schema-validator` to compare them through Contract
IR and differential validation.

## Security invariants

- version is exactly `1`;
- audience is exactly `zed-edge-fallback`;
- grants are read-only;
- 1..16 grants per capability;
- each grant binds one canonical Zed package coordinate to one provider
  resource;
- GitHub grants bind one `owner/repo`;
- npm grants bind one exact private scoped package;
- private Cargo grants bind one crate and one exact credential-free HTTPS
  registry origin;
- `credential_ref` identifies a credential broker lookup and is never a
  credential;
- additional fields are rejected, so raw tokens, passwords, authorization
  headers, SSH keys, and other unversioned secret-bearing fields cannot be
  added to a grant.

Cryptographic JWT-header policy, five-minute maximum token lifetime, issuer
key rotation, replay handling, destination URL confinement, and cache policy
are runtime policy. The payload contract deliberately does not pretend JSON
Schema alone proves those properties.

Consumers:

- `zed-pkg/zed-infra` Cloudflare Workers verify the capability offline from
  pinned/cached public keys and derive a provider request plan.
- Shared Auth / `zed-api-server.rs` will issue the capability after proving the
  user's package authorization.
- Provider credential brokers resolve `credential_ref` to short-lived,
  provider-specific read credentials without placing those secrets in the JWT.
