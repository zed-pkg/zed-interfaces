# Zed signed-build v1 contract (draft)

This is a separate, domain-separated integrity assertion for a compiled
artifact, NOT the existing zpkg.signed-version publisher metadata and NOT
the in-toto/DSSE envelope used by the experimental Oreslang reference
verifier. No signing scheme may be silently substituted for another.

Authority: Rust src/rust/signed_build.rs and its negative tests.
The claims bind:
- audience: zed-pkg/oreslang-build-api/v1
- org/package/version, constrained device key ID
- UUID challenge, SHA-256 of server-issued 256-bit nonce
- source Git commit AND independent source SHA-256
- toolchain SHA-256, target backend
- exact artifact bytes SHA-256, nonzero size and archive format
- issued_at_unix_ms and expires_at_unix_ms, <=5 minute lifetime

The deterministic preimage is the UTF-8 prefix
    zpkg.signed-build/1 newline
followed immediately by serde_json::to_vec(claims).

The signer uses Zed's existing Ed25519 publisher-key representation and
DetachedSignatureV1, not an upload bearer token. The API must independently
look up a currently authorized key, validate signer identity and verify
the original immutable challenge and its durable state before consuming it.

**Security classification:** even after verifying this signature and the
corresponding immutable registry blob, evidence tier is
signed-self-report-not-compiled-proof. The signature alone does NOT prove
the source was compiled, that a TEE ran, that the challenge was issued by
the correct authority, or that an API challenge was consumed atomically.

Canonical challenge issuance, durable one-use admission and release-level
attestation are separate requirements in Oreslang API:
https://github.com/ores-truffle-oreslang/oreslang-api-server.ores/pull/1

Future interoperability must explicitly translate between this Zed
domain-separated signed claim and the Oreslang in-toto/DSSE v1 reference.
No unsigned translation is authorized to elevate a receipt to build proof.

Legacy schema/codegen promotion is pending compatibility tests: only Rust
types/tests are introduced in this PR. Browser/SDK surfaces must not pretend
the contract is already generated for every language.
