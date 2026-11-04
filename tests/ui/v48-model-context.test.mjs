import {webcrypto} from 'node:crypto';
import {Api} from '../../apps/workbench-desktop/ui/api.ts';
import {FoundationResponse} from '../../packages/contracts-ts/dist/index.js';
import test from 'node:test';import assert from 'node:assert/strict';
globalThis.crypto??=webcrypto;
const id='00000000-0000-4000-8000-000000000001',sha='a'.repeat(64),context={workspaceId:id,snapshotId:id,snapshotSha256:sha,factsSha256:sha,contextSha256:sha};
test('API accepts a bounded common model task context and rejects malformed identities',async()=>{for(const change of [null,{workspaceId:'bad'},{snapshotId:''},{contextSha256:'b'},{factsSha256:'B'.repeat(64)}]){const r={agentContext:{...context,...change}};const bytes=Array.from(FoundationResponse.encode(FoundationResponse.fromPartial({requestId:id,seq:'1',outcome:{$case:'result',result:r}})).finish());const call=()=>new Api(async()=>bytes).call({$case:'readLlmTask',readLlmTask:{workspaceId:id,taskId:id}});if(!change)assert.deepEqual((await call()).agentContext,context);else await assert.rejects(call(),e=>e.code==='PROTOCOL_ERROR');}});
