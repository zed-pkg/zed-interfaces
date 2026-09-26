import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = dirname(fileURLToPath(import.meta.url));

function readFixture(relativePath) {
  return JSON.parse(readFileSync(resolve(root, relativePath), "utf8"));
}

function semanticErrors(document) {
  const errors = [];
  const origins = Array.isArray(document.origins) ? document.origins : [];
  const byId = new Map();

  for (const origin of origins) {
    if (byId.has(origin.id)) {
      errors.push(`duplicate_origin_id:${origin.id}`);
    } else {
      byId.set(origin.id, origin);
    }

    let parsed;
    try {
      parsed = new URL(origin.base_url);
    } catch {
      errors.push(`invalid_origin_url:${origin.id}`);
      continue;
    }

    if (parsed.protocol !== "https:") {
      errors.push(`non_https_origin:${origin.id}`);
    }
    if (parsed.username || parsed.password) {
      errors.push(`credential_bearing_origin_url:${origin.id}`);
    }

    const providerAllowed =
      (origin.runtime_stack === "kubernetes" && ["aws", "gcp"].includes(origin.provider)) ||
      (origin.runtime_stack === "cloud_run" && origin.provider === "gcp") ||
      (origin.runtime_stack === "beamscale" && origin.provider === "beamscale") ||
      (origin.runtime_stack === "scintilla" && origin.provider === "scintilla");

    if (!providerAllowed) {
      errors.push(`provider_runtime_mismatch:${origin.id}`);
    }

    const routeFamilies = Array.isArray(origin.route_families) ? origin.route_families : [];
    if (new Set(routeFamilies).size !== routeFamilies.length) {
      errors.push(`duplicate_route_family:${origin.id}`);
    }
  }

  const authorities = document.write_authorities || {};
  const authorityCapabilities = {
    registry: "registry_write",
    accounts: "account_write",
    organizations: "organization_write",
    auth: "auth_write",
  };

  for (const [authorityName, capability] of Object.entries(authorityCapabilities)) {
    const declared = authorities[authorityName];
    const capableOrigins = origins.filter((origin) =>
      Array.isArray(origin.route_families) && origin.route_families.includes(capability),
    );

    if (capableOrigins.length > 0 && !declared) {
      errors.push(`missing_write_authority:${authorityName}`);
      continue;
    }
    if (!declared) {
      continue;
    }

    const authority = byId.get(declared);
    if (!authority) {
      errors.push(`unknown_write_authority:${authorityName}:${declared}`);
      continue;
    }
    if (!authority.route_families.includes(capability)) {
      errors.push(`write_authority_capability_mismatch:${authorityName}:${declared}`);
    }
  }

  return errors;
}

const valid = readFixture("instances/OriginSetV1/valid/multi-cloud.json");
assert.deepEqual(semanticErrors(valid), []);

const duplicate = readFixture("instances/OriginSetV1/invalid/duplicate-origin-id.json");
assert.ok(semanticErrors(duplicate).some((error) => error.startsWith("duplicate_origin_id:")));

const providerMismatch = readFixture(
  "instances/OriginSetV1/invalid/provider-runtime-mismatch.json",
);
assert.ok(
  semanticErrors(providerMismatch).some((error) => error.startsWith("provider_runtime_mismatch:")),
);

const writeMismatch = readFixture(
  "instances/OriginSetV1/invalid/write-authority-capability-mismatch.json",
);
assert.ok(
  semanticErrors(writeMismatch).some((error) =>
    error.startsWith("write_authority_capability_mismatch:"),
  ),
);

const httpOrigin = readFixture("instances/OriginSetV1/invalid/http-origin.json");
assert.ok(semanticErrors(httpOrigin).some((error) => error.startsWith("non_https_origin:")));

console.log("OriginSetV1 semantic admission fixtures passed");
