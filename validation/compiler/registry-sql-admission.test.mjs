import assert from 'node:assert/strict';
import test from 'node:test';
import { inspectRegistryPlacement, REQUIRED_TABLES } from './registry-sql-admission.mjs';

function fixture() {
  const $defs = {};
  const models = {};
  const tables = {};
  REQUIRED_TABLES.forEach((table, index) => {
    const name = `RegistryRow${index}`;
    $defs[name] = { type: 'object', properties: { id: { type: 'string' } } };
    models[name] = { table };
    tables[table] = { id: { data_type: 'uuid', nullable: false } };
  });
  return { authority: { $defs, 'x-orm': { schema: 'public', models } },
    catalog: { schema: 'public', tables } };
}
const codes = ({ authority, catalog }) => inspectRegistryPlacement(authority, catalog).findings.map((x) => x.code);

test('complete explicit table/column inventory passes', () => {
  const f = fixture();
  assert.equal(inspectRegistryPlacement(f.authority, f.catalog).status, 'passed');
});
test('does not silently equate zed_pkg and public', () => {
  const f = fixture(); f.authority['x-orm'].schema = 'zed_pkg';
  assert.ok(codes(f).includes('schema-mismatch'));
});
test('does not invent a zed_ prefix for an authored table', () => {
  const f = fixture(); f.authority['x-orm'].models.RegistryRow0.table = 'users';
  assert.ok(codes(f).includes('table-missing'));
});
test('requires the CLI SQL entity', () => {
  const f = fixture(); delete f.catalog.tables.zed_clis;
  assert.ok(codes(f).includes('required-sql-table-missing'));
});
test('requires the CLI authority model', () => {
  const f = fixture(); delete f.authority.$defs.RegistryRow9;
  delete f.authority['x-orm'].models.RegistryRow9;
  assert.ok(codes(f).includes('required-table-unmodeled'));
});
test('does not silently normalize column names', () => {
  const f = fixture(); f.authority.$defs.RegistryRow0.properties.createdAt = { type: 'string' };
  f.catalog.tables.zed_users.created_at = { data_type: 'timestamp with time zone', nullable: false };
  assert.ok(codes(f).includes('column-unmapped'));
  assert.ok(codes(f).includes('column-unmodeled'));
});
test('rejects one-sided annotations and duplicate mappings', () => {
  const f = fixture(); delete f.authority['x-orm'].models.RegistryRow0;
  f.authority['x-orm'].models.Ghost = { table: 'zed_ghost' };
  f.authority['x-orm'].models.RegistryRow2.table = 'zed_orgs';
  assert.ok(codes(f).includes('model-without-table'));
  assert.ok(codes(f).includes('table-without-model'));
  assert.ok(codes(f).includes('duplicate-table-mapping'));
});
test('refuses missing catalog or metadata instead of passing empty coverage', () => {
  assert.throws(() => inspectRegistryPlacement({}, {}), TypeError);
  const f = fixture(); f.catalog.tables = [];
  assert.throws(() => inspectRegistryPlacement(f.authority, f.catalog), TypeError);
});
