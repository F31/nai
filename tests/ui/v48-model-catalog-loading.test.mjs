import test from 'node:test';
import assert from 'node:assert/strict';
import {webcrypto} from 'node:crypto';
import {readModelCatalog,catalogError} from '../../apps/workbench-desktop/ui/model-catalog-loading.ts';
import {ApiError} from '../../apps/workbench-desktop/ui/api.ts';
globalThis.crypto??=webcrypto;
const id=n=>'00000000-0000-4000-8000-'+String(n).padStart(12,'0'),sha='a'.repeat(64);
const settings=(shared=false)=>({sharedCatalogEnabled:shared,llmModels:[],ttsModels:[]});
const source=(n,capability='llm')=>({catalogModelId:'',sourceWorkspaceId:id(n),sourceModelId:id(n+100),sourceRevision:'1',sourceSha256:sha,capability});
const preview=(rows=[])=>({sharedModelPreview:{planSha256:sha,executable:false,rows}});
const committed=request=>({sharedModelMigration:{migrationId:request.migrationId,stage:'committed',completed:request.entries.length,total:request.entries.length,backupId:id(99),backupManifestSha256:sha,request}});
function apiFor(handler){const calls=[];return{calls,async call(op){calls.push(op);return handler(op,calls.length);}};}
test('ready platform catalogue reads the database without migration',async()=>{
 const model={id:id(1),name:'Agnes'},api=apiFor(()=>({...settings(true),llmModels:[model]}));
 assert.deepEqual((await readModelCatalog(api,id(1))).result.llmModels,[model]);assert.equal(api.calls.length,1);
});
test('all workspace LLM/TTS models load automatically; identical names keep distinct mappings',async()=>{
 let migrated=false;const rows=[1,2,3].map((n,i)=>({name:'同名模型',source:source(n,i===2?'tts':'llm')}));let request;
 const api=apiFor(op=>{switch(op.$case){case 'readLlmSettings':return migrated?{...settings(true),llmModels:[{id:request.entries[0].catalogModelId},{id:request.entries[1].catalogModelId}],ttsModels:[{id:request.entries[2].catalogModelId}]}:settings();case 'readSharedMigration':return{};case 'previewSharedModels':return preview(rows);case 'migrateSharedModels':request=op.migrateSharedModels;migrated=true;return committed(request);}});
 const r=await readModelCatalog(api,id(1));assert.equal(r.result.llmModels.length,2);assert.equal(r.result.ttsModels.length,1);assert.equal(new Set(request.entries.map(x=>x.catalogModelId)).size,3);assert.deepEqual(request.entries.map(x=>x.sourceWorkspaceId),[id(1),id(2),id(3)]);assert.equal(request.entries[2].capability,'tts');
});
test('restart resumes the exact persisted mapping without a new preview or identity',async()=>{
 const request={migrationId:id(10),planSha256:sha,entries:[{...source(1),catalogModelId:id(20)}]};let done=false;
 const api=apiFor(op=>{if(op.$case==='readLlmSettings')return settings(done);if(op.$case==='readSharedMigration')return{sharedModelMigration:{request}};assert.equal(op.$case,'migrateSharedModels');assert.deepEqual(op.migrateSharedModels,request);done=true;return committed(request);});
 assert.equal((await readModelCatalog(api,id(1))).result.sharedCatalogEnabled,true);assert.equal(api.calls.some(x=>x.$case==='previewSharedModels'),false);
});
test('empty database initializes through bounded journal steps and verifies committed read',async()=>{
 let initialized=false,steps=0;
 const api=apiFor(op=>{if(op.$case==='readLlmSettings')return settings(initialized);if(op.$case==='readSharedMigration')return{};if(op.$case==='previewSharedModels')return preview();const r=op.migrateSharedModels;assert.deepEqual(r.entries,[]);initialized=++steps===2;return initialized?committed(r):{sharedModelMigration:{migrationId:r.migrationId,stage:'preparing',completed:0,total:0,backupId:'',backupManifestSha256:'',request:r}};});
 assert.equal((await readModelCatalog(api,id(1))).result.sharedCatalogEnabled,true);assert.equal(steps,2);assert.equal(api.calls.at(-1).$case,'readLlmSettings');
});
test('changed preview refusal propagates without retrying or deleting sources',async()=>{
 const api=apiFor(op=>{if(op.$case==='readLlmSettings')return settings();if(op.$case==='readSharedMigration')return{};if(op.$case==='previewSharedModels')return preview();throw new ApiError('REVISION_CONFLICT');});await assert.rejects(readModelCatalog(api,id(1)),/REVISION_CONFLICT/);assert.equal(api.calls.filter(x=>x.$case==='migrateSharedModels').length,1);
});
test('read failure remains an error instead of an empty result',async()=>{
 const api=apiFor(()=>{throw new ApiError('ENGINE_LOST');});await assert.rejects(readModelCatalog(api,id(1)),/ENGINE_LOST/);assert.equal(api.calls.length,1);
});
test('invalid preview or missing persisted request cannot authorize migration',async()=>{
 for(const state of [{sharedModelMigration:{}},{}]){const api=apiFor(op=>op.$case==='readLlmSettings'?settings():op.$case==='readSharedMigration'?state:preview([{}]));await assert.rejects(readModelCatalog(api,id(1)),/PROTOCOL_ERROR/);assert.equal(api.calls.some(x=>x.$case==='migrateSharedModels'),false);}
});
test('capacity overflow preserves all models and never starts partial migration',async()=>{
 const api=apiFor(op=>op.$case==='readLlmSettings'?settings():op.$case==='readSharedMigration'?{}:preview(Array.from({length:9},(_,i)=>({source:source(i+1)}))));await assert.rejects(readModelCatalog(api,id(1)),/RESOURCE_EXHAUSTED/);assert.equal(api.calls.some(x=>x.$case==='migrateSharedModels'),false);assert.match(catalogError(new ApiError('RESOURCE_EXHAUSTED')),/未截断或删除/);
});
test('concurrent committed catalogue is reread without duplicate migration',async()=>{
 let n=0;const api=apiFor(op=>op.$case==='readLlmSettings'?settings(++n===2):{sharedCatalogEnabled:true});assert.equal((await readModelCatalog(api,id(1))).result.sharedCatalogEnabled,true);assert.equal(api.calls.length,3);
});
test('backup or credential failure never exposes a partial catalogue',async()=>{
 const request={migrationId:id(10),planSha256:sha,entries:[{...source(1),catalogModelId:id(20)}]};const api=apiFor(op=>{if(op.$case==='readLlmSettings')return settings();if(op.$case==='readSharedMigration')return{sharedModelMigration:{request}};throw new ApiError('INTEGRITY_FAILED');});await assert.rejects(readModelCatalog(api,id(1)),/INTEGRITY_FAILED/);assert.equal(api.calls.length,3);assert.match(catalogError(new ApiError('INTEGRITY_FAILED')),/保留现场/);assert.match(catalogError(new ApiError('BUSY')),/停止后刷新/);
});
