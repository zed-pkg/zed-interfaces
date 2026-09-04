#!/usr/bin/env node
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { dirname, resolve } from "node:path";

const root = process.cwd();
const mode = process.argv.find((value) =>
  /^--(?:check|write|self-test)$/.test(value),
);
if (!mode) {
  throw new Error(
    "usage: orm-annotation-check.mjs --check|--write|--self-test",
  );
}

const JSON_AUTHORITY = "validation/authorities/server/contracts.json";
const TYPESPEC_AUTHORITY = "validation/authorities/server/contracts.tsp";
const RECEIPT = "generated/persistence/orm-annotation-receipt.v1.json";
const IDENTIFIER = /^[a-z][a-z0-9_]*$/;
const MODEL_IDENTIFIER = /^[A-Z][A-Za-z0-9]*$/;

function sort(value) {
  if (Array.isArray(value)) {
    return value.map(sort);
  }
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .map((key) => [key, sort(value[key])]),
    );
  }
  return value;
}

function canonical(value) {
  return `${JSON.stringify(sort(value), null, 2)}\n`;
}

function hash(value) {
  return createHash("sha256").update(value).digest("hex");
}

function fail(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function read(relative) {
  return readFileSync(resolve(root, relative), "utf8");
}

function jsonAuthority(document) {
  const metadata = document["x-orm"];
  fail(metadata && typeof metadata === "object", "JSON Schema is missing x-orm");
  return sort(metadata);
}

function typeSpecAuthority(source) {
  let schema;
  const models = {};
  for (const line of source.split(/\r?\n/)) {
    let match = line.match(/^\/\/ @orm-schema (.+)$/);
    if (match) {
      fail(schema === undefined, "TypeSpec contains duplicate @orm-schema");
      const payload = JSON.parse(match[1]);
      schema = payload.schema;
      continue;
    }

    match = line.match(/^\/\/ @orm (.+)$/);
    if (!match) {
      continue;
    }
    const payload = JSON.parse(match[1]);
    const model = payload.model;
    fail(MODEL_IDENTIFIER.test(model ?? ""), "invalid TypeSpec ORM model name");
    fail(!Object.hasOwn(models, model), `duplicate TypeSpec ORM model ${model}`);
    delete payload.model;
    models[model] = payload;
  }
  fail(typeof schema === "string", "TypeSpec is missing @orm-schema");
  return sort({ schema, models });
}

function columnsFor(document, model) {
  const definition = document.$defs?.[model];
  fail(definition?.type === "object", `missing JSON Schema model ${model}`);
  fail(
    definition.additionalProperties === false,
    `${model} must reject additional properties`,
  );
  return new Set(Object.keys(definition.properties ?? {}));
}

function validateColumnList(columns, available, label, allowEmpty = false) {
  fail(Array.isArray(columns), `${label} must be an array`);
  fail(allowEmpty || columns.length > 0, `${label} cannot be empty`);
  fail(
    new Set(columns).size === columns.length,
    `${label} contains duplicate columns`,
  );
  for (const column of columns) {
    fail(available.has(column), `${label} references unknown column ${column}`);
  }
}

function validate(document, metadata) {
  fail(IDENTIFIER.test(metadata.schema ?? ""), "invalid database schema name");
  const modelNames = Object.keys(metadata.models ?? {});
  fail(modelNames.length > 0, "ORM metadata has no models");

  const definitionNames = Object.keys(document.$defs ?? {}).sort();
  fail(
    canonical(modelNames.sort()) === canonical(definitionNames),
    "every server model must have exactly one ORM annotation",
  );

  const tables = new Map();
  const namedObjects = new Set();
  const availableByTable = new Map();

  for (const model of modelNames) {
    fail(MODEL_IDENTIFIER.test(model), `invalid model name ${model}`);
    const config = metadata.models[model];
    fail(IDENTIFIER.test(config.table ?? ""), `${model} has invalid table name`);
    fail(!tables.has(config.table), `duplicate table ${config.table}`);
    tables.set(config.table, model);

    const available = columnsFor(document, model);
    availableByTable.set(config.table, available);
    validateColumnList(config.primaryKey, available, `${model}.primaryKey`);

    for (const [kind, objects] of [
      ["unique", config.unique ?? []],
      ["index", config.indexes ?? []],
    ]) {
      fail(Array.isArray(objects), `${model}.${kind} entries must be an array`);
      for (const object of objects) {
        fail(
          IDENTIFIER.test(object.name ?? ""),
          `${model}.${kind} has invalid name`,
        );
        fail(
          !namedObjects.has(object.name),
          `duplicate database object name ${object.name}`,
        );
        namedObjects.add(object.name);
        validateColumnList(
          object.columns,
          available,
          `${model}.${kind}.${object.name}`,
        );
      }
    }
  }

  for (const model of modelNames) {
    const config = metadata.models[model];
    const available = availableByTable.get(config.table);
    const foreignKeys = config.foreignKeys ?? [];
    fail(Array.isArray(foreignKeys), `${model}.foreignKeys must be an array`);
    for (const foreignKey of foreignKeys) {
      fail(
        IDENTIFIER.test(foreignKey.name ?? ""),
        `${model}.foreignKey has invalid name`,
      );
      fail(
        !namedObjects.has(foreignKey.name),
        `duplicate database object name ${foreignKey.name}`,
      );
      namedObjects.add(foreignKey.name);
      validateColumnList(
        foreignKey.columns,
        available,
        `${model}.foreignKey.${foreignKey.name}`,
      );
      const referenced = foreignKey.references;
      fail(
        referenced && tables.has(referenced.table),
        `${model}.${foreignKey.name} references an unknown table`,
      );
      const target = availableByTable.get(referenced.table);
      validateColumnList(
        referenced.columns,
        target,
        `${model}.foreignKey.${foreignKey.name}.references`,
      );
      fail(
        foreignKey.columns.length === referenced.columns.length,
        `${model}.${foreignKey.name} has mismatched column arity`,
      );
      fail(
        ["cascade", "restrict", "set-null", "no-action"].includes(
          foreignKey.onDelete,
        ),
        `${model}.${foreignKey.name} has unsupported onDelete`,
      );
    }
  }
}

function compare(document, typeSpecSource) {
  const left = jsonAuthority(document);
  const right = typeSpecAuthority(typeSpecSource);
  validate(document, left);
  fail(
    canonical(left) === canonical(right),
    "TypeSpec and JSON Schema ORM annotations disagree",
  );
  return left;
}

function receipt(metadata) {
  return {
    receiptVersion: "zed.orm-annotations.v1",
    repository: "zed-pkg/zed-interfaces",
    schema: metadata.schema,
    models: Object.keys(metadata.models).sort(),
    authorities: {
      jsonSchema: JSON_AUTHORITY,
      typespec: TYPESPEC_AUTHORITY,
    },
    semanticDigest: hash(canonical(metadata)),
    agreement: true,
  };
}

function run() {
  const document = JSON.parse(read(JSON_AUTHORITY));
  const metadata = compare(document, read(TYPESPEC_AUTHORITY));
  const output = canonical(receipt(metadata));
  const destination = resolve(root, RECEIPT);

  if (mode === "--write") {
    mkdirSync(dirname(destination), { recursive: true });
    writeFileSync(destination, output);
    console.log(`wrote ${RECEIPT}`);
    return;
  }

  fail(existsSync(destination), `missing ${RECEIPT}`);
  fail(readFileSync(destination, "utf8") === output, `stale ${RECEIPT}`);
  console.log(
    `verified ORM annotation parity; digest=${receipt(metadata).semanticDigest}`,
  );
}

function selfTest() {
  const document = {
    $defs: {
      Row: {
        type: "object",
        additionalProperties: false,
        required: ["id"],
        properties: { id: { type: "string" } },
      },
    },
    "x-orm": {
      schema: "example",
      models: {
        Row: {
          table: "rows",
          primaryKey: ["id"],
          unique: [],
          foreignKeys: [],
          indexes: [],
        },
      },
    },
  };
  const source = [
    "// @orm-schema {\"schema\":\"example\"}",
    "// @orm {\"foreignKeys\":[],\"indexes\":[],\"model\":\"Row\",\"primaryKey\":[\"id\"],\"table\":\"rows\",\"unique\":[]}",
    "model Row { id: string; }",
  ].join("\n");
  compare(document, source);

  const drifted = source.replace('"table":"rows"', '"table":"other_rows"');
  let rejected = false;
  try {
    compare(document, drifted);
  } catch {
    rejected = true;
  }
  fail(rejected, "self-test failed to detect annotation drift");
  console.log("ORM annotation parity self-tests passed");
}

if (mode === "--self-test") {
  selfTest();
} else {
  run();
}
