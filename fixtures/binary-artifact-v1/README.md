# Binary artifact v1 conformance fixtures

`golden/minimal.json` is the exact canonical JSON byte representation of a minimal valid `zpkg.binary-artifact/v1` descriptor. Consumers should parse it, validate all cross-field relationships, re-serialize it with the canonical encoder, and require byte-for-byte equality.

The archive-level conformance suite belongs in `zed-e2e`: it should combine this descriptor contract with deterministic ZIP vectors for traversal, duplicate and case-folding collisions, symlinks and special files, overlapping ranges, encryption, unsupported compression, self-extracting prefixes, file-count and expansion limits, missing and extra payloads, executable-mode disagreement, and inner/outer digest corruption.
