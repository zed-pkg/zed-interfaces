import {createHash} from 'node:crypto';
const tupleKeys=['ecosystem','registry_uri','package_name','package_version','resolved_revision','artifact_digest','source_digest','outer_checksum'];
const scalar=(v)=>v===undefined||v===null?'':v;
export const utf16Compare=(a,b)=>a<b?-1:a>b?1:0;
function canonical(value) {
  if(Array.isArray(value))return '['+value.map(canonical).join(',')+']';
  if(value && typeof value==='object')return '{'+Object.keys(value).sort(utf16Compare).map(k=>JSON.stringify(k)+':'+canonical(value[k])).join(',')+'}';
  if(typeof value==='number' && !Number.isFinite(value))throw Error('nonfinite');
  return JSON.stringify(value);
}
const key=(d)=>tupleKeys.map(k=>scalar(d[k]));
export function normalizeClosure(closure) {
  if(!closure||!Array.isArray(closure.dependencies)||!closure.root)throw Error('invalid_closure');
  const dependencies=closure.dependencies.map(d=>structuredClone(d));
  for(const o of [closure.root,...dependencies]) {
    for(const k of Object.keys(o))if(k.endsWith('_digest')||k==='outer_checksum') {
      if(typeof o[k]!=='string')throw Error('invalid_digest');
      o[k]=o[k].toLowerCase();
    }
  }
  const root=structuredClone(closure.root);
  for(const k of Object.keys(root))if(k.endsWith('_digest')||k==='outer_checksum')root[k]=root[k].toLowerCase();
  const compare=(a,b)=>{
    const ka=key(a),kb=key(b);
    for(let i=0;i<ka.length;i++){const c=utf16Compare(ka[i],kb[i]);if(c)return c;}
    return 0;
  };
  dependencies.sort(compare);
  for(let i=1;i<dependencies.length;i++)if(compare(dependencies[i-1],dependencies[i])===0)throw Error('duplicate_dependency');
  if(dependencies.some(d=>compare(d,root)===0))throw Error('root_repeated');
  const out={...closure,root,dependencies};
  delete out.closure_digest;
  if(typeof out.dependency_lock_digest==='string')out.dependency_lock_digest=out.dependency_lock_digest.toLowerCase();
  const preimage=canonical(out);
  const digest=createHash('sha256').update(Buffer.from(preimage,'utf8')).digest('hex');
  return {preimage,digest,normalized:{...out,closure_digest:digest}};
}
