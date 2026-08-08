#!/usr/bin/env python3
"""One-use public-surface patch for the root-signature discovery DTO."""

from pathlib import Path

path = Path("src/lib.rs")
source = path.read_text(encoding="utf-8")
old = (
    "    RegistryLimitsV1, RegistryProtocolErrorCodeV1, RegistryProtocolErrorV1,\n"
    "    RegistryProtocolV1Error, RegistryPublishRequestV1, RegistrySigningKeyStateV1,\n"
    "    RegistrySigningKeyV1, RegistryVisibilityV1,\n"
)
new = (
    "    RegistryLimitsV1, RegistryProtocolErrorCodeV1, RegistryProtocolErrorV1,\n"
    "    RegistryProtocolV1Error, RegistryPublishRequestV1, RegistryRootSignatureV1,\n"
    "    RegistrySigningKeyStateV1, RegistrySigningKeyV1, RegistryVisibilityV1,\n"
)
count = source.count(old)
if count != 1:
    raise SystemExit(f"expected one registry export block, found {count}")
path.write_text(source.replace(old, new, 1), encoding="utf-8")
