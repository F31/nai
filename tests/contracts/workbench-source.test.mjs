import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {validators,readJson,apiDocuments} from '../../scripts/contracts/validation.mjs';
const check=await validators();
const config={edition:'Lite',deployment:'Local',presentation:'ContentOnly',behavior:'Assisted',delivery_scope:'Private',queue_limit:2,task_deadline_ms:3000,shadow:false};
test('five independent dimensions accept no-media and Pro Local profiles',()=>{
 for(const presentation of ['ContentOnly','Voice','Avatar','HumanAssist'])for(const edition of ['Lite','Standard','Pro'])assert.equal(check.workbench({...config,edition,presentation}),true);
});
test('unknown dimensions unbounded queues deadlines and plaintext credential fields fail',()=>{
 for(const delta of [{edition:'Unknown'},{presentation:'Unknown'},{behavior:'Unknown'},{deployment:'Unknown'},{delivery_scope:'Unknown'},{queue_limit:0},{queue_limit:17},{task_deadline_ms:99},{task_deadline_ms:30001},{api_key:'fixture'},{shadow:true,delivery_scope:'Public'}])assert.equal(check.workbench({...config,...delta}),false,JSON.stringify(delta));
});
test('active catalog freezes bootstrap and foundation; future domain entries stay draft',async()=>{
 const pointer=await readJson('schemas/catalog/source.json');assert.equal(pointer.source,'schemas/catalog/capabilities.json');
 const c=await readJson(pointer.source);assert.equal(check.catalog(c),true,JSON.stringify(check.catalog.errors));assert.equal(c.protocol_major,2);assert.deepEqual(c.adapters_required,[]);
 assert.deepEqual(c.capabilities.filter(i=>i.stage==='frozen').map(i=>i.id).sort(),['workbench.foundation@1','workbench.inspect@1','workbench.prepare@1']);
 for(const id of ['delivery.dispatch@1','content.objects@1','live.program@1','live.assist@1','live.review@1'])assert.equal(c.capabilities.find(i=>i.id===id)?.stage,'draft');
});
test('HTTP target declaration is not falsely advertised as implemented',async()=>{
 const docs=await apiDocuments();const api=docs['schemas/openapi/workbench.yaml'];assert.equal(api['x-implementation-status'],'contract-only');assert.ok(api.paths['/api/v2/capabilities']);assert.ok(api.paths['/api/v2/prepare']);
});
test('source protobuf explicitly separates generic delivery and ACK evidence',async()=>{
 const source=await readFile('schemas/workbench/bootstrap.proto','utf8');assert.match(source,/package nai\.workbench\.v1/);assert.match(source,/CURRENT=2/);assert.match(source,/FRAME_BYTES=65536/);assert.match(source,/DELIVERY_STATE_OUTCOME_UNKNOWN/);assert.match(source,/EVIDENCE_KIND_HUMAN_CONFIRMATION/);assert.match(source,/WorkbenchDeliveryUnit/);assert.doesNotMatch(source,/bytes (pcm|pixels)/);
});
