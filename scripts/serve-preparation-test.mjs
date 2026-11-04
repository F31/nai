// Explicit local UI test bridge to a real foundation process. Never bundled or started by the product.
import http from 'node:http';
import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import {spawn} from 'node:child_process';
import {randomBytes,randomUUID} from 'node:crypto';
import {execFileSync} from 'node:child_process';
import {FoundationRequest,FoundationResponse,FoundationCommand} from '../packages/contracts-ts/dist/index.js';
const engine=process.env.NAI_FOUNDATION_ENGINE;if(!engine||!path.isAbsolute(engine))throw Error('explicit foundation binary required');await fs.access(engine);
const data=await fs.mkdtemp(path.join(os.tmpdir(),'nai-preparation-ui-')),principal=randomUUID();let owner=null,seq=0,pending=null,buffer=Buffer.alloc(0);
// Explicit synthetic provider: never bundled, never binds outside loopback, no production Key.
let provider=null,testWorkspace=null;
if(process.env.NAI_TEST_LLM_PROVIDER==='1'){
 process.env.NAI_LLM_TEST_LOOPBACK='1';let calls=0;
 provider=http.createServer(async(req,res)=>{if(req.method!=='POST'||req.url!=='/v1/chat/completions'){res.statusCode=404;res.end();return;}let text='';for await(const part of req){text+=part;if(text.length>16384){res.statusCode=413;res.end();return;}}const body=JSON.parse(text);if(req.headers.authorization!=='Bearer synthetic-ui-test-key'||body.max_tokens!==256)throw Error('invalid synthetic request');calls++;console.log(`SYNTHETIC_LLM_HTTP ${calls}`);setTimeout(()=>{res.setHeader('Content-Type','application/json');res.end(JSON.stringify({choices:[{finish_reason:'stop',message:{content:body.response_format?JSON.stringify({text:'标准款容量为500毫升，便于随身携带。',fact_ids:['capacity']}):'NAI_OK'}}],usage:{prompt_tokens:100,completion_tokens:20}}));},calls===2?8000:500);});
 await new Promise(resolve=>provider.listen(13644,'127.0.0.1',resolve));
}
function close(){owner?.kill();owner=null;seq=0;buffer=Buffer.alloc(0);pending?.reject(Error('WB_ENGINE_LOST'));pending=null;}
function open(){if(owner)return;const token=randomBytes(32).toString('hex');const child=spawn(engine,['--data-root',data,'--desktop-owner',...(!seq&&initialized?[]:['--initialize'])],{windowsHide:true,env:{...process.env,NAI_WORKBENCH_TOKEN:token,NAI_FOUNDATION_PRINCIPAL:principal},stdio:['pipe','pipe','ignore']});owner={token,kill:()=>child.kill(),child};child.stdout.on('data',bytes=>{buffer=Buffer.concat([buffer,bytes]);if(buffer.length>65540){close();return;}if(buffer.length>=4&&buffer.length>=buffer.readUInt32LE()+4){const body=buffer.subarray(4,4+buffer.readUInt32LE());buffer=buffer.subarray(body.length+4);const task=pending;pending=null;task?.resolve(Array.from(body));}});child.on('exit',()=>{if(owner?.child===child)close();});child.on('error',()=>close());}
let initialized=false;
async function call(command){if(pending)throw Error('WB_BUSY');if(!owner)throw Error('WB_NOT_CONNECTED');const r=FoundationRequest.fromPartial({protocolMajor:2,authorization:Buffer.from(owner.token),requestId:randomUUID(),seq:String(++seq),ttlMs:4000,command});const bytes=FoundationRequest.encode(r).finish(),header=Buffer.alloc(4);header.writeUInt32LE(bytes.length);return new Promise((resolve,reject)=>{const timer=setTimeout(()=>{close();reject(Error('WB_OUTCOME_UNKNOWN'));},4500);pending={resolve:raw=>{clearTimeout(timer);const reply=FoundationResponse.decode(Uint8Array.from(raw));if(reply.outcome?.$case==='result')initialized=true;resolve(raw);},reject:e=>{clearTimeout(timer);reject(e);}};owner.child.stdin.write(Buffer.concat([header,bytes]));});}
const inject=`window.__TAURI__={core:{invoke:async(name,args)=>{const response=await fetch('/__test_invoke',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({name,args})});const result=await response.json();if(!response.ok)throw Error(result.error);return result;}}};`;
if(process.env.NAI_TEST_FAQ_SEED==='1'){
 open();const ws=randomUUID();testWorkspace=ws;const command=async operation=>{const raw=await call(FoundationCommand.fromPartial({idempotencyKey:randomUUID(),operation}));const result=FoundationResponse.decode(Uint8Array.from(raw)).outcome;if(result?.$case!=='result')throw Error('test seed rejected');return result.result;};
 await command({$case:'createWorkspace',createWorkspace:{workspaceId:ws,name:'FAQ验收测试工作区',budgetLimitMinor:'0',currency:'CNY'}});
 const product=JSON.parse(await fs.readFile('tests/fixtures/contracts/preparation.json','utf8'));product.name='演练水杯';
 const refs=[];for(const doc of [product,{schema_version:1,kind:'host_draft',name:'测试主持',style:'professional',preferences:''},{schema_version:1,kind:'series_draft',name:'测试系列',goal:'介绍用途和限制'}]){const id=randomUUID(),r=await command({$case:'putPreparation',putPreparation:{workspaceId:ws,objectId:id,expectedRevision:'0',body:JSON.stringify(doc)}});refs.push({object_id:id,revision:Number(r.revision),sha256:r.sha256});}
 const showId=randomUUID();const show=await command({$case:'putPreparation',putPreparation:{workspaceId:ws,objectId:showId,expectedRevision:'0',body:JSON.stringify({schema_version:1,kind:'show_draft',name:'FAQ测试场次',product:refs[0],host:refs[1],series:refs[2],cues:[{id:'intro',title:'开场',talk_track_id:'opening',mandatory_fact_ids:['capacity'],allow_questions:true}]})}});
 const report=await command({$case:'publicationImpact',publicationImpact:{workspaceId:ws,reportId:randomUUID(),showId,showRevision:show.revision,previousSnapshotId:''}});await command({$case:'publish',publish:{workspaceId:ws,snapshotId:randomUUID(),reportId:report.resourceId,reportSha256:report.sha256,acknowledgeUnknown:true}});
 console.log('FAQ_TEST_FIXTURE_CREATED: isolated synthetic product, real commands, no provider');
 if(process.env.NAI_TEST_MODEL_CATALOG_SEED==='1'){
  for(const suffix of ['A','B'])await command({$case:'saveLlmModel',saveLlmModel:{workspaceId:ws,modelId:randomUUID(),expectedRevision:'0',name:'列表验收文本 '+suffix,baseUrl:'https://example.invalid/v1',model:'test-text-'+suffix.toLowerCase(),inputMinorPerMillion:'100',outputMinorPerMillion:'200',enabled:true,apiKey:[],pricesConfirmed:true}});
  await fs.writeFile('.nai-diagnostics/model-catalog-ui-root.json',JSON.stringify({root:data,principal,workspace:ws}));
  console.log('MODEL_CATALOG_FIXTURE_CREATED: metadata only, no provider requests');
 }
 if(process.env.NAI_TEST_LLM_CHECK_SEED==='1'){
  if(!provider)throw Error('explicit synthetic provider required');
  await command({$case:'saveLlmModel',saveLlmModel:{workspaceId:ws,modelId:randomUUID(),expectedRevision:'0',name:'本机条件检查测试模型',baseUrl:'http://127.0.0.1:13644/v1',model:'test-model',inputMinorPerMillion:'100',outputMinorPerMillion:'200',enabled:true,apiKey:Buffer.from('synthetic-ui-test-key'),pricesConfirmed:true}});
  await fs.writeFile('.nai-diagnostics/rf04-local-check-ui-root.json',JSON.stringify({root:data,principal,workspace:ws}));
  console.log('LOCAL_CHECK_FIXTURE_CREATED: synthetic key, budget=0, no provider calls');
 }
}
const server=http.createServer(async(req,res)=>{try{
 if(req.method==='POST'&&req.url==='/__test_invoke'){
  if(req.headers.origin!=='http://127.0.0.1:13643'||req.headers['content-type']!=='application/json')throw Error('WB_PERMISSION_DENIED');
  let text='';for await(const part of req){text+=part;if(text.length>80000)throw Error('WB_FRAME_LIMIT');}const {name,args}=JSON.parse(text);let result=null;
  if(name==='workbench_open'){open();result=await call(FoundationCommand.fromPartial({idempotencyKey:randomUUID(),operation:{$case:'listWorkspaces',listWorkspaces:{limit:32}}}));}
  else if(name==='foundation_command'){if(!Array.isArray(args.command)||args.command.length>16384)throw Error('WB_FRAME_LIMIT');result=await call(FoundationCommand.decode(Uint8Array.from(args.command)));}
  else if(name==='workbench_close')close();else if(name!=='frontend_ready')throw Error('WB_PERMISSION_DENIED');
  res.setHeader('Content-Type','application/json');res.end(JSON.stringify(result));return;
 }
 const url=new URL(req.url,'http://127.0.0.1'),base=path.resolve('apps/workbench-desktop/dist'),file=path.resolve(base,'.'+url.pathname);if(!file.startsWith(base+path.sep))throw Error('path');let bytes=await fs.readFile(file);if(url.pathname==='/index.html')bytes=Buffer.from(bytes.toString().replace('<head>','<head><script>'+inject+'</script>'));res.setHeader('Content-Type',({'.js':'text/javascript','.css':'text/css','.html':'text/html'})[path.extname(file)]??'application/octet-stream');res.end(bytes);
 }catch(e){res.statusCode=500;res.setHeader('Content-Type','application/json');res.end(JSON.stringify({error:e.message}));}});
server.listen(13643,'127.0.0.1',()=>console.log('PREPARATION_TEST_UI http://127.0.0.1:13643/index.html real_engine=true provider_calls=0'));
async function shutdown(){
 close();server.close();provider?.close();
 if(process.platform==='win32'&&testWorkspace){
  const scope=`${principal}:${String.fromCharCode(92,92,63,92)}${data}`;
  const references=new Set();for(const file of await fs.readdir(path.join(data,'objects')).catch(()=>[])){try{const v=JSON.parse(await fs.readFile(path.join(data,'objects',file),'utf8'));if(['publication_llm_config','publication_tts_config'].includes(v.kind)&&/^[a-f0-9-]{36}$/.test(v.secret?.reference??''))references.add(JSON.stringify([v.kind,v.secret.reference]));}catch{}}
  for(const entry of references){const [kind,reference]=JSON.parse(entry);const service=kind==='publication_tts_config'?'tts':'llm';execFileSync('cmdkey.exe',[`/delete:${scope}:${testWorkspace}:${reference}.nai.workbench.v43.${service}`],{windowsHide:true,stdio:'ignore'});}
 }
}
process.on('SIGINT',()=>void shutdown());process.on('SIGTERM',()=>void shutdown());

