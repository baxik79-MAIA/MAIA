#!/usr/bin/env node
import fs from 'node:fs';
import { pathToFileURL } from 'node:url';

function validUnicodeString(s) {
  for (let i=0; i<s.length; i++) {
    const c=s.charCodeAt(i);
    if (c>=0xD800 && c<=0xDBFF) {
      if (i+1>=s.length) return false;
      const n=s.charCodeAt(++i);
      if (!(n>=0xDC00 && n<=0xDFFF)) return false;
    } else if (c>=0xDC00 && c<=0xDFFF) return false;
  }
  return true;
}
export function jcs(v) {
  let out='';
  function ser(x) {
    if (x === null || typeof x !== 'object') {
      if (typeof x === 'number' && !Number.isFinite(x)) throw new Error('JCS forbids NaN/Infinity');
      if (typeof x === 'string' && !validUnicodeString(x)) throw new Error('JCS forbids lone surrogates');
      const z=JSON.stringify(x);
      if (z === undefined) throw new Error('Unsupported JSON value');
      out += z; return;
    }
    if (Array.isArray(x)) {
      out += '[';
      for (let i=0;i<x.length;i++) { if(i) out+=','; ser(x[i]); }
      out += ']'; return;
    }
    out += '{';
    const keys=Object.keys(x);
    for (const k of keys) if (!validUnicodeString(k)) throw new Error('JCS forbids lone surrogates');
    keys.sort(); // ECMAScript UTF-16 code-unit lexicographic order
    keys.forEach((k,i)=>{ if(i) out+=','; out+=JSON.stringify(k)+':'; ser(x[k]); });
    out += '}';
  }
  ser(v); return out;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href && process.argv.includes('--self-test')) {
  const idx=process.argv.indexOf('--self-test');
  const path=process.argv[idx+1];
  const vectors=JSON.parse(fs.readFileSync(path,'utf8'));
  let bad=0;
  for (const v of vectors) {
    const got=jcs(v.input);
    if (got!==v.canonical) { console.error(`JCS FAIL ${v.name}\nexpected: ${v.canonical}\ngot:      ${got}`); bad++; }
  }
  if (bad) process.exit(1);
  console.log(`JCS RFC8785 SELF-TEST OK: ${vectors.length} vector(s)`);
}
