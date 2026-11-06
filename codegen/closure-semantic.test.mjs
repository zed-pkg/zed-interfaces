import assert from 'node:assert/strict';
import {test} from 'node:test';
import {readFileSync} from 'node:fs';
import {normalizeClosure,utf16Compare} from './closure-semantic.mjs';
const fixture=JSON.parse(readFileSync(new URL('../validation/package-security/instances/PackageDependencyClosure/valid/hex-closure.json',import.meta.url),'utf8'));
test('UTF-16 collation differs from UTF-8 for astral versus BMP private-use',()=>{
 assert.equal(utf16Compare('\u{10000}','\uE000'),-1);
 const a=structuredClone(fixture);
 a.dependencies[0].package_name='\uE000';
 a.dependencies[1].package_name='\u{10000}';
 const one=normalizeClosure(a);
 const reverse=normalizeClosure({...a,dependencies:[...a.dependencies].reverse()});
 assert.equal(one.digest,reverse.digest);
 assert.equal(one.preimage,reverse.preimage);
 assert.equal(one.normalized.dependencies[0].package_name,'\u{10000}');
 assert.equal(one.digest,'84efe3277da20763fbc5134d9568390493d6b4f7faffd02e4f47e66302af68a4');
});
test('duplicate and root identities fail instead of disappearing',()=>{
 const duplicate={...fixture,dependencies:[fixture.dependencies[0],structuredClone(fixture.dependencies[0])]};
 assert.throws(()=>normalizeClosure(duplicate),/duplicate_dependency/);
 const root={...fixture,dependencies:[fixture.root]};
 assert.throws(()=>normalizeClosure(root),/root_repeated/);
});
test('lock and resolver metadata are in the semantic digest',()=>{
 const a=normalizeClosure(fixture);
 assert.notEqual(a.digest,normalizeClosure({...fixture,resolver_version:'9.9.9'}).digest);
 assert.notEqual(a.digest,normalizeClosure({...fixture,dependency_lock_digest:'f'.repeat(64)}).digest);
});

test('published Hex golden preimage is byte-for-byte JCS and SHA-256 matched',()=>{
 const expected=readFileSync(new URL('../validation/package-security/golden/hex-closure.canonical-preimage.json',import.meta.url),'utf8').trimEnd();
 const receipt=normalizeClosure(fixture);
 assert.equal(receipt.digest,'2f704cfa7c9f59894d97d4b2552e7dec0e6bdee6852a70b1dca8cd50b9e78684');
 assert.equal(receipt.digest,fixture.closure_digest);
 assert.equal(receipt.preimage,expected);
 assert.equal(Buffer.byteLength(receipt.preimage,'utf8'),1550);
});
