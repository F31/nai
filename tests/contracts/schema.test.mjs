import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, readdir } from 'node:fs/promises';
import { validators, readJson, validateSemantic, apiDocument, AJV_SOURCES } from '../../scripts/contracts/validation.mjs';

const check = await validators();
const runtime = await readJson('tests/fixtures/contracts/runtime.json');
const catalog = await readJson('schemas/catalog/capabilities.json');
const provider = {schema_version:1,id:'tts-test',protocol:'http_stream',endpoint:'https://example.invalid/tts',model:'test',credential_ref:'credential:test',region:'test',data_policy_ref:'test',timeout_ms:1000,max_retries:1,max_concurrency:1,price_revision:'test'};
const plugin = {manifest_schema:1,plugin_id:'test-only',plugin_version:'0.1.0',kernel_range:'>=0.1.0,<1.0.0',protocol_major:1,contracts:['avatar.render@1'],supported_hosts:['subprocess'],activation:'on_session_start',idle_policy:'keep_warm_during_session',requires:['audio.timeline@1'],os:['windows'],architectures:['x86_64'],artifact_hash:'a'.repeat(64),license_ref:'fixture-not-a-license',install_bytes:'100',config_schema_ref:'test',data_schema_ref:'test',health_operation:'health',migration_ref:'none',rollback_range:'0.1.x',price_policy_ref:'test',resources:{profile_id:'test',certification_ref:'candidate-not-certified'},permissions:{network:false,media_output:true,secrets:[]}};
const model = {schema_version:1,model_id:'test-only',version:'1',license_ref:'fixture-not-a-license',graphs:[{id:'full-pipeline',file:{path:'pipeline.onnx',sha256:'b'.repeat(64),license_ref:'fixture'},external_weights:[],inputs:[{name:'audio',dtype:'float32',shape:[1,'samples']}],outputs:[{name:'frame',dtype:'float32',shape:[1,3,64,64]}],opset:17}],required_graph_ids:['full-pipeline'],preprocess_revision:'test',postprocess_revision:'test',sample_rate:16000,window_samples:640,hop_samples:640,lookahead_samples:0,padding:'zero',normalization:'test',frame_mapping:'test',fps_num:25,fps_den:1,runtime_matrix:[{os:'windows',arch:'x86_64',ort:'test',ort_crate:'test',ep:'cuda',precision:'fp32',driver:'test',cuda:'test',cudnn:'test',certification_ref:'candidate-not-certified'}],bom_ref:'test',golden_ref:'test'};
const platforms = await readJson('schemas/platform-matrix/platforms.json');

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
test('capability catalog declares a stage for every entry and no duplicate IDs', () => {
  const ids = new Set(catalog.capabilities.map(capability=>capability.id));
  assert.equal(ids.size, catalog.capabilities.length, 'duplicate capability id');
  for (const capability of catalog.capabilities) {
    assert.ok(['frozen','draft'].includes(capability.stage), capability.id);
    // A half-declared draft would make the stage field unreadable.
    if (capability.stage === 'draft') for (const key of ['request','response','stream','session_scoped']) assert.equal(capability[key], undefined, `${capability.id} is draft yet declares ${key}`);
  }
  for (const required of ['platform.events','agent.decide','guard.check','knowledge.retrieve','memory.access','skill.invoke','show.plan','llm.generate','tts.synthesize','avatar.render','output.deliver','control.manage','storage.persist','runtime.observe','platform.capabilities','platform.lifecycle','avatar.profile','media.preview','channel.open','media.clock','audio.timeline','embedding.generate','rerank.rank','moderation.check','classification.classify','skill.list','gateway.invoke']) assert.ok(ids.has(`${required}@1`),required);
  // The six §37.4 open-platform contracts are declared but not frozen. Pin the
  // set so "declared" cannot silently become "negotiable", nor vanish again.
  assert.deepEqual(catalog.capabilities.filter(capability=>capability.stage==='draft').map(capability=>capability.id).sort(), ['asr.transcribe@1','data.query@1','data.subscribe@1','device.control@1','device.sense@1','market.catalog@1']);
  const bad = structuredClone(catalog); bad.capabilities.push(bad.capabilities[0]);
  assert.throws(()=>validateSemantic('catalog',bad),/duplicate/);
  const half = structuredClone(catalog); half.capabilities.find(capability=>capability.stage==='draft').request='nai.v1.Handshake';
  assert.throws(()=>validateSemantic('catalog',half),/draft capability declares consumer messages/);
});
test('platform matrix states every capability on every platform under a closed state vocabulary', () => {
  valid('platforms',platforms);
  for (const capability of platforms.capabilities) assert.equal(typeof capability.note,'string',capability.id);
  // A state outside the three-value vocabulary is not a state.
  const unknown = structuredClone(platforms); unknown.capabilities[0].states.linux={state:'maybe'};
  assert.equal(check.platforms(unknown),false);
  // Unavailable is a state that carries a cause; a blank one is not registerable.
  const reasonless = structuredClone(platforms); reasonless.capabilities[0].states.macos={state:'explicit-unavailable'};
  assert.equal(check.platforms(reasonless),false);
  // Supported is not a state that carries a cause.
  const stray = structuredClone(platforms); stray.capabilities[0].states.linux={state:'supported',reason:'not_ported_to_target_os'};
  assert.equal(check.platforms(stray),false);
  // The schema deliberately does not enumerate reason codes: a plausible-looking
  // string must pass here and be judged against the SDK enum elsewhere.
  const plausible = structuredClone(platforms); plausible.capabilities[0].states.macos={state:'explicit-unavailable',reason:'not_a_real_reason'};
  assert.equal(check.platforms(plausible),true);
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
  const backup = {schema_version:1,db_schema_version:6,created_at_utc_ms:'1',runtime_version:'0.1.0',contract_catalog_sha256:'a'.repeat(64),sensitive_scope:'configuration-facts-audit-registered-content;credential-references-only',database:{path:'state.sqlite3',sha256:'b'.repeat(64),bytes:1024},artifacts:[{path:`knowledge/${'c'.repeat(64)}`,sha256:'c'.repeat(64),bytes:5}]};
  assert.equal(check.backup(backup),true);
  assert.equal(check.backup({...backup,db_schema_version:2}),false);
  // Pin the immediately-previous version too: a schema-version bump is a
  // deliberate compatibility break, and this sample is what goes stale when
  // only one side of the pair is updated.
  assert.equal(check.backup({...backup,db_schema_version:5}),false);
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
});

/// ── 每份 schema 要么被执行体消费，要么被具名声明（ADR 0051 决策 3）──────────────
///
/// 「有一份 schema」MUST NOT 被当作「有一份契约」：`schemas/tests/registry.schema.json`
/// 与 `schemas/rag-golden/case-set.schema.json` 都曾在**无人读取**的状态下存在。
/// 判据必须是**派生**的（扫源码），否则守卫会变成一张会过期的清单。

const SKIP_DIRS = new Set(['node_modules', '.git', 'target', 'graphify-out', 'dist', 'build', '.venv']);
const CODE_EXTENSIONS = ['.rs', '.mjs', '.ts', '.tsx', '.yml'];

async function codeFiles(dir = '.', out = []) {
  for (const entry of await readdir(dir, {withFileTypes: true})) {
    if (entry.isDirectory()) {
      if (SKIP_DIRS.has(entry.name)) continue;
      await codeFiles(`${dir}/${entry.name}`, out);
    } else if (CODE_EXTENSIONS.some(extension => entry.name.endsWith(extension))) {
      out.push(`${dir.replace(/^\.\//, '')}/${entry.name}`);
    }
  }
  return out;
}

/// 磁盘上的全部 schema（`schemas/**/*.schema.json`）。
async function schemasOnDisk(dir = 'schemas', out = []) {
  for (const entry of await readdir(dir, {withFileTypes: true})) {
    const next = `${dir}/${entry.name}`;
    if (entry.isDirectory()) await schemasOnDisk(next, out);
    else if (entry.name.endsWith('.schema.json')) out.push(next);
  }
  return out.sort();
}

/// 被消费的 schema ⇒ 它的消费来源。三条派生口径，MUST NOT 写死清单。
///
/// **本文件自己被排除在扫描之外**：它**按设计**要逐个写出 schema 路径（声明例外、构造注入），
/// 若把这些字面量算作「消费」，判据就会**自我满足**——`NAMED_EXCEPTIONS` 里的那份会被判成
/// 「既被消费又是例外」，而 `runtime` 之类的注入样本也会凭空获得一个假的消费者。
/// 排除之后，`upgrade-package` 只能由 Rust 的 `include_str!` 找到、其余由校验器清单与
/// 真正的加载点找到 ⇒ 三条来源**各自都被行使**。
const SELF = 'tests/contracts/schema.test.mjs';

async function consumedSchemas() {
  const consumed = new Map();
  const add = (schema, why) => {
    if (!consumed.has(schema)) consumed.set(schema, []);
    consumed.get(schema).push(why);
  };
  // ① 校验器清单（`schemas/<值>.schema.json`）——注意它不含 `.schema.json` 后缀，
  //    所以文本扫描抓不到它，必须由导出清单派生。
  for (const source of Object.values(AJV_SOURCES)) add(`schemas/${source}.schema.json`, 'the Ajv validator list');
  // ② Rust 侧 `include_str!`。`config/upgrade-package` 只在这里被消费。
  const rust = /include_str!\(\s*"(?:\.\.\/)*schemas\/([A-Za-z0-9_./-]+\.schema\.json)"/g;
  // ③ 脚本/工作流里的路径字面量。
  const literal = /['"`]schemas\/([A-Za-z0-9_./-]+\.schema\.json)['"`]/g;
  for (const file of await codeFiles()) {
    if (file === SELF) continue;
    const text = stripCommentLines(await readFile(file, 'utf8'), file.endsWith('.yml'));
    for (const pattern of [rust, literal]) {
      pattern.lastIndex = 0;
      for (const match of text.matchAll(pattern)) add(`schemas/${match[1]}`, file);
    }
  }
  return consumed;
}

/// 去掉**整行**注释后再扫描。
///
/// 不这么做，守卫会在**危险的方向**上变瞎：注释里提一句 schema 路径就会被算成「有消费者」，
/// 于是删掉真正的加载点、只留一句注释，孤儿照样判不出来——而「没人读的契约」正是本守卫
/// 唯一要抓的东西。整行口径是**成比例**的取舍：它挡住最常见的形态（文档注释/块注释引用路径），
/// 不去冒险解析行尾注释与字符串里的 `//`（`https://` 会因此被误切）。行尾注释仍算消费，
/// 这个假阳性方向是安全的：它只会让孤儿**更难**被漏掉，不会凭空造出消费者。
function stripCommentLines(text, hashComments) {
  const marker = hashComments ? /^\s*(\/\/|\*|\/\*|#)/ : /^\s*(\/\/|\*|\/\*)/;
  return text
    .split('\n')
    .filter(line => !marker.test(line))
    .join('\n');
}

/// 具名例外：**为什么这份 schema 不接线**。每条 MUST 有非空理由——
/// 具名例外的全部价值就是那条理由，缺了它例外就退化成豁免。
const NAMED_EXCEPTIONS = new Map([
  [
    'schemas/rag-golden/case-set.schema.json',
    '载荷格式的**文档陈述**（供题集产出方按结构手写 *.json）。执行体是 xtask/src/golden.rs 的 parse_case_set：' +
      '它对每类缺陷产出稳定 token（unknown_field / field_missing / case_set_schema_unsupported…），调用方据此定位。' +
      '再接一份 JSON Schema 会形成同一形状的第二处判定，而两份判定的失败措辞不同 ⇒ 只会让「为什么被拒」有两个答案。',
  ],
]);

function coverageErrors(all, consumed, exceptions) {
  const errors = [];
  for (const schema of all) {
    const reasons = consumed.get(schema);
    const exception = exceptions.get(schema);
    if (reasons && exception) {
      errors.push(`${schema} is both consumed (${reasons.join(', ')}) and named as an exception; drop the exception`);
    } else if (!reasons && !exception) {
      errors.push(`${schema} is consumed by no execution path and is not named as an exception — a schema nobody reads is not a contract`);
    } else if (exception !== undefined && exception.trim() === '') {
      errors.push(`${schema} is named as an exception without a reason; a named exception is exactly its reason`);
    }
  }
  for (const schema of exceptions.keys()) {
    if (!all.includes(schema)) errors.push(`the exception list names ${schema}, which is not on disk`);
  }
  return errors;
}

test('every schema under schemas/ is enforced by an execution path or named with a reason', async () => {
  const all = await schemasOnDisk();
  const consumed = await consumedSchemas();
  assert.deepEqual(coverageErrors(all, consumed, NAMED_EXCEPTIONS), []);
  // 非空洞：扫描必须真的走完了三条来源，而不是「什么都没找到所以没有孤儿」。
  assert.ok(all.length > 1, `expected more than one schema on disk, found ${all.length}`);
  assert.ok(
    consumed.has('schemas/config/upgrade-package.schema.json'),
    'the Rust-side consumption (include_str!) must be found — otherwise the scan only covers Node',
  );
  assert.ok(
    consumed.has('schemas/platform-matrix/platforms.schema.json') &&
      consumed.has('schemas/tests/registry.schema.json'),
    'the literal scan and the validator list must both contribute',
  );
  assert.ok(
    NAMED_EXCEPTIONS.size > 0 && all.includes([...NAMED_EXCEPTIONS.keys()][0]),
    'the named exception must name a schema that actually exists',
  );
  // 注入一：某份被消费的 schema 从消费集合里消失 ⇒ 立刻变成孤儿。
  const dropped = new Map(consumed);
  dropped.delete('schemas/config/runtime.schema.json');
  assert.ok(
    coverageErrors(all, dropped, NAMED_EXCEPTIONS).some(
      error => error.includes('schemas/config/runtime.schema.json') && error.includes('nobody reads'),
    ),
    'a schema that drops out of the validator list must be reported as an orphan',
  );
  // 注入二：给一份**已被消费**的 schema 也加例外 ⇒ 报「既是消费又是例外」。
  const overlapping = new Map(NAMED_EXCEPTIONS);
  overlapping.set('schemas/config/runtime.schema.json', '想给它开个口子');
  assert.ok(
    coverageErrors(all, consumed, overlapping).some(
      error => error.includes('both consumed') && error.includes('drop the exception'),
    ),
    'an exception that shadows a real consumer must be reported',
  );
  // 注入三：例外缺理由 ⇒ 失败（理由就是例外的全部价值）。
  assert.ok(
    coverageErrors(all, consumed, new Map([[ 'schemas/rag-golden/case-set.schema.json', '   ' ]])).some(error =>
      error.includes('without a reason'),
    ),
    'an exception without a reason must fail',
  );
  // 注入四：例外指向一份不存在的 schema ⇒ 陈旧例外也要报。
  assert.ok(
    coverageErrors(all, consumed, new Map([['schemas/rag-golden/gone.schema.json', '陈旧']])).some(error =>
      error.includes('not on disk'),
    ),
    'an exception naming a schema that does not exist must fail',
  );
  // 注入五：注释里的路径 MUST NOT 被算作消费——否则「删掉真正的加载点、只留一句注释」就能骗过守卫，
  // 而这正是本守卫唯一要抓的东西（没人读的契约）。同时确认代码里的路径仍然算数。
  const pathLike = /schemas\/[A-Za-z0-9_./-]+\.schema\.json/;
  assert.ok(
    !pathLike.test(stripCommentLines('// schemas/rag-golden/case-set.schema.json', false)),
    'a path mentioned in a whole-line comment must not count as a consumer',
  );
  assert.ok(
    !pathLike.test(stripCommentLines(' * see schemas/rag-golden/case-set.schema.json', false)),
    'a path mentioned in a block-comment continuation must not count either',
  );
  assert.ok(
    !pathLike.test(stripCommentLines('# schemas/rag-golden/case-set.schema.json', true)),
    'a path mentioned in a workflow comment must not count either',
  );
  assert.ok(
    pathLike.test(stripCommentLines("await readFile('schemas/catalog/catalog.schema.json', 'utf8')", false)),
    'a path used in code must still count as a consumer',
  );
});
