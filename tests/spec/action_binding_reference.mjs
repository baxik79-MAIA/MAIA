// Conformance oracle only. Not a production payload canonicalizer or execution API.
import { jcs } from '../../tools/jcs_reference.mjs';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import assert from 'node:assert/strict';

const fields = ['schema','action_id','action_version','plan_id','plan_version','ordinal',
  'action_type','input_hash','input_canonicalizer','connector_selection','connector_profile_id',
  'connector_binding_hash','tool_definition_fingerprint','risk_class','source_preconditions'];
const sourceFields = ['external_id','source_version_token','normalized_payload_hash'];
const risks = ['read','analyze','draft','write_internal','send_internal','send_external','destructive','privileged'];
function require(ok) { if (!ok) throw new Error('Invalid binding fixture'); }
function keys(value, expected) {
  require(value !== null && typeof value === 'object' && !Array.isArray(value));
  require(Object.keys(value).length === expected.length && expected.every(k => Object.hasOwn(value,k)));
}
function version(v) { require(typeof v === 'string' && /^[1-9][0-9]*$/.test(v) && BigInt(v).toString() === v && BigInt(v) <= 18446744073709551615n); }
function uuid(v) { require(typeof v === 'string' && v.length === 36 && /^[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(v)); }
function hash(v) { require(typeof v === 'string' && v.length === 64 && /^[0-9a-f]{64}$/.test(v)); }
function text(v) { require(typeof v === 'string' && v.length > 0); }
function compare(a,b) {
  for (const key of sourceFields) {
    const n = Buffer.compare(Buffer.from(a[key], 'utf8'),Buffer.from(b[key], 'utf8'));
    if (n) return n;
  }
  return 0;
}
export function actionBinding(value) {
  keys(value,fields);
  require(value.schema === 'maia.action-binding.v1');
  uuid(value.action_id); uuid(value.plan_id);
  version(value.action_version); version(value.plan_version);
  require(Number.isInteger(value.ordinal) && value.ordinal >= 0 && value.ordinal <= 4294967295);
  require(typeof value.action_type === 'string' && !/\s/.test(value.action_type) && /^[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)+$/.test(value.action_type));
  hash(value.input_hash);
  keys(value.input_canonicalizer,['id','version']); text(value.input_canonicalizer.id); version(value.input_canonicalizer.version);
  require(risks.includes(value.risk_class));
  require(['none','fixed','policy_routed'].includes(value.connector_selection));
  if (value.connector_profile_id !== null) uuid(value.connector_profile_id);
  if (value.connector_binding_hash !== null) hash(value.connector_binding_hash);
  if (value.tool_definition_fingerprint !== null) hash(value.tool_definition_fingerprint);
  if (value.connector_selection === 'none') require(value.connector_profile_id === null && value.connector_binding_hash === null);
  if (value.connector_selection === 'fixed') require(value.connector_profile_id !== null && value.connector_binding_hash !== null);
  if (value.connector_selection === 'policy_routed') require(['read','analyze','draft'].includes(value.risk_class));
  require(Array.isArray(value.source_preconditions));
  if (value.source_preconditions.length) require(value.connector_profile_id !== null);
  for (const s of value.source_preconditions) {
    keys(s,sourceFields); text(s.external_id); text(s.source_version_token); hash(s.normalized_payload_hash);
    jcs(s); // Reject invalid Unicode before byte sorting, which would otherwise replace surrogates.
  }
  const sources = [...value.source_preconditions].sort(compare);
  for (let i=1;i<sources.length;i++) require(compare(sources[i-1],sources[i]) !== 0);
  const canonical = jcs({...value,source_preconditions:sources});
  return {canonical,sha256:createHash('sha256').update(canonical,'utf8').digest('hex')};
}

if (process.argv.includes('--self-test')) {
  const fixtures = JSON.parse(fs.readFileSync(new URL('./action_binding_vectors.json',import.meta.url),'utf8'));
  let checks=0;
  for (const vector of fixtures.valid) {
    assert.deepEqual(actionBinding(vector.input),{canonical:vector.canonical,sha256:vector.sha256}); checks++;
  }
  const base=fixtures.valid[0].input;
  assert.notEqual(fixtures.valid[1].sha256,fixtures.valid[2].sha256);checks++;
  const canonicalizerId=structuredClone(base);canonicalizerId.input_canonicalizer.id='another.canonicalizer';
  assert.notEqual(actionBinding(canonicalizerId).sha256,fixtures.valid[0].sha256);checks++;
  for (const key of sourceFields) {
    const input=structuredClone(base);
    input.source_preconditions[0][key] = key === 'normalized_payload_hash' ? 'e'.repeat(64) : 'changed';
    assert.notEqual(actionBinding(input).sha256,fixtures.valid[0].sha256);checks++;
  }
  for (const key of ['action_version','plan_version','action_type','input_hash','action_id']) {
    const input=structuredClone(base);input[key]+='\n';assert.throws(()=>actionBinding(input));checks++;
  }
  for (const [field,value] of Object.entries(fixtures.mutations)) {
    const input=structuredClone(base);input[field]=value;
    assert.notEqual(actionBinding(input).sha256,fixtures.valid[0].sha256,field);checks++;
  }
  for (const field of fields) {
    const input=structuredClone(base);delete input[field];assert.throws(()=>actionBinding(input),field);checks++;
  }
  for (const vector of fixtures.invalid) {
    const input={...structuredClone(base),...vector.changes};assert.throws(()=>actionBinding(input),vector.name);checks++;
  }
  const reverse=structuredClone(base);reverse.source_preconditions.reverse();
  assert.equal(actionBinding(reverse).sha256,fixtures.valid[0].sha256);checks++;
  const duplicate=structuredClone(base);duplicate.source_preconditions.push(duplicate.source_preconditions[0]);
  assert.throws(()=>actionBinding(duplicate));checks++;
  const unknown=structuredClone(base);unknown.state='running';assert.throws(()=>actionBinding(unknown));checks++;
  const invalidUnicode=structuredClone(base);invalidUnicode.source_preconditions[0].external_id='\ud800';
  assert.throws(()=>actionBinding(invalidUnicode));checks++;
  console.log(`ACTION BINDING SELF-TEST OK: ${checks} checks`);
}
