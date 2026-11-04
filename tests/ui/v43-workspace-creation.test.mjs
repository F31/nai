import test from 'node:test';
import assert from 'node:assert/strict';
import {Api,ApiError,explain} from '../../apps/workbench-desktop/ui/api.ts';
import {WorkspaceCreation} from '../../apps/workbench-desktop/ui/workspace-creation.ts';
import {verifyPrivateAudioHost} from '../../apps/workbench-desktop/ui/native-audio.ts';
import {FoundationCommand,FoundationResponse,AuditionStopNonce} from '../../packages/contracts-ts/dist/index.js';
const uuid='00000000-0000-4000-8000-000000000001';
test('configured workspace creation carries explicit budget and recovery preserves one write',async()=>{
 for(const [budgetMode,budgetLimitMinor] of [[3,'0'],[2,'1000'],[1,'0']]){
  let writes=0,row;const api=new Api(async(name,args)=>{
   const op=FoundationCommand.decode(Uint8Array.from(args.command)).operation;
   let result;
   if(op.$case==='createWorkspaceSettings'){writes++;const x=op.createWorkspaceSettings;assert.equal(x.settings.budgetMode,budgetMode);assert.equal(x.settings.budgetLimitMinor,budgetLimitMinor);assert.equal(x.settings.expectedRevision,'0');row={id:x.workspaceId,name:x.settings.name,kind:'workspace',revision:'1'};result={resourceId:row.id};}
   else{assert.equal(op.$case,'listWorkspaces');result={resources:[row]};}
   return Array.from(FoundationResponse.encode(FoundationResponse.fromPartial({requestId:uuid,seq:'1',outcome:{$case:'result',result}})).finish());
  });
  const flow=new WorkspaceCreation(api);let fail=true;
  const activate=async()=>{if(fail)throw Error('切换失败');};
  await assert.rejects(()=>flow.create('预算工作区',()=>{},activate,{name:'',budgetMode,budgetLimitMinor,currency:'CNY',expectedRevision:'0'}));
  fail=false;await flow.create('不应重建',()=>{},activate);assert.equal(writes,1);assert.equal(flow.pending,null);
 }
});
function backend({unknown=false,reject=false}={}){
 const calls=[],rows=[];let seq=0;
 const api=new Api(async(name,args)=>{assert.equal(name,'foundation_command');const op=FoundationCommand.decode(Uint8Array.from(args.command)).operation;calls.push(op.$case);let result={};
  if(op.$case==='createWorkspace'){if(reject)throw new ApiError('INVALID_ARGUMENT');const x=op.createWorkspace;rows.push({id:x.workspaceId,name:x.name,kind:'workspace',revision:'1',sha256:'',updatedAtMs:'0',parentId:''});if(unknown)throw Error('WB_OUTCOME_UNKNOWN');result={resourceId:x.workspaceId,revision:'1'};}
  else if(op.$case==='listWorkspaces')result={resources:rows};else assert.fail('unexpected operation');
  return Array.from(FoundationResponse.encode(FoundationResponse.fromPartial({requestId:uuid,seq:String(++seq),outcome:{$case:'result',result}})).finish());
 });return {api,calls,rows};
}
test('acknowledged create survives selection denial and recovery never sends a second create',async()=>{
 const {api,calls,rows}=backend(),flow=new WorkspaceCreation(api);let denied=true;const activated=[];
 const activate=async id=>{activated.push(id);if(denied)throw Error('workbench_stop_audition not allowed.');};
 await assert.rejects(()=>flow.create(' 我的直播间 ',()=>{},activate),e=>e.code==='WORKSPACE_CREATED_PENDING');
 const id=flow.pending.id;assert.equal(rows[0].name,'我的直播间');assert.match(explain(new ApiError('WORKSPACE_CREATED_PENDING')),/已创建/);
 denied=false;await flow.create('must not create a different name',()=>{},activate);
 assert.equal(flow.pending,null);assert.deepEqual(activated,[id,id]);assert.equal(calls.filter(c=>c==='createWorkspace').length,1);
});
test('ambiguous write is reconciled by listing the same ID instead of replaying creation',async()=>{
 const {api,calls}=backend({unknown:true}),flow=new WorkspaceCreation(api);await assert.rejects(()=>flow.create('one',()=>{},async()=>{}),e=>e.code==='OUTCOME_UNKNOWN');
 assert.equal(flow.pending.confirmed,false);const id=flow.pending.id;let selected='';await flow.create('one',()=>{},async x=>{selected=x;});assert.equal(selected,id);assert.equal(calls.filter(c=>c==='createWorkspace').length,1);
});
test('deterministic rejected write never reports a created workspace',async()=>{
 const {api}=backend({reject:true}),flow=new WorkspaceCreation(api);await assert.rejects(()=>flow.create('one',()=>{},async()=>assert.fail()),e=>e.code==='INVALID_ARGUMENT');assert.equal(flow.pending,null);
});
test('native readiness invokes stop and nonce, fails clearly before ready on missing permissions or host',async()=>{
 const calls=[];await verifyPrivateAudioHost(async name=>{calls.push(name);return name==='workbench_audition_nonce'?Array.from(AuditionStopNonce.encode({nonce:uuid}).finish()):null;});assert.deepEqual(calls,['workbench_stop_audition','workbench_audition_nonce']);
 await assert.rejects(()=>verifyPrivateAudioHost(undefined),e=>e.code==='NATIVE_HOST_REQUIRED');
 await assert.rejects(()=>verifyPrivateAudioHost(async()=>{throw 'workbench_stop_audition not allowed';}),e=>e.code==='HOST_AUDIO_PERMISSION_MISSING');
 await assert.rejects(()=>verifyPrivateAudioHost(async name=>name==='workbench_audition_nonce'?[256]:null),e=>e.code==='PROTOCOL_ERROR');
 assert.match(explain('workbench_stop_audition not allowed'),/权限/);assert.match(explain(new ApiError('NATIVE_HOST_REQUIRED')),/桌面窗口/);
});
test('native audio ACL is narrow and both commands are declared in the app manifest and main capability',async()=>{
 const {readFile}=await import('node:fs/promises');
 const permission=await readFile('apps/workbench-desktop/permissions/private-audio.toml','utf8');assert.match(permission,/commands.allow=\["workbench_stop_audition","workbench_audition_nonce"\]/);
 const config=JSON.parse(await readFile('apps/workbench-desktop/capabilities/default.json','utf8'));assert.deepEqual(config.windows,['main']);assert.ok(config.permissions.includes('allow-private-audio-control'));
 const manifest=await readFile('apps/workbench-desktop/build.rs','utf8');for(const cmd of ['workbench_stop_audition','workbench_audition_nonce'])assert.ok(manifest.includes(`"${cmd}"`));assert.doesNotMatch(permission,/shell|sql|filesystem/);
});

