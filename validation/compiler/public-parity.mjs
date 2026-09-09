import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { promisify } from 'node:util';

const exec = promisify(execFile);
export const VALIDATOR_REVISION = '4473504c4c9d2831d825919f70c03994d8ce01d2';
export const PUBLIC_MODELS = Object.freeze(['RequestMeta', 'PageQuery', 'ProblemDetails']);
export const DECLARATIONS = Object.freeze([...PUBLIC_MODELS, 'PublicValidationContract']);
const here = resolve(import.meta.dirname, '../..');
const hash = (bytes) => createHash('sha256').update(bytes).digest('hex');
const json = async (path) => JSON.parse(await readFile(path, 'utf8'));

export function validateCorpus(corpus) {
  assert.equal(corpus?.schema, 'zed.public-validation-corpus/v1');
  assert.ok(Array.isArray(corpus.cases) && corpus.cases.length > 0, 'recorded corpus must not be empty');
  const seen = new Set();
  for (const item of corpus.cases) {
    assert.ok(item && DECLARATIONS.includes(item.declaration), 'unknown corpus declaration');
    assert.match(item.id, /^[a-z0-9][a-z0-9-]{0,79}$/u, 'unsafe corpus identity');
    assert.equal(typeof item.valid, 'boolean', 'corpus must record an expected verdict');
    assert.ok(Object.hasOwn(item, 'value'), 'corpus value is missing');
    const key = `${item.declaration}/${item.id}`;
    assert.ok(!seen.has(key), `duplicate corpus identity: ${key}`);
    seen.add(key);
  }
  for (const name of DECLARATIONS) for (const valid of [true, false]) {
    assert.ok(corpus.cases.some((item) => item.declaration === name && item.valid === valid),
      `${name} needs both valid and invalid recorded evidence`);
  }
  return corpus;
}

function deepFreeze(value) {
  if (value && typeof value === 'object') {
    for (const child of Object.values(value)) deepFreeze(child);
    Object.freeze(value);
  }
  return value;
}

/** Run the real compiler over existing production authorities, not a canary.
 * Each call owns a fresh disposable workspace. No previous receipt is reused.
 * The returned evidence is immutable and valid only for this exact checked input.
 */
export async function checkPublicParity({ repositoryRoot = here,
  validatorRoot = join(here, '.deps/typespec-json-schema-validator') } = {}) {
  const root = resolve(repositoryRoot);
  const tool = resolve(validatorRoot);
  assert.equal((await exec('git', ['-C', tool, 'rev-parse', 'HEAD'])).stdout.trim(), VALIDATOR_REVISION,
    'validator must be the reviewed immutable revision');
  await exec('git', ['-C', tool, 'diff', '--exit-code', 'HEAD', '--', 'src', 'bin', '.cli-flags.toml', 'package.json', 'package-lock.json']);
  const manifest = await json(join(root, 'validation/parity/manifest.v2.json'));
  assert.deepEqual([...manifest.scopes.isomorphic.models].sort(), [...PUBLIC_MODELS].sort(), 'production public-model inventory changed');
  assert.equal(manifest.authorities.typespec, 'validation/typespec/validation.tsp');
  assert.equal(manifest.authorities.jsonSchema, 'validation/public-contracts.v1.json');
  const typespec = join(root, manifest.authorities.typespec);
  const authoredSchema = join(root, manifest.authorities.jsonSchema);
  const corpusBytes = await readFile(join(root, 'validation/compiler/cases.json'));
  const corpus = validateCorpus(JSON.parse(corpusBytes));
  const work = await mkdtemp(join(tmpdir(), 'zed-public-contract-'));
  try {
    const instances = join(work, 'instances');
    for (const item of corpus.cases) {
      const directory = join(instances, item.declaration, item.valid ? 'valid' : 'invalid');
      await mkdir(directory, { recursive: true });
      await writeFile(join(directory, `${item.id}.json`), `${JSON.stringify(item.value)}\n`, { flag: 'wx' });
    }
    const reportPath = join(work, 'report.json');
    const irPath = join(work, 'contract-ir.json');
    const generated = join(work, 'witness');
    const args = ['check', `--typespec=${typespec}`, `--schema=${authoredSchema}`,
      `--instances=${instances}`, `--output-dir=${generated}`, `--report=${reportPath}`,
      `--contract-ir=${irPath}`, '--seal-object-schemas=false', '--probes=true',
      '--max-probes=64', '--format-assertion=true', '--quiet'];
    const env = Object.fromEntries(Object.entries(process.env).filter(([key]) => !key.startsWith('TSJSV_')));
    try {
      await exec(process.execPath, [join(tool, 'bin/typespec-json-schema-validator.mjs'), ...args],
        { cwd: tool, env, timeout: 120000, maxBuffer: 2 * 1024 * 1024 });
    } catch (error) {
      let detail;
      try { const report = await json(reportPath); detail = JSON.stringify({ status: report.status, findings: report.findings, error: report.error }); }
      catch { detail = error.message; }
      throw new Error(`production public parity stopped: ${detail}`, { cause: error });
    }
    const [contractIr, report] = await Promise.all([json(irPath), json(reportPath)]);
    const { verifyConsumerContract } = await import(pathToFileURL(join(tool, 'src/consumer-verification.mjs')).href);
    const expectedDeclarations = DECLARATIONS.map((name) => `Zed.Validation.${name}`);
    const verification = await verifyConsumerContract({ contractIr, report, typespec, authoredSchema,
      generatedSchema: join(generated, 'typespec.generated.schema.json'), expectedDeclarations });
    assert.equal(hash(await readFile(join(root, 'validation/compiler/cases.json'))), hash(corpusBytes), 'recorded corpus changed during admission');
    assert.deepEqual(await json(join(root, 'validation/parity/manifest.v2.json')), manifest, 'scope policy changed during admission');
    return deepFreeze({ schema: 'zed.public-contract-parity/v1', validatorRevision: VALIDATOR_REVISION,
      corpusDigest: hash(corpusBytes), cases: corpus.cases, contractIr, report, verification });
  } finally {
    await rm(work, { recursive: true, force: true });
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  try {
    assert.equal(process.argv.length, 2, 'this internal entrypoint accepts no CLI options');
    const result = await checkPublicParity();
    console.log(JSON.stringify({ schema: result.schema, validatorRevision: result.validatorRevision,
      corpusDigest: result.corpusDigest, recordedCases: result.cases.length,
      declarations: result.contractIr.declarations.map((item) => item.id), verification: result.verification }));
  } catch (error) {
    console.error(error.message);
    process.exitCode = 2;
  }
}
