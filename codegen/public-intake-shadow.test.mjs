import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const read = (path) => readFileSync(new URL(`../${path}`, import.meta.url), "utf8");
const schema = (name) => JSON.parse(read(`schemas/${name}.json`));
const typeSpec = read("contracts/public-intake/v1/main.tsp");
const proto = read("proto/zed/public_intake/v1/public_intake.proto");
const rust = read("src/rust/public_intake.rs");

const requestSchemas = new Map([
  ["PreInterestRegistrationRequestV1", schema("pre-interest-registration-request-v1")],
  ["QuoteRequestV1", schema("quote-request-v1")],
  ["PublicIntakeAcceptedV1", schema("public-intake-accepted-v1")],
  ["PublicIntakeErrorV1", schema("public-intake-error-v1")],
]);

function block(source, kind, name) {
  const match = source.match(new RegExp(`${kind}\\s+${name}\\s*\\{([\\s\\S]*?)\\n\\}`, "m"));
  assert.ok(match, `${kind} ${name} exists`);
  return match[1];
}

function typeSpecFields(name) {
  return new Set(
    block(typeSpec, "model", name)
      .split("\n")
      .map((line) => line.match(/^\s*([A-Za-z][A-Za-z0-9]*)(?:\?)?\s*:/)?.[1])
      .filter(Boolean),
  );
}

function toCamel(value) {
  return value.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase());
}

function protoFields(name) {
  const fields = [];
  const numbers = [];
  for (const line of block(proto, "message", name).split("\n")) {
    const match = line.match(/^\s*(?:optional\s+|repeated\s+)?[.A-Za-z][.A-Za-z0-9_]*\s+([a-z][a-z0-9_]*)\s*=\s*(\d+)\s*;/);
    if (match) {
      fields.push(toCamel(match[1]));
      numbers.push(Number(match[2]));
    }
  }
  assert.equal(new Set(numbers).size, numbers.length, `${name} field numbers are unique`);
  assert.deepEqual(numbers, numbers.slice().sort((a, b) => a - b), `${name} field numbers are monotonic`);
  return new Set(fields);
}

function wireValues(source, kind, name) {
  const values = [];
  for (const line of block(source, kind, name).split("\n")) {
    const explicit = line.match(/:\s*"([^"]+)"/);
    if (explicit) {
      values.push(explicit[1]);
      continue;
    }
    const ordinary = line.match(/^\s*([a-z][A-Za-z0-9]*)\s*,/);
    if (ordinary) values.push(ordinary[1]);
  }
  return values;
}

function protoWireValues(name) {
  return block(proto, "enum", name)
    .split("\n")
    .map((line) => line.match(/\/\/\s*wire:\s*([^\s]+)/)?.[1])
    .filter(Boolean);
}

function assertSameSet(actual, expected, label) {
  assert.deepEqual([...actual].sort(), [...expected].sort(), label);
}

test("TypeSpec and Protobuf message fields exactly shadow generated JSON Schemas", () => {
  for (const [name, document] of requestSchemas) {
    const expected = new Set(Object.keys(document.properties));
    assertSameSet(typeSpecFields(name), expected, `${name} TypeSpec fields`);
    assertSameSet(protoFields(name), expected, `${name} Protobuf fields`);
    assert.equal(document.additionalProperties, false, `${name} remains closed`);
  }
});

test("enumeration wire values remain aligned across all representations", () => {
  const definitions = {
    PublicIntakeSchemaV1: requestSchemas.get("PreInterestRegistrationRequestV1").$defs.PublicIntakeSchemaV1.enum,
    PublicIntakeSourceHostV1: requestSchemas.get("PreInterestRegistrationRequestV1").$defs.PublicIntakeSourceHostV1.enum,
    PublicIntakePartyV1: requestSchemas.get("PreInterestRegistrationRequestV1").$defs.PublicIntakePartyV1.enum,
    PublicIntakeInterestV1: requestSchemas.get("PreInterestRegistrationRequestV1").$defs.PublicIntakeInterestV1.enum,
    QuoteDeploymentModelV1: requestSchemas.get("QuoteRequestV1").$defs.QuoteDeploymentModelV1.enum,
    QuoteTeamSizeBandV1: requestSchemas.get("QuoteRequestV1").$defs.QuoteTeamSizeBandV1.enum,
    QuotePackageCountBandV1: requestSchemas.get("QuoteRequestV1").$defs.QuotePackageCountBandV1.enum,
    QuoteMonthlyDownloadBandV1: requestSchemas.get("QuoteRequestV1").$defs.QuoteMonthlyDownloadBandV1.enum,
    QuoteMigrationWindowV1: requestSchemas.get("QuoteRequestV1").$defs.QuoteMigrationWindowV1.enum,
    PublicIntakeAcceptedStatusV1: requestSchemas.get("PublicIntakeAcceptedV1").$defs.PublicIntakeAcceptedStatusV1.enum,
    PublicIntakeErrorCodeV1: requestSchemas.get("PublicIntakeErrorV1").$defs.PublicIntakeErrorCodeV1.enum,
  };

  for (const [name, expected] of Object.entries(definitions)) {
    assertSameSet(wireValues(typeSpec, "enum", name), expected, `${name} TypeSpec values`);
    assertSameSet(protoWireValues(name), expected, `${name} Protobuf semantic values`);
  }
});

test("transport routes stay coupled to the Rust authority", () => {
  for (const path of ["/v1/pre-interest", "/v1/quote-requests"]) {
    assert.match(typeSpec, new RegExp(`@route\\(\\"${path}\\"\\)`));
    assert.match(proto, new RegExp(`HTTP: POST ${path.replaceAll("/", "\\/")}`));
    assert.ok(rust.includes(`"${path}"`), `Rust exports ${path}`);
  }
});

test("Protobuf RPCs use unique Buf-standard transport envelopes", () => {
  const service = block(proto, "service", "PublicIntakeService");
  assert.match(
    service,
    /rpc SubmitPreInterest\(SubmitPreInterestRequest\) returns \(SubmitPreInterestResponse\);/,
  );
  assert.match(
    service,
    /rpc SubmitQuoteRequest\(SubmitQuoteRequestRequest\) returns \(SubmitQuoteRequestResponse\);/,
  );
  assertSameSet(protoFields("SubmitPreInterestRequest"), new Set(["request"]), "pre-interest request envelope");
  assertSameSet(protoFields("SubmitPreInterestResponse"), new Set(["accepted"]), "pre-interest response envelope");
  assertSameSet(protoFields("SubmitQuoteRequestRequest"), new Set(["request"]), "quote request envelope");
  assertSameSet(protoFields("SubmitQuoteRequestResponse"), new Set(["accepted"]), "quote response envelope");
});

test("public wire messages cannot carry credentials or administrative authority", () => {
  const forbidden = /password|secret|token|privateKey|apiKey|roleGrant|admin/i;
  for (const [name, document] of requestSchemas) {
    for (const field of Object.keys(document.properties)) {
      assert.doesNotMatch(field, forbidden, `${name}.${field}`);
    }
  }
});

test("protobuf field numbers are frozen at the reviewed v1 allocation", () => {
  const expected = {
    PreInterestRegistrationRequestV1: 16,
    QuoteRequestV1: 22,
    PublicIntakeAcceptedV1: 2,
    PublicIntakeErrorV1: 3,
  };
  for (const [name, last] of Object.entries(expected)) {
    const numbers = [...block(proto, "message", name).matchAll(/=\s*(\d+)\s*;/g)].map((match) => Number(match[1]));
    assert.equal(numbers.at(-1), last, `${name} final field number`);
    assert.deepEqual(numbers, Array.from({ length: last }, (_, index) => index + 1), `${name} has no reused or skipped v1 tags`);
  }
});
