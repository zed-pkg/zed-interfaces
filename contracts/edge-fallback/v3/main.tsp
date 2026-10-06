// Independently authored TypeSpec authority for authenticated edge fallback capabilities.
// V3 adds signed Shared Auth lineage required for outage-time broker/revocation checks.
// The JSON Schema beside this file is a peer authority; generated schemas are
// comparison evidence only and neither authority has precedence.

import "@typespec/json-schema";
using TypeSpec.JsonSchema;

namespace Zed.EdgeFallback.V3;

@id("EdgeFallbackCapabilityV3")
model EdgeFallbackCapabilityV3 {
  zed_edge_capability: 3;

  @minLength(1)
  @maxLength(512)
  iss: string;

  aud: "zed-edge-fallback";

  @minLength(1)
  @maxLength(256)
  sub: string;

  @minLength(1)
  @maxLength(256)
  @pattern("^[A-Za-z0-9][A-Za-z0-9:._/-]{0,255}$")
  sid: string;

  @minLength(1)
  @maxLength(256)
  @pattern("^[A-Za-z0-9][A-Za-z0-9:._/-]{0,255}$")
  parent_jti: string;

  // Authentication assurance established before the outage capability was minted.
  @minValue(1)
  @maxValue(2)
  assurance: safeint;

  // Canonical Shared Auth epochs bound at issuance. The edge does not invent
  // newer epochs while the authority is unavailable.
  @minValue(0)
  @maxValue(9007199254740991)
  session_epoch: safeint;

  @minValue(0)
  @maxValue(9007199254740991)
  policy_epoch: safeint;

  // Unix second at which revocation/session validity was last authoritatively
  // checked. Runtime outage policy limits how stale this value may become.
  @minValue(0)
  @maxValue(9007199254740991)
  revocation_checked_at: safeint;

  @minValue(0)
  @maxValue(9007199254740991)
  iat: safeint;

  @minValue(0)
  @maxValue(9007199254740991)
  nbf: safeint;

  @minValue(0)
  @maxValue(9007199254740991)
  exp: safeint;

  @minLength(8)
  @maxLength(192)
  @pattern("^[A-Za-z0-9][A-Za-z0-9._:-]{7,191}$")
  jti: string;

  @minItems(1)
  @maxItems(16)
  grants: EdgeFallbackGrantV3[];
}

union EdgeFallbackGrantV3 {
  github: GitHubEdgeFallbackGrantV3,
  npm: NpmEdgeFallbackGrantV3,
  cargoRegistry: CargoRegistryEdgeFallbackGrantV3,
}

model GitHubEdgeFallbackGrantV3 {
  provider: "github";
  operation: "read";

  @minLength(3)
  @maxLength(257)
  @pattern("^[a-z0-9]+(?:-[a-z0-9]+)*/[a-z0-9]+(?:-[a-z0-9]+)*$")
  package: string;

  @minLength(3)
  @maxLength(256)
  @pattern("^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$")
  resource: string;

  @minLength(1)
  @maxLength(192)
  @pattern("^[A-Za-z0-9][A-Za-z0-9:._/-]{0,191}$")
  credential_ref: string;
}

model NpmEdgeFallbackGrantV3 {
  provider: "npm";
  operation: "read";

  @minLength(3)
  @maxLength(257)
  @pattern("^[a-z0-9]+(?:-[a-z0-9]+)*/[a-z0-9]+(?:-[a-z0-9]+)*$")
  package: string;

  @minLength(4)
  @maxLength(256)
  @pattern("^@[a-z0-9][a-z0-9._-]*/[a-z0-9][a-z0-9._-]*$")
  resource: string;

  @minLength(1)
  @maxLength(192)
  @pattern("^[A-Za-z0-9][A-Za-z0-9:._/-]{0,191}$")
  credential_ref: string;
}

model CargoRegistryEdgeFallbackGrantV3 {
  provider: "cargo-registry";
  operation: "read";

  @minLength(3)
  @maxLength(257)
  @pattern("^[a-z0-9]+(?:-[a-z0-9]+)*/[a-z0-9]+(?:-[a-z0-9]+)*$")
  package: string;

  @minLength(1)
  @maxLength(128)
  @pattern("^[A-Za-z0-9][A-Za-z0-9_-]{0,127}$")
  resource: string;

  @minLength(9)
  @maxLength(512)
  @pattern("^https://[A-Za-z0-9.-]+(?::[0-9]{1,5})?$")
  origin: string;

  @minLength(1)
  @maxLength(192)
  @pattern("^[A-Za-z0-9][A-Za-z0-9:._/-]{0,191}$")
  credential_ref: string;
}
