import {test} from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import {validators,readJson} from '../../scripts/contracts/validation.mjs';
const check=await validators();
test('root manifest is exact, isolated and never carries credentials',()=>{
 const root={kind:'nai-foundation-v2',schema_version:2,owner:'00000000-0000-0000-0000-000000000001'};
 assert.equal(check.foundationRoot(root),true);
 for(const bad of [{...root,kind:'nai-legacy'},{...root,schema_version:1},{...root,owner:'client actor'},{...root,api_key:'fixture'}])assert.equal(check.foundationRoot(bad),false);
});
test('foundation wire keeps actor authority out of untrusted commands and separates stop outcomes',async()=>{
 const source=await readFile('schemas/workbench/foundation.proto','utf8');
 assert.doesNotMatch(source,/\bactor(?:_id)?\s*=/);assert.match(source,/oneof operation/);
 for(const field of ['outcome_unknown_count','dispatch_in_flight','already_submitted'])assert.ok(source.includes(field));
 assert.doesNotMatch(source,/bytes (pcm|pixels)/);
});
test('only the implemented foundation command is frozen; future domain services stay draft',async()=>{
 const catalog=await readJson('schemas/catalog/capabilities.json');const c=catalog.capabilities.find(c=>c.id==='workbench.foundation@1');
 assert.equal(c.stage,'frozen');assert.equal(c.request,'nai.workbench.v1.FoundationRequest');assert.equal(c.response,'nai.workbench.v1.FoundationResponse');
 for(const id of ['content.objects@1','live.program@1','delivery.dispatch@1'])assert.equal(catalog.capabilities.find(c=>c.id===id).stage,'draft');
});
test('native UI is not a second database writer',async()=>{
 const manifest=await readFile('apps/workbench-desktop/Cargo.toml','utf8');assert.doesNotMatch(manifest,/rusqlite|foundation-sqlite|nai-storage/);
 const schema=await readFile('schemas/storage/foundation-v2.sql','utf8');assert.match(schema,/UNIQUE|PRIMARY KEY/);assert.match(schema,/attempts BETWEEN 0 AND 1/);assert.match(schema,/command_results/);assert.match(schema,/actual_minor INTEGER/);
});
