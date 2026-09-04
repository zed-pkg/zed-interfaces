# Public-intake contract instructions

These instructions refine the repository-root `agents.md` for the v1 commercial-intake contract.

- Apply the organization-wide agent and functional-boundary policy from `https://github.com/ORESoftware/my-ai/blob/main/AGENTS.md` together with every readable ancestor `agents.md`.
- Keep TypeSpec, Protobuf, JSON Schema, Rust, Dart, and TypeScript as independently checked representations. Protobuf never replaces the other schema technologies.
- Preserve closed request objects, stable Protobuf field numbers, exact host/party binding, distinct registration and quote intents, and PII-free public responses.
- Never add credentials, authentication tokens, identity documents, private keys, regulated-data prompts, or administrative grants to public intake DTOs.
- Changes to fields, enum wire values, or endpoint paths require the semantic-shadow test and every generated projection to move together.
