import assert from 'node:assert/strict';
import { cp, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import test from 'node:test';
import { checkPublicParity, validateCorpus, DECLARATIONS } from './public-parity.mjs';

const root = resolve(import.meta.dirname, '../..');
const validatorRoot = join(root, '.deps/typespec-json-schema-validator');
const corpus = JSON.parse(await readFile(join(import.meta.dirname, 'cases.json'), 'utf8'));

test('recorded corpus covers both verdicts for every production declaration', () => {
  assert.equal(validateCorpus(corpus), corpus);
  assert.equal(corpus.cases.length, 34);
});
for (const [name, change, message] of [
  ['unknown declaration', (v) => { v.cases[0].declaration = 'Missing'; }, /unknown corpus declaration/],
  ['unsafe fixture path', (v) => { v.cases[0].id = '../escape'; }, /unsafe corpus identity/],
  ['duplicate fixture', (v) => { v.cases.push(v.cases[0]); }, /duplicate corpus identity/],
  ['unstated verdict', (v) => { delete v.cases[0].valid; }, /expected verdict/],
  ['missing negative evidence', (v) => { v.cases = v.cases.filter((x) => x.valid); }, /both valid and invalid/],
]) test(`rejects ${name}`, () => {
  const value = structuredClone(corpus); change(value);
  assert.throws(() => validateCorpus(value), message);
});

test('real production authorities compile and verify a complete immutable IR', async () => {
  const result = await checkPublicParity();
  assert.equal(result.contractIr.admission.scope.complete, true);
  assert.deepEqual(result.contractIr.declarations.map((x) => x.id).sort(), DECLARATIONS.map((x) => `Zed.Validation.${x}`).sort());
  assert.ok(Object.isFrozen(result.cases));
  assert.ok(Object.isFrozen(result.contractIr));
});

for (const [name, path, mutate] of [
  ['one-sided schema constraint drift', 'validation/public-contracts.v1.json', (text) => {
    const doc = JSON.parse(text); doc.$defs.RequestMeta.properties.requestId.minLength = 2; return JSON.stringify(doc);
  }],
  ['unmapped TypeSpec declaration', 'validation/typespec/validation.tsp', (text) => `${text}\nmodel Unreviewed { value: string; }\n`],
  ['incorrect recorded expectation', 'validation/compiler/cases.json', (text) => {
    const doc = JSON.parse(text); doc.cases.find((x) => x.id === 'string-number').valid = true; return JSON.stringify(doc);
  }],
]) test(`real compiler gate rejects ${name}`, async (t) => {
  const repositoryRoot = await mkdtemp(join(tmpdir(), 'zed-public-mutation-'));
  t.after(() => rm(repositoryRoot, { recursive: true, force: true }));
  await cp(join(root, 'validation'), join(repositoryRoot, 'validation'), { recursive: true });
  const target = join(repositoryRoot, path);
  await writeFile(target, mutate(await readFile(target, 'utf8')));
  await assert.rejects(checkPublicParity({ repositoryRoot, validatorRoot }), /production public parity stopped/);
});
