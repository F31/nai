import {test} from 'node:test';
import assert from 'node:assert/strict';
import {inspect} from '../../apps/workbench-desktop/ui/client.ts';
import {WorkbenchResponse,Presentation} from '../../packages/contracts-ts/dist/index.js';
const mode=Presentation.PRESENTATION_CONTENT_ONLY;
const response=()=>WorkbenchResponse.fromPartial({requestId:'independent-id',seq:'1',handshake:{protocolMajor:2,maxFrameBytes:65536},outcome:{$case:'capabilities',capabilities:{config:{edition:1,deployment:1,presentation:mode,behavior:2,deliveryScope:1,queueLimit:2,taskDeadlineMs:3000,shadow:false},businessReady:false,status:2,providerCalls:0,startedMediaProcesses:0,capabilities:[{capabilityId:'workbench.inspect@1',implementation:2,available:true}],requiredCapabilities:['storage.persist@1']}}});
const encoded=r=>Array.from(WorkbenchResponse.encode(r).finish());
test('uses exactly one narrow host request; Prepare remains not ready',async()=>{
 const calls=[];const invoke=async(name,args)=>{calls.push({name,args});const r=response();if(args.prepare)r.outcome.capabilities.status=1;return encoded(r);};
 assert.equal((await inspect(invoke,mode)).businessReady,false);
 assert.equal((await inspect(invoke,mode,true)).status,1);
 assert.deepEqual(calls,[{name:'workbench_inspect',args:{presentation:mode,prepare:false}},{name:'workbench_inspect',args:{presentation:mode,prepare:true}}]);
});
test('rejects forged readiness, counters, enums, identity and duplicate capabilities',async()=>{
 const mutations=[r=>r.seq='2',r=>r.requestId='',r=>r.handshake.protocolMajor=1,r=>r.handshake.maxFrameBytes=999999,r=>r.outcome.capabilities.businessReady=true,r=>r.outcome.capabilities.providerCalls=1,r=>r.outcome.capabilities.startedMediaProcesses=1,r=>r.outcome.capabilities.config.edition=99,r=>r.outcome.capabilities.config.shadow=true,r=>r.outcome.capabilities.config.presentation=3,r=>r.outcome.capabilities.capabilities=[],r=>r.outcome.capabilities.capabilities.push({...r.outcome.capabilities.capabilities[0]}),r=>r.outcome.capabilities.capabilities[0].implementation=99,r=>r.outcome.capabilities.capabilities[0].capabilityId='avatar.render@1'];
 for(const mutate of mutations){const r=response();mutate(r);await assert.rejects(inspect(async()=>encoded(r),mode),/WB_PROTOCOL_ERROR/);}
});
test('rejects malformed and oversized wire data without retrying host errors',async()=>{
 for(const raw of [null,[],[256],[-1],[1.1],Array(65537).fill(0)])await assert.rejects(inspect(async()=>raw,mode));
 let calls=0;await assert.rejects(inspect(async()=>{calls++;throw Error('WB_BUSY');},mode),/WB_BUSY/);assert.equal(calls,1);
});

test('independent native app exposes narrow preparation and diagnostic permissions with new-page assets',async()=>{
 const {readFile,readdir}=await import('node:fs/promises');
 const config=JSON.parse(await readFile('apps/workbench-desktop/tauri.conf.json','utf8'));
 assert.equal(config.identifier,'cn.nai.workbench.bootstrap');assert.equal(config.build.frontendDist,'dist');assert.equal(config.app.windows[0].url,'index.html');assert.equal(config.bundle.active,false);
 const permission=await readFile('apps/workbench-desktop/permissions/bootstrap.toml','utf8');assert.match(permission,/commands.allow=\["workbench_inspect","frontend_ready"\]/);
 const preparation=await readFile('apps/workbench-desktop/permissions/preparation.toml','utf8');assert.match(preparation,/commands.allow\s*=\s*\["workbench_open","foundation_command","workbench_close"\]/);assert.doesNotMatch(preparation,/grant|shell|sql|filesystem/);
 const manifest=await readFile('apps/workbench-desktop/Cargo.toml','utf8');assert.doesNotMatch(manifest,/nai-storage|nai-runtime["=]|nai-provider|nai-engine/);
 const build=await readFile('apps/workbench-desktop/vite.config.mjs','utf8');assert.ok(build.includes('./index.html'));assert.ok(!build.includes('../desktop'));assert.ok(build.includes('dedupe'));
});


