import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { before, test } from 'node:test';
import { emitTypeSpecJsonSchema, SchemaResolver, validateInstance } from '../tmp/tsjsv/src/index.mjs';

// Independently specified JSON-wire expectations. Neither authority generates this corpus.
const registration = () => ({ repository: 'fixture/interfaces', authority: 'typespec', generatedWitnessOnly: true });
const cases = [];
const add = (name, declaration, instance, expected) => cases.push({ name, declaration, instance, expected });
for (const value of ['typespec', 'json-schema']) add(`authority accepts ${value}`, 'AuthorityKind', value, true);
for (const value of ['', 'TypeSpec', 'protobuf', null, false, 0, [], {}]) {
  add(`authority rejects ${JSON.stringify(value)}`, 'AuthorityKind', value, false);
}
for (const authority of ['typespec', 'json-schema']) {
  for (const generatedWitnessOnly of [true, false]) {
    add(`registration ${authority} flag=${generatedWitnessOnly}`, 'PeerAuthorityRegistration', { ...registration(), authority, generatedWitnessOnly }, true);
  }
}
// No minLength or const restriction exists in these authorities. Do not silently
// introduce one through tests: empty strings and false remain shape-valid.
for (const [name, patch] of [
  ['empty repository', { repository: '' }], ['empty optional owner', { owner: '' }],
  ['Unicode owner', { owner: '\u{1f680} e\u0301' }], ['Unicode repository', { repository: '\u00e9/e\u0301' }],
]) add(name, 'PeerAuthorityRegistration', { ...registration(), ...patch }, true);
for (const field of ['repository', 'authority', 'generatedWitnessOnly']) {
  const value = registration(); delete value[field];
  add(`missing required ${field}`, 'PeerAuthorityRegistration', value, false);
}
for (const field of ['repository', 'authority', 'generatedWitnessOnly', 'owner']) {
  add(`explicit null ${field}`, 'PeerAuthorityRegistration', { ...registration(), [field]: null }, false);
}
for (const [field, values] of [
  ['repository', [false, 0, [], {}]], ['owner', [false, 0, [], {}]],
  ['authority', ['TypeSpec', 'protobuf', '', false, 0, [], {}]],
  ['generatedWitnessOnly', [0, 1, 'false', [], {}]],
]) for (const value of values) add(`${field} rejects ${JSON.stringify(value)}`, 'PeerAuthorityRegistration', { ...registration(), [field]: value }, false);
for (const key of ['extra', '__proto__', 'constructor', 'toString', 'hasOwnProperty']) {
  // A JSON parse is essential: __proto__ in an object literal is not an own member.
  const value = JSON.parse(JSON.stringify({ ...registration(), [key]: { polluted: true } }));
  add(`unknown own member ${key}`, 'PeerAuthorityRegistration', value, false);
}
for (const value of [null, false, 0, '', []]) add(`registration rejects root ${JSON.stringify(value)}`, 'PeerAuthorityRegistration', value, false);

let lanes;
function lane(document, path) {
  const resolver = new SchemaResolver([{ path, document }]);
  const base = resolver.documents[0].base;
  return (declaration, instance) => validateInstance({
    schema: { $ref: `#/$defs/${declaration}` }, instance, resolver, base,
  }).valid;
}
function freeze(value) {
  if (value && typeof value === 'object') {
    for (const child of Object.values(value)) freeze(child);
    Object.freeze(value);
  }
  return value;
}
before(async () => {
  const authoredPath = resolve('schema-authority-canary/authored.schema.json');
  const emitted = await emitTypeSpecJsonSchema({
    entry: resolve('schema-authority-canary/main.tsp'),
    outputDir: resolve('tmp/peer-wire/generated'),
    bundleId: 'wire.generated.schema.json', sealObjectSchemas: true,
  });
  lanes = [
    ['authored JSON Schema A', lane(JSON.parse(await readFile(authoredPath, 'utf8')), authoredPath)],
    ['official TypeSpec comparison witness B', lane(JSON.parse(await readFile(emitted.generatedPath, 'utf8')), emitted.generatedPath)],
  ];
});

test('independent wire corpus is unique, JSON-only, and covers both declarations and verdicts', () => {
  assert.equal(cases.length, 55);
  assert.equal(new Set(cases.map(item => item.name)).size, cases.length);
  for (const declaration of ['AuthorityKind', 'PeerAuthorityRegistration']) {
    for (const expected of [true, false]) assert.ok(cases.some(item => item.declaration === declaration && item.expected === expected));
  }
  for (const item of cases) assert.deepEqual(JSON.parse(JSON.stringify(item.instance)), item.instance);
});
for (const { name, declaration, instance, expected } of cases) {
  test(`both independent schema lanes: ${name}`, () => {
    for (const [label, accepts] of lanes) {
      const input = JSON.parse(JSON.stringify(instance));
      const before = JSON.parse(JSON.stringify(input));
      freeze(input);
      assert.equal(accepts(declaration, input), expected, label);
      assert.deepEqual(input, before, `${label} must not coerce, default, strip or normalize`);
    }
  });
}

test('reserved-member negative is a real own JSON property, not a modified prototype', () => {
  const { instance } = cases.find(item => item.name === 'unknown own member __proto__');
  assert.equal(Object.hasOwn(instance, '__proto__'), true);
  assert.equal(Object.getPrototypeOf(instance), Object.prototype);
  assert.equal(Object.hasOwn(Object.prototype, 'polluted'), false);
});
