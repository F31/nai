import test from 'node:test';
import assert from 'node:assert/strict';
import descriptors from 'ts-proto-descriptors';
import { readFile } from 'node:fs/promises';
import { validators, readJson, validateSemantic, apiDocument } from '../../scripts/contracts/validation.mjs';

const check = await validators();
const runtime = await readJson('tests/fixtures/contracts/runtime.json');
const catalog = await readJson('schemas/catalog/capabilities.json');
const provider = {schema_version:1,id:'tts-test',protocol:'http_stream',endpoint:'https://example.invalid/tts',model:'test',credential_ref:'credential:test',region:'test',data_policy_ref:'test',timeout_ms:1000,max_retries:1,max_concurrency:1,price_revision:'test'};
const plugin = {manifest_schema:1,plugin_id:'test-only',plugin_version:'0.1.0',kernel_range:'>=0.1.0,<1.0.0',protocol_major:1,contracts:['avatar.render@1'],supported_hosts:['subprocess'],activation:'on_session_start',idle_policy:'keep_warm_during_session',requires:['audio.timeline@1'],os:['windows'],architectures:['x86_64'],artifact_hash:'a'.repeat(64),license_ref:'fixture-not-a-license',install_bytes:'100',config_schema_ref:'test',data_schema_ref:'test',health_operation:'health',migration_ref:'none',rollback_range:'0.1.x',price_policy_ref:'test',resources:{profile_id:'test',certification_ref:'candidate-not-certified'},permissions:{network:false,media_output:true,secrets:[]}};
const model = {schema_version:1,model_id:'test-only',version:'1',license_ref:'fixture-not-a-license',graphs:[{id:'full-pipeline',file:{path:'pipeline.onnx',sha256:'b'.repeat(64),license_ref:'fixture'},external_weights:[],inputs:[{name:'audio',dtype:'float32',shape:[1,'samples']}],outputs:[{name:'frame',dtype:'float32',shape:[1,3,64,64]}],opset:17}],required_graph_ids:['full-pipeline'],preprocess_revision:'test',postprocess_revision:'test',sample_rate:16000,window_samples:640,hop_samples:640,lookahead_samples:0,padding:'zero',normalization:'test',frame_mapping:'test',fps_num:25,fps_den:1,runtime_matrix:[{os:'windows',arch:'x86_64',ort:'test',ort_crate:'test',ep:'cuda',precision:'fp32',driver:'test',cuda:'test',cudnn:'test',certification_ref:'candidate-not-certified'}],bom_ref:'test',golden_ref:'test'};

function valid(kind, value) {
  assert.equal(check[kind](value), true, JSON.stringify(check[kind].errors));
  validateSemantic(kind, value);
}

test('all config/manifest/catalog schemas accept complete test-only structures', () => {
  for (const [kind, value] of Object.entries({runtime,provider,plugin,model,catalog})) valid(kind, value);
});
test('2D config requires bounded explicit layers, PCM and RMS parameters', () => {
  const value = {schema_version:1,pixel_format:'rgba8',sample_rate:16000,max_buffer_samples:16000,noise_gate:0.02,full_open_rms:0.5,attack_ms:10,release_ms:100,layers:['base','mouth_closed','mouth_open'].map(role=>({asset_id:role,role,width:2,height:2,x:0,y:0}))};
  assert.equal(check.avatar2d(value),true);
  for (const mutate of [v=>{v.max_buffer_samples=96001;},v=>{v.attack_ms=0;},v=>{v.layers[0].width=1921;},v=>{v.layers.pop();},v=>{v.pixel_format='rgb';},v=>{v.unknown=true;}]) {
    const bad=structuredClone(value);mutate(bad);assert.equal(check.avatar2d(bad),false);
  }
});
test('runtime rejects unknown fields, unsafe permissions and nonpositive deadlines', () => {
  for (const change of [value=>{value.api_key='secret';},value=>{value.scheduler.qa_ttl_ms=0;},value=>{value.session_defaults.allow_auto_without_platform_grant=true;},value=>{value.knowledge.semantic_enabled=true;}]) {
    const bad = structuredClone(runtime); change(bad); assert.equal(check.runtime(bad), false);
  }
  const bad = structuredClone(runtime); bad.media.lease_expire_ms=100;
  assert.throws(()=>validateSemantic('runtime',bad),/lease/);
});
test('provider accepts credential references but rejects plaintext and credential URLs', () => {
  assert.equal(check.provider({...provider,api_key:'secret'}), false);
  assert.equal(check.provider({...provider,credential_ref:'sk-secret'}), false);
  assert.throws(()=>validateSemantic('provider',{...provider,endpoint:'https://user:secret@example.invalid'}),/credentials/);
  valid('provider',{...provider,protocol:'websocket',endpoint:'wss://example.invalid/tts'});
  assert.equal(check.provider({...provider,protocol:'websocket',endpoint:'ws://example.invalid/tts'}),false);
});
test('scheduler and budget extensions are bounded and use exact integer strings', () => {
  const extended=structuredClone(runtime);
  Object.assign(extended.scheduler,{max_ingress_events:64,max_state_slots:8,max_dedup_entries:256,viewer_cooldown_ms:1000,fairness_window:10,max_viewer_share_percent:50,starvation_ms:2000});
  Object.assign(extended.budgets,{global_limit_minor:'18446744073709551615',max_ledger_entries:1000,global_usage_limits:{requests:'100'},global_concurrent_limits:{provider_slots:'2'}});
  valid('runtime',extended);
  const bad=structuredClone(extended);bad.budgets.global_limit_minor=10;assert.equal(check.runtime(bad),false);
  bad.budgets.global_limit_minor='10';bad.scheduler.max_state_slots=0;assert.equal(check.runtime(bad),false);
  valid('provider',{...provider,quota:{currency:'CNY',limit_minor:'10',usage_limits:{tokens:'1000'},concurrent_limits:{provider_slots:'1'}}});
  assert.equal(check.provider({...provider,quota:{currency:'CNY',limit_minor:'10.5',usage_limits:{},concurrent_limits:{}}}),false);
});
test('plugin and model do not acquire ready status from incomplete or malformed manifests', () => {
  const badPlugin = structuredClone(plugin); delete badPlugin.resources.certification_ref;
  assert.equal(check.plugin(badPlugin), false);
  const bad = structuredClone(model); bad.graphs[0].inputs=[];
  assert.equal(check.model(bad), false);
  const escaped = structuredClone(model); escaped.graphs[0].file.path='../outside.onnx';
  assert.equal(check.model(escaped), false);
  const missing = structuredClone(model); missing.required_graph_ids.push('missing-audio-encoder');
  assert.throws(()=>validateSemantic('model',missing),/incomplete/);
});
test('capability catalog has resolvable types, explicit stream semantics and no duplicate IDs', async () => {
  const descriptor = descriptors.FileDescriptorSet.decode(await readFile('target/contracts/descriptor.bin'));
  const names = new Set(descriptor.file.flatMap(file=>file.messageType.map(message=>`${file.package}.${message.name}`)));
  for (const capability of catalog.capabilities) {
    assert.ok(names.has(capability.request)); assert.ok(names.has(capability.response));
  }
  const ids = new Set(catalog.capabilities.map(capability=>capability.id));
  for (const required of ['platform.events','agent.decide','guard.check','knowledge.retrieve','memory.access','skill.invoke','show.plan','llm.generate','tts.synthesize','avatar.render','output.deliver','control.manage','storage.persist','runtime.observe','platform.capabilities','platform.lifecycle','avatar.profile','media.preview','channel.open','media.clock','audio.timeline','embedding.generate','rerank.rank','moderation.check','classification.classify','skill.list','gateway.invoke']) assert.ok(ids.has(`${required}@1`),required);
  const bad = structuredClone(catalog); bad.capabilities.push(bad.capabilities[0]);
  assert.throws(()=>validateSemantic('catalog',bad),/duplicate/);
});
test('avatar and UI schemas reject missing license fields or arbitrary HTML/script declarations', () => {
  const avatar = {schema_version:1,avatar_id:'test',version:'test',kind:'2d',license_ref:'fixture-only',assets:[{id:'body',path:'body.png',sha256:'a'.repeat(64),license_ref:'fixture-only'}],model_binding:'none-for-2d',resource_profile_ref:'candidate',supported_actions:['idle'],supported_emotions:['neutral'],voice_binding:'test',native_width:64,native_height:64,output_width:64,output_height:64,fps_num:25,fps_den:1,base_duration_samples:'48000',switch_points:['0'],personalization_refs:{}};
  valid('avatar',avatar);
  const bad = structuredClone(avatar); delete bad.assets[0].license_ref;
  assert.equal(check.avatar(bad),false);
  const ui = {schema_version:1,plugin_id:'test',config_schema_ref:'test',navigation:[{id:'test',label:'测试',component_ref:'approved:test',required_permission:'runtime.read'}]};
  valid('ui',ui);
  assert.equal(check.ui({...ui,html:'<script>test</script>'}),false);
  ui.navigation[0].component_ref='https://example.invalid/arbitrary.html';
  assert.equal(check.ui(ui),false);
});
test('backup schema requires bounded hashes/versions and rejects escaping content paths', () => {
  const backup = {schema_version:1,db_schema_version:5,created_at_utc_ms:'1',runtime_version:'0.1.0',contract_catalog_sha256:'a'.repeat(64),sensitive_scope:'configuration-facts-audit-registered-content;credential-references-only',database:{path:'state.sqlite3',sha256:'b'.repeat(64),bytes:1024},artifacts:[{path:`knowledge/${'c'.repeat(64)}`,sha256:'c'.repeat(64),bytes:5}]};
  assert.equal(check.backup(backup),true);
  assert.equal(check.backup({...backup,db_schema_version:2}),false);
  const bad=structuredClone(backup);bad.artifacts[0].path='../../outside';assert.equal(check.backup(bad),false);
  bad.artifacts=[];bad.database.bytes=268435457;assert.equal(check.backup(bad),false);
});
test('binding plan config requires explicit scope, bounded fallback and known mode syntax', () => {
  const plan={schema_version:1,session_id:'test',mode:'rehearsal',emits_output:false,requirements:[{capability_id:'agent.decide@1',required:true}],selections:[{capability_id:'agent.decide@1',primary_plugin_id:'test-primary',fallback_plugin_ids:['test-backup'],policy_scope_ref:'test-scope'}]};
  assert.equal(check.binding(plan),true);
  const bad=structuredClone(plan);delete bad.selections[0].policy_scope_ref;assert.equal(check.binding(bad),false);
  assert.equal(check.binding({...plan,mode:'unknown-auto'}),false);
  assert.equal(check.binding({...plan,is_trusted:true}),false);
});
test('OpenAPI preserves permissions, session revision/epoch, actual ACK and deferred jobs', async () => {
  const api = await apiDocument();
  assert.equal(api.paths['/sessions/{id}/mute'].post['x-required-permission'],'media.mute');
  assert.ok(api.components.schemas.SessionCommand.allOf[1].required.includes('generation_epoch'));
  assert.ok(api.components.schemas.SessionCommand.allOf[1].required.includes('device_id'));
  assert.ok(api.components.schemas.SessionCommand.allOf[1].required.includes('seq'));
  assert.equal(api['x-implementation-status'],'partial-loopback-engineering');
  assert.equal(api.paths['/sessions/{id}/mute'].post['x-implementation-status'],'implemented-loopback');
  assert.equal(api.paths['/sessions/{id}/start'].post['x-implementation-status'],undefined);
  assert.equal(api.paths['/approvals'].get['x-implementation-status'],'implemented-loopback');
  assert.equal(api.paths['/approvals/{id}/approve'].post['x-implementation-status'],'implemented-loopback');
  assert.equal(api.paths['/approvals/{id}/reject'].post['x-implementation-status'],'implemented-loopback');
  assert.ok(api.components.schemas.Command.required.includes('expected_state_revision'));
  assert.ok(api.components.schemas.ApprovalCommand.allOf[1].required.includes('candidate_hash'));
  assert.ok(api.paths['/knowledge/import'].post.responses['202']);
  assert.equal(api.components.schemas.CommandAck.properties.executed.type,'boolean');
  assert.equal(api.security[0].bearerAuth.length,0);
  const descriptor = descriptors.FileDescriptorSet.decode(await readFile('target/contracts/descriptor.bin'));
  const error = descriptor.file.flatMap(file=>file.enumType).find(type=>type.name==='ErrorCode');
  assert.deepEqual(api.components.schemas.ErrorResponse.properties.error.properties.code.enum.sort(),error.value.filter(value=>value.number!==0).map(value=>value.name).sort());
});
