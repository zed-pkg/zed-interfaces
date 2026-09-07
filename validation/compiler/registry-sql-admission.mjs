import { createHash } from 'node:crypto';
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

export const REQUIRED_TABLES = Object.freeze([
  'zed_users', 'zed_orgs', 'zed_projects', 'zed_packages',
  'zed_package_versions', 'zed_package_licenses', 'zed_entity_embeddings',
  'zed_package_uploads', 'zed_package_downloads', 'zed_clis',
]);

const record = (value) => value !== null && typeof value === 'object' && !Array.isArray(value);

// Inventory only. Passing this gate does NOT certify SQL types, constraints,
// RLS, ORM behavior, or migration safety. Those need database/runtime tests.
// In particular, never guess camelCase -> snake_case or a schema/table alias.
export function inspectRegistryPlacement(authority, catalog) {
  if (!record(authority?.$defs) || !record(authority?.['x-orm']?.models)
      || typeof authority['x-orm'].schema !== 'string'
      || typeof catalog?.schema !== 'string' || !record(catalog?.tables)) {
    throw new TypeError('Missing explicit authority metadata or PostgreSQL catalog');
  }
  const findings = [];
  const add = (code, detail) => findings.push({ code, ...detail });
  const metadata = authority['x-orm'];
  if (metadata.schema !== catalog.schema) {
    add('schema-mismatch', { authored: metadata.schema, actual: catalog.schema });
  }
  const covered = new Set();
  for (const model of Object.keys(authority.$defs).sort()) {
    const definition = authority.$defs[model];
    const mapping = metadata.models[model];
    if (!record(mapping) || typeof mapping.table !== 'string') {
      add('model-without-table', { model });
      continue;
    }
    const table = mapping.table;
    if (covered.has(table)) add('duplicate-table-mapping', { model, table });
    covered.add(table);
    if (!Object.hasOwn(catalog.tables, table)) {
      add('table-missing', { model, table });
      continue;
    }
    if (!record(catalog.tables[table]) || !record(definition?.properties)) {
      throw new TypeError(`Malformed column inventory for ${model}`);
    }
    for (const column of Object.keys(definition.properties).sort()) {
      if (!Object.hasOwn(catalog.tables[table], column)) {
        add('column-unmapped', { model, table, column });
      }
    }
    for (const column of Object.keys(catalog.tables[table]).sort()) {
      if (!Object.hasOwn(definition.properties, column)) {
        add('column-unmodeled', { model, table, column });
      }
    }
  }
  for (const model of Object.keys(metadata.models).sort()) {
    if (!Object.hasOwn(authority.$defs, model)) add('table-without-model', { model });
  }
  for (const table of REQUIRED_TABLES) {
    if (!Object.hasOwn(catalog.tables, table)) add('required-sql-table-missing', { table });
    if (!covered.has(table)) add('required-table-unmodeled', { table });
  }
  return {
    schema: 'zed.registry-sql-admission/v1',
    scope: 'schema-table-column-inventory-only',
    status: findings.length ? 'stopped_for_evaluation' : 'passed',
    findings,
  };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const output = '.artifacts/registry-sql-admission.json';
  try {
    const authored = readFileSync('validation/authorities/server/contracts.json', 'utf8');
    const observed = readFileSync('.artifacts/registry-catalog.json', 'utf8');
    const report = inspectRegistryPlacement(JSON.parse(authored), JSON.parse(observed));
    report.inputDigests = {
      authority: createHash('sha256').update(authored).digest('hex'),
      catalog: createHash('sha256').update(observed).digest('hex'),
    };
    mkdirSync('.artifacts', { recursive: true });
    writeFileSync(output, `${JSON.stringify(report, null, 2)}\n`);
    console.log(JSON.stringify(report));
    process.exitCode = report.status === 'passed' ? 0 : 2;
  } catch (error) {
    console.error(`registry SQL admission failed: ${error.message}`);
    process.exitCode = 3;
  }
}
