// Independently authored TypeSpec authority for Cloudflare multi-origin routing.
// Generated JSON Schema is comparison evidence only; authored.schema.json is a peer authority.
import "@typespec/json-schema";
using TypeSpec.JsonSchema;

namespace ZedOriginSet;

enum OriginRuntimeStackV1 {
  kubernetes,
  cloudRun: "cloud_run",
  beamscale,
  scintilla,
}

enum OriginProviderV1 {
  aws,
  gcp,
  beamscale,
  scintilla,
}

enum OriginRouteFamilyV1 {
  registryRead: "registry_read",
  artifactRead: "artifact_read",
  accountRead: "account_read",
  authRead: "auth_read",
  registryWrite: "registry_write",
  accountWrite: "account_write",
  organizationWrite: "organization_write",
  authWrite: "auth_write",
}

model OriginReadPolicyV1 {
  @minValue(100)
  @maxValue(30000)
  request_deadline_ms: uint32;

  @minValue(0)
  @maxValue(5000)
  hedge_delay_ms: uint32;

  @minValue(1)
  @maxValue(2)
  max_inflight_origins: uint32;
}

model OriginWriteAuthoritiesV1 {
  @minLength(1)
  @maxLength(63)
  @pattern("^[a-z][a-z0-9_-]{0,62}$")
  registry?: string;

  @minLength(1)
  @maxLength(63)
  @pattern("^[a-z][a-z0-9_-]{0,62}$")
  accounts?: string;

  @minLength(1)
  @maxLength(63)
  @pattern("^[a-z][a-z0-9_-]{0,62}$")
  organizations?: string;

  @minLength(1)
  @maxLength(63)
  @pattern("^[a-z][a-z0-9_-]{0,62}$")
  auth?: string;
}

model OriginV1 {
  @minLength(1)
  @maxLength(63)
  @pattern("^[a-z][a-z0-9_-]{0,62}$")
  id: string;

  @minLength(9)
  @maxLength(512)
  @pattern("^https://[^\\s/@]+(?:/[^\\s]*)?$")
  base_url: string;

  runtime_stack: OriginRuntimeStackV1;
  provider: OriginProviderV1;

  @minLength(1)
  @maxLength(63)
  @pattern("^[A-Za-z0-9][A-Za-z0-9._-]{0,62}$")
  region?: string;

  @minValue(0)
  @maxValue(1000)
  priority: uint32;

  @minLength(1)
  @maxLength(256)
  deployment_receipt: string;

  @minLength(1)
  @maxLength(128)
  @pattern("^/[A-Za-z0-9/_-]{0,127}$")
  health_path: string;

  @minLength(1)
  @maxLength(128)
  @pattern("^/[A-Za-z0-9/_-]{0,127}$")
  compatibility_path: string;

  @minLength(1)
  @maxLength(128)
  expected_audience: string;

  @minItems(1)
  @maxItems(8)
  route_families: OriginRouteFamilyV1[];
}

@id("OriginSetV1")
model OriginSetV1 {
  contract_version: "zed.origin-set/v1";

  @minLength(1)
  @maxLength(63)
  @pattern("^[a-z][a-z0-9_-]{0,62}$")
  set_id: string;

  @minValue(1)
  generation: uint32;

  @minItems(1)
  @maxItems(16)
  origins: OriginV1[];

  read_policy: OriginReadPolicyV1;
  write_authorities: OriginWriteAuthoritiesV1;
}
