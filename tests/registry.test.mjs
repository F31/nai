import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {readRegistry,validateRegistry,inspectRegistry,BASELINE} from '../scripts/contracts/registry.mjs';
import {validators} from '../scripts/contracts/validation.mjs';
const text=await readFile(BASELINE,'utf8'),original=await readRegistry();
test('active catalog and engineering context agree with the executing baseline',async()=>{
 const source=JSON.parse(await readFile('schemas/catalog/source.json','utf8'));assert.equal(source.baseline,BASELINE);
 for(const p of ['AGENTS.md','openspec/config.yaml'])assert.ok((await readFile(p,'utf8')).includes(BASELINE),p);
 assert.equal(original.invariants.length,62);assert.equal(original.work_items.length,15);
});
test('ACC schema exact 164 baseline requirements and 62 INV, pending only',async()=>{const v=await validators();assert.equal(v.registry(original),true,JSON.stringify(v.registry.errors));assert.deepEqual(await inspectRegistry(original),[]);assert.equal(original.acceptance.length,164);assert.ok(original.acceptance.every(a=>a.status==='pending'));});
test('holes duplicate legacy and invented requirements fail',()=>{for(const f of [r=>r.acceptance.pop(),r=>r.acceptance.push(r.acceptance[0]),r=>r.acceptance[0].id='S01',r=>r.acceptance[0].expected='fabricated']){const r=structuredClone(original);f(r);assert.ok(validateRegistry(r,text).length);}});
test('INV and task holes or fake tested state fail',()=>{for(const f of [r=>r.invariants.pop(),r=>r.work_items.pop(),r=>r.acceptance[0].work_items=['V48-99'],r=>r.acceptance[0].invariants=['INV-99'],r=>r.acceptance[0].status='tested']){const r=structuredClone(original);f(r);assert.ok(validateRegistry(r,text).length);}});
test('partial bootstrap evidence cannot promote entire ACC',()=>{const r=structuredClone(original);r.acceptance[0].status='tested';r.acceptance[0].evidence=[{kind:'prerequisite',result:'passed',complete:false,environment:'windows',scope:'bootstrap',report:'docs/design.md'}];assert.ok(validateRegistry(r,text).some(e=>e.includes('complete real')));r.acceptance[0].evidence[0].complete=true;assert.ok(validateRegistry(r,text).some(e=>e.includes('prerequisite cannot')));});
test('unsafe or missing report fails',async()=>{const r=structuredClone(original);r.acceptance[0].evidence=[{kind:'prerequisite',result:'passed',complete:false,environment:'portable',scope:'fixture',report:'../secret'}];assert.ok(validateRegistry(r,text).some(e=>e.includes('unsafe')));r.acceptance[0].evidence[0].report='docs/not-a-real-report.md';assert.ok((await inspectRegistry(r)).some(e=>e.includes('missing evidence')));});
test('legacy snapshot bytes and status provenance remain intact',async()=>{const m=JSON.parse(await readFile('tests/legacy/v311/manifest.json','utf8'));for(const [n,r] of Object.entries(m))assert.equal(createHash('sha256').update(await readFile('tests/legacy/v311/'+n)).digest('hex'),r.sha256,n);});
test('V43 source snapshot and all prerequisite evidence remain intact',async()=>{const raw=await readFile('tests/legacy/v43/registry.yaml'),m=JSON.parse(await readFile('tests/legacy/v43/manifest.json','utf8'));assert.equal(createHash('sha256').update(raw).digest('hex'),m['registry.yaml'].sha256);const old=JSON.parse(raw);assert.equal(old.acceptance.length,90);for(const a of old.acceptance){const next=original.acceptance.find(n=>n.id===a.id);assert.ok(next);assert.deepEqual(next.evidence,a.evidence);assert.equal(next.status,'pending');}});
test('simulated environment and false exclusion claims never satisfy real acceptance',()=>{
 for(const environment of ['Simulated','MOCK provider','fixture playback']){const r=structuredClone(original);r.acceptance[0].status='tested';r.acceptance[0].evidence=[{kind:'acceptance',result:'passed',complete:true,environment,scope:'full',report:'docs/design.md'}];assert.ok(validateRegistry(r,text).some(e=>e.includes('complete real')));}
 for(const status of ['not_selected','not_applicable']){const r=structuredClone(original),a=r.acceptance[0];a.status=status;assert.ok(validateRegistry(r,text).some(e=>e.includes('exclusion')));a.condition={expression:'pack.selected',result:'unknown',claimed:false,reason:'unknown'};assert.ok(validateRegistry(r,text).some(e=>e.includes('exclusion')));a.condition.result='false';a.condition.claimed=true;assert.ok(validateRegistry(r,text).some(e=>e.includes('exclusion')));}
});
