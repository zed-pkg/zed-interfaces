// Tests for the JSON Schema -> Dart/TS generator.
//
//   node --test codegen/generate.test.mjs
//
// The generator is the only thing standing between a Rust type change and a
// front-end that silently decodes the wrong shape, so the cases below pin the
// decisions that are easy to get wrong: nullability, serde defaults, reserved
// identifiers, shared types, and the "nothing is skipped silently" rule.

import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";

import { GenError, build, internals, loadIndex, plan } from "./generate.mjs";

const { buildType, emitDart, emitTs, dartIdent, dartEnumIdent, dartStr, snake } = internals;

/** Minimal single-module plan so the emitters can be exercised in isolation. */
function planOf(name, schema, defs = {}) {
  const collected = [];
  for (const [defName, def] of Object.entries(defs)) collected.push(buildType(defName, def, "t.json", collected));
  collected.push(buildType(name, schema, "t.json", collected));
  const byName = new Map(collected.map((type) => [type.name, { type, modules: new Set(["t"]) }]));
  const home = new Map([...byName.keys()].map((k) => [k, "t"]));
  const modules = new Map([["t", { file: "t.json", targets: ["dart", "ts"], types: collected, root: name }]]);
  return { modules, home, byName };
}

const dartOf = (...args) => emitDart(planOf(...args))["dart/lib/t.dart"];
const tsOf = (...args) => emitTs(planOf(...args))["ts/t.ts"];

const OBJ = (properties, required = []) => ({ type: "object", properties, required });

test("required scalars are non-nullable and decoded directly", () => {
  const dart = dartOf("Thing", OBJ({ org: { type: "string" } }, ["org"]));
  assert.match(dart, /required this\.org,/);
  assert.match(dart, /final String org;/);
  assert.match(dart, /org: json\['org'\] as String,/);
  assert.match(tsOf("Thing", OBJ({ org: { type: "string" } }, ["org"])), /readonly org: string;/);
});

test("a nullable type decodes null-aware rather than through a ternary", () => {
  const dart = dartOf("Thing", OBJ({ latest: { type: ["string", "null"] } }, ["latest"]));
  assert.match(dart, /final String\? latest;/);
  assert.match(dart, /latest: json\['latest'\] as String\?,/);
  assert.doesNotMatch(dart, /latest: json\['latest'\] == null/);
});

test("an optional field with no default is nullable in Dart and optional in TS", () => {
  const schema = OBJ({ tags: { type: "array", items: { type: "string" } } });
  assert.match(dartOf("Thing", schema), /final List<String>\? tags;/);
  assert.match(tsOf("Thing", schema), /readonly tags\?: readonly string\[\];/);
});

test("a serde default keeps the Dart field non-nullable and fills the default in", () => {
  const schema = OBJ({ yanked: { type: "boolean", default: false } });
  const dart = dartOf("Thing", schema);
  assert.match(dart, /this\.yanked = false,/);
  assert.match(dart, /final bool yanked;/);
  assert.match(dart, /yanked: json\['yanked'\] == null/);
  // TS models the wire, where the key really can be absent.
  assert.match(tsOf("Thing", schema), /readonly yanked\?: boolean;/);
});

test("an enum $ref defaults to the matching Dart enum value", () => {
  const dart = dartOf(
    "Thing",
    OBJ({ format: { $ref: "#/$defs/ArtifactFormat", default: "tar.gz" } }),
    { ArtifactFormat: { type: "string", enum: ["tar.gz", "zip"] } },
  );
  assert.match(dart, /this\.format = ArtifactFormat\.tarGz,/);
  assert.match(dart, /tarGz\('tar\.gz'\),/);
  assert.match(dart, /format: json\['format'\] == null\n\s+\? ArtifactFormat\.tarGz/);
});

test("oneOf-of-const is an enum, and per-variant docs survive into both languages", () => {
  const defs = {
    AuditAction: {
      oneOf: [
        { type: "string", const: "publish", description: "A version was published." },
        { type: "string", const: "org_claim", description: "The org was claimed." },
      ],
    },
  };
  const dart = dartOf("Thing", OBJ({ action: { $ref: "#/$defs/AuditAction" } }, ["action"]), defs);
  assert.match(dart, /\/\/\/ A version was published\.\n\s+publish\('publish'\),/);
  assert.match(dart, /orgClaim\('org_claim'\);/);
  const ts = tsOf("Thing", OBJ({ action: { $ref: "#/$defs/AuditAction" } }, ["action"]), defs);
  assert.match(ts, /export type AuditAction = "publish" \| "org_claim";/);
  assert.match(ts, /AUDIT_ACTION_VALUES = \["publish", "org_claim"\] as const;/);
});

test("a nullable enum ref uses maybeFromJson instead of an unchecked cast", () => {
  const dart = dartOf(
    "Thing",
    OBJ({ kind: { anyOf: [{ $ref: "#/$defs/Op" }, { type: "null" }] } }, ["kind"]),
    { Op: { type: "string", enum: ["upsert", "delete"] } },
  );
  assert.match(dart, /kind: Op\.maybeFromJson\(json\['kind'\] as String\?\),/);
  assert.match(dart, /final Op\? kind;/);
});

test("lists of refs round-trip through fromJson/toJson", () => {
  const dart = dartOf(
    "Thing",
    OBJ({ entries: { type: "array", items: { $ref: "#/$defs/Entry" } } }, ["entries"]),
    { Entry: OBJ({ id: { type: "string" } }, ["id"]) },
  );
  assert.match(dart, /entries: \(json\['entries'\] as List<dynamic>\)\.map\(\(e\) => Entry\.fromJson\(e as Map<String, dynamic>\)\)\.toList\(\),/);
  assert.match(dart, /'entries': entries\.map\(\(e\) => e\.toJson\(\)\)\.toList\(\),/);
});

test("maps become Map/Record of the value type", () => {
  const schema = OBJ({ env: { type: "object", additionalProperties: { type: "string" } } }, ["env"]);
  assert.match(dartOf("Thing", schema), /final Map<String, String> env;/);
  assert.match(tsOf("Thing", schema), /readonly env: Readonly<Record<string, string>>;/);
});

test("a `true` schema is opaque JSON, not a guessed type", () => {
  const schema = OBJ({ row: true }, ["row"]);
  assert.match(dartOf("Thing", schema), /final Object\? row;/);
  assert.match(tsOf("Thing", schema), /readonly row: unknown;/);
});

test("`name` is a legal field but an illegal enum value, and only the latter is escaped", () => {
  assert.equal(dartIdent("name"), "name");
  assert.equal(dartEnumIdent("name"), "name_");
  assert.equal(dartIdent("default"), "default_");
  assert.equal(dartIdent("hashCode"), "hashCode_");
  assert.equal(dartIdent("vcs_commit"), "vcsCommit");
  assert.equal(snake("VersionScheme"), "version_scheme");
});

test("Dart string literals escape quotes and interpolation", () => {
  assert.equal(dartStr("it's"), "'it\\'s'");
  assert.equal(dartStr("$value"), "'\\$value'");
});

test("an unsupported construct fails loudly instead of emitting a wrong type", () => {
  assert.throws(() => buildType("T", OBJ({ x: { type: ["string", "integer"] } }, ["x"]), "t.json", []), GenError);
  assert.throws(
    () => buildType("T", OBJ({ x: { anyOf: [{ type: "string" }, { type: "integer" }] } }, ["x"]), "t.json", []),
    GenError,
  );
  assert.throws(
    () => buildType("T", OBJ({ x: { type: "object", properties: { y: { type: "string" } } } }, ["x"]), "t.json", []),
    GenError,
  );
});

// --- the index is the demarcation, so its rules get their own coverage -------

function withSchemaDir(files, fn) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "zed-codegen-"));
  try {
    for (const [name, content] of Object.entries(files)) {
      fs.writeFileSync(path.join(dir, name), typeof content === "string" ? content : JSON.stringify(content));
    }
    return fn(dir);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

test("a schema that is not classified in index.json is an error, not a silent skip", () => {
  withSchemaDir(
    {
      "index.json": { schemas: [{ file: "a.json", targets: ["ts"] }] },
      "a.json": { title: "A", type: "object", properties: {} },
      "b.json": { title: "B", type: "object", properties: {} },
    },
    (dir) => {
      assert.throws(() => loadIndex(dir), (error) => error instanceof GenError && /does not classify: b\.json/.test(error.message));
    },
  );
});

test("index.json rejects unknown targets, missing files, and duplicates", () => {
  withSchemaDir({ "index.json": { schemas: [{ file: "a.json", targets: ["swift"] }] }, "a.json": {} }, (dir) =>
    assert.throws(() => loadIndex(dir), /unknown target "swift"/));
  withSchemaDir({ "index.json": { schemas: [{ file: "gone.json", targets: [] }] } }, (dir) =>
    assert.throws(() => loadIndex(dir), /lists gone\.json, which does not exist/));
  withSchemaDir(
    { "index.json": { schemas: [{ file: "a.json", targets: [] }, { file: "a.json", targets: [] }] }, "a.json": {} },
    (dir) => assert.throws(() => loadIndex(dir), /a\.json is listed twice/),
  );
});

// --- properties of the real repository ---------------------------------------

test("every schema in the repository is classified", () => {
  const entries = loadIndex();
  const files = fs.readdirSync(path.join(import.meta.dirname, "..", "schemas")).filter((f) => f.endsWith(".json") && f !== "index.json");
  assert.equal(entries.length, files.length);
});

test("Rust-only schemas emit nothing, front-end schemas emit both slices", () => {
  const emitted = build();
  const frontEnd = loadIndex().filter((entry) => entry.targets.length);
  for (const entry of frontEnd) {
    const module = entry.file.replace(/\.json$/, "");
    assert.ok(`ts/${module}.ts` in emitted, `missing ts slice for ${entry.file}`);
    assert.ok(`dart/lib/${snake(module)}.dart` in emitted, `missing dart slice for ${entry.file}`);
  }
  for (const entry of loadIndex().filter((e) => !e.targets.length)) {
    const module = entry.file.replace(/\.json$/, "");
    assert.ok(!(`ts/${module}.ts` in emitted), `${entry.file} is Rust-only but emitted a TS slice`);
  }
});

test("a type used by two schemas is defined once, in common", () => {
  const { home } = plan();
  // PackageSummary is reachable from both the browse listing and search.
  assert.equal(home.get("PackageSummary"), "common");
  assert.equal(home.get("AuditEntry"), "audit-log-response");
});
