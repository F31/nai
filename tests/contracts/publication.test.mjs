import {test} from 'node:test';
import assert from 'node:assert/strict';
import {validators} from '../../scripts/contracts/validation.mjs';
const validate=(await validators()).publication;
const uuid='00000000-0000-0000-0000-000000000001';
const ref={object_id:uuid,revision:1,sha256:'a'.repeat(64)};
const refs={show:ref,product:ref,host:ref,series:ref};
const common={schema_version:1,created_at_ms:1000,created_by:uuid};
test('LLM encrypted configuration rejects plaintext, incomplete ciphertext and unbounded prices',()=>{
 const model={...common,kind:'publication_llm_config',name:'fixture',base_url:'https://example.invalid/v1',model:'test-model',input_minor_per_million:100,output_minor_per_million:200,enabled:true,secret:{reference:uuid,nonce:'a'.repeat(24),ciphertext:'b'.repeat(34)}};
 assert.equal(validate(model),true,JSON.stringify(validate.errors));
 for(const p of [{api_key:'synthetic-never-save'},{secret:'plaintext'},{secret:{reference:uuid,nonce:'',ciphertext:''}},{input_minor_per_million:-1}])assert.equal(validate({...model,...p}),false);
});
test('snapshots explicitly restrict profile and never contain mutable output authorization',()=>{
 const doc={...common,kind:'publication_snapshot',refs,previous_snapshot:null,report:ref,scope:'private',presentation:'content_only',valid_until_ms:null,explicit_dependencies:[]};
 assert.equal(validate(doc),true);
 for(const delta of [{scope:'public'},{presentation:'avatar'},{approved:true},{session_id:uuid},{credential_ref:'fixture'},{refs:{...refs,product:{...ref,revision:0}}}])assert.equal(validate({...doc,...delta}),false);
});
test('impact and revocation artifacts bind identity, watermarks and an exact version',()=>{
 const doc={...common,kind:'publication_impact',role_revision:1,refs,previous_snapshot:null,graph_sha256:'b'.repeat(64),expires_at_ms:31000,changed_fields:['product/facts/price/value'],explicit_dependencies:[],affected_sessions:[],affected_snapshots:[],unknown_items:['free_text_semantics_requires_human_review']};
 assert.equal(validate(doc),true);assert.equal(validate({...doc,graph_sha256:''}),false);assert.equal(validate({...doc,role_revision:0}),false);
 assert.equal(validate({...common,kind:'publication_revocation',snapshot:ref,reason:'价格已判错'}),true);
 assert.equal(validate({...common,kind:'publication_revocation',snapshot:{object_id:uuid},reason:'错误'}),false);
});

test('published speech task binds source, approval hash and fee cap without carrying raw text or PCM',()=>{
 const doc={...common,kind:'publication_tts_preview',role_revision:1,model_ref:ref,source:ref,approval_sha256:'b'.repeat(64),max_cost_minor:10,state:'queued',reason:'QUEUED',expires_at_ms:61000,reserved_minor:1,target:'private_workbench',sample_count:0,audio_sha256:''};
 assert.equal(validate(doc),true,JSON.stringify(validate.errors));
 for(const patch of [{approval_sha256:''},{source:null},{target:'public'},{max_cost_minor:0},{pcm:'bad'},{text:'unapproved text'}])assert.equal(validate({...doc,...patch}),false);
});

test('retrieved FAQ requires bounded method and original question provenance',()=>{
 const doc={...common,kind:'publication_faq_task',role_revision:1,snapshot:ref,refs,question:'这个杯子的容量有多大？',result:'retrieved',faq_id:'capacity',text:'标准款容量为500毫升。',fact_ids:['capacity'],evidence:[{fact_id:'capacity',label:'容量',value:'500',unit:'毫升',currency:'',source:'测试规格页',conditions:'标准款',valid_from_ms:null,valid_until_ms:null}],expires_at_ms:301000,provider_calls:0,api_cost_minor:0,currency:'CNY',target:'private_workbench',retrieval:{strategy:'faq-lexical-zh-v1',method:'lexical',matched_question:'容量多大？',score:1000}};
 assert.equal(validate(doc),true,JSON.stringify(validate.errors));
 for(const patch of [{retrieval:undefined},{retrieval:{...doc.retrieval,method:'exact'}},{retrieval:{...doc.retrieval,matched_question:''}},{retrieval:{...doc.retrieval,score:1001}},{retrieval:{...doc.retrieval,strategy:'unknown'}}])assert.equal(validate({...doc,...patch}),false);
});

test('private question parents and bounded bucket indices reject authority or forged fields',()=>{
 const context={workspace_id:uuid,snapshot_id:uuid,snapshot_sha256:'a'.repeat(64),facts_sha256:'b'.repeat(64),context_sha256:'c'.repeat(64)};
 const parent={...common,kind:'publication_question_task',role_revision:1,updated_at_ms:1000,expires_at_ms:121000,context,question:'容量多大？',dedup_key:'d'.repeat(64),state:'queued',occurrences:1,child_task_id:'',child_task_sha256:''};
 assert.equal(validate(parent),true,JSON.stringify(validate.errors));for(const p of [{context:{...context,session_id:uuid}},{state:'approved'},{occurrences:100},{question:'x'.repeat(257)},{api_key:'forbidden'},{child_task_sha256:'bad'}])assert.equal(validate({...parent,...p}),false);
 const index={schema_version:1,kind:'publication_question_index',created_by:uuid,context_sha256:'c'.repeat(64),bucket:1,entries:[uuid]};assert.equal(validate(index),true,JSON.stringify(validate.errors));assert.equal(validate({...index,entries:[uuid,uuid]}),false);assert.equal(validate({...index,entries:Array(33).fill(uuid)}),false);
});

test('business model artifacts constrain the common context without granting old artifacts authority',()=>{
 const context={workspace_id:uuid,snapshot_id:uuid,snapshot_sha256:'a'.repeat(64),facts_sha256:'b'.repeat(64),context_sha256:'c'.repeat(64)};
 const speech={...common,kind:'publication_tts_preview',role_revision:1,model_ref:ref,source:ref,approval_sha256:'b'.repeat(64),max_cost_minor:10,state:'queued',reason:'QUEUED',expires_at_ms:61000,reserved_minor:1,target:'private_workbench',sample_count:0,audio_sha256:'',task_context:context};
 assert.equal(validate(speech),true,JSON.stringify(validate.errors));for(const patch of [{snapshot_id:'bad'},{facts_sha256:'bad'},{session_id:uuid}])assert.equal(validate({...speech,task_context:{...context,...patch}}),false);
 const old={...speech};delete old.task_context;assert.equal(validate(old),true,'historical artifact can be parsed; execution gate must refuse it');
});

test('live checkpoint artifact bounds queue and state without accepting page-supplied media claims',()=>{
 const context={workspace_id:uuid,snapshot_id:uuid,snapshot_sha256:'a'.repeat(64),facts_sha256:'b'.repeat(64),context_sha256:'c'.repeat(64)};
 const live={...common,kind:'publication_live_run',name:'本场队列',updated_at_ms:1000,role_revision:1,context,session_epoch:1,cue_ids:['intro','details'],cursor:0,state:1,submitted_count:0,skipped_count:0,active_approval:'',active_bundle:'',last_approval:'',last_bundle:''};assert.equal(validate(live),true,JSON.stringify(validate.errors));for(const patch of [{cue_ids:[]},{cue_ids:Array(33).fill('intro')},{cue_ids:['intro','intro']},{state:8},{session_epoch:0},{cursor:33},{hardware_ack:true},{permit:'forged'}])assert.equal(validate({...live,...patch}),false);
});
