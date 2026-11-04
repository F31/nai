import test from 'node:test';
import assert from 'node:assert/strict';
import {runMigration,validMigrationStatus} from '../../apps/workbench-desktop/ui/shared-model-migration.ts';
const id=n=>`00000000-0000-0000-0000-${n.toString(16).padStart(12,'0')}`;
const request={migrationId:id(1),planSha256:'a'.repeat(64),entries:[{catalogModelId:id(2),sourceWorkspaceId:id(3),sourceModelId:id(4),sourceRevision:'1',sourceSha256:'b'.repeat(64),capability:'llm'}]};
const status=(stage,completed=0)=>({migrationId:request.migrationId,stage,completed,total:1,backupId:stage==='preparing'?'':id(5),backupManifestSha256:stage==='preparing'?'':'c'.repeat(64),request});
test('one confirmation drives bounded steps and publishes only committed',async()=>{const stages=[status('preparing'),status('rewrapping'),status('rewrapping',1),status('committed',1)],seen=[];let calls=0;await runMigration(request,async r=>{assert.equal(r,request);return stages[calls++];},s=>seen.push(s.stage));assert.equal(calls,4);assert.equal(seen.at(-1),'committed');});
test('errors stop immediately and never blindly retry a side effect',async()=>{let calls=0;await assert.rejects(runMigration(request,async()=>{calls++;throw Error('vault unavailable');},()=>{}),/vault unavailable/);assert.equal(calls,1);});
test('restart continues original mapping and skips no completion checks',async()=>{let calls=0;await runMigration(request,async()=>{calls++;return status('committed',1);},()=>{});assert.equal(calls,1);assert.equal(validMigrationStatus(status('committed',0)),false);});
test('mismatched response cannot authorize publication',async()=>{await assert.rejects(runMigration(request,async()=>({...status('committed',1),migrationId:id(99)}),()=>{}),/响应不匹配/);});
test('stalled state has a finite command bound',async()=>{let calls=0;await assert.rejects(runMigration(request,async()=>{calls++;return status('preparing');},()=>{}),/尚未完成/);assert.equal(calls,5);});

