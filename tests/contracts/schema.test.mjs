import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, readdir, writeFile, mkdir, rm } from 'node:fs/promises';
import path from 'node:path';
import { validators, readJson, validateSemantic, apiDocument, apiDocuments, AJV_SOURCES, TS_GENERATED_SOURCES } from '../../scripts/contracts/validation.mjs';
import { parseIndex, readTrackedPaths, describeFailure } from '../../scripts/contracts/git-index.mjs';
import {
  CONFIG_SCHEMAS,
  EXTERNAL_READERS,
  guardedKeys,
  keyLabel,
  consumersOf,
  readSites,
  indexShape,
  helperShape,
  keyHelpersByFile,
  serdeFieldsByFile,
  WIRED,
  UNWIRED,
  UNWIRED_CAUSES,
  partitionErrors,
  evidenceErrors,
  causeErrors,
  attributionErrors,
  scanExclusionErrors,
  SCAN_EXCLUSIONS,
  inScan,
} from '../../scripts/contracts/config-consumption.mjs';
import {
  HTTP_METHODS,
  PATH_ITEM_FIELDS,
  OPENAPI_DIR,
  OPENAPI_DOCUMENTS,
  documentSetErrors,
  openapiDocumentPaths,
  operations,
  operationShapeErrors,
  operationErrors,
} from '../../scripts/contracts/openapi-ops.mjs';

/// 本文件判据用到的两份描述与它们**各自**的已实现声明。W86 起「已实现的是哪些操作」是
/// **每一份文档各自的属性**（`remote-knowledge.yaml` 自述 `client-only`，它的表为空）——
/// 把 `control.yaml` 的八条套上去只会得到八条孤儿声明。
const CONTROL = 'schemas/openapi/control.yaml';
const REMOTE = 'schemas/openapi/remote-knowledge.yaml';
const controlDeclared = OPENAPI_DOCUMENTS[CONTROL].implemented;

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
  const backup = {schema_version:1,db_schema_version:7,created_at_utc_ms:'1',runtime_version:'0.1.0',contract_catalog_sha256:'a'.repeat(64),sensitive_scope:'configuration-facts-audit-registered-content;credential-references-only',database:{path:'state.sqlite3',sha256:'b'.repeat(64),bytes:1024},artifacts:[{path:`knowledge/${'c'.repeat(64)}`,sha256:'c'.repeat(64),bytes:5}]};
  assert.equal(check.backup(backup),true);
  assert.equal(check.backup({...backup,db_schema_version:2}),false);
  // Pin the immediately-previous version too: a schema-version bump is a
  // deliberate compatibility break, and this sample is what goes stale when
  // only one side of the pair is updated.
  assert.equal(check.backup({...backup,db_schema_version:6}),false);
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

/// ── OpenAPI 操作集的闭世界（W85，ADR 0063；W86 起逐文档参数化）─────────────────
///
/// `apiDocument()` 的两道逐操作检查（权限声明、实现状态）原先都靠一张**手写的方法清单**
/// （`['get','post','put','delete']`）。手写清单的补集就是盲区：往文档里加一个 `patch` 操作，
/// 既不查权限、也不查状态。W85 探针实测（SwaggerParser 校验 + 逐字复刻那段循环）：
/// 加一个无权限、无状态的 `patch` ⇒ 判定返回**空错误**；同位置换成 `get` ⇒ 立刻报错。
/// **SwaggerParser 两种情况都放行**——它管结构合法性，不管覆盖。
///
/// 补法是闭世界（与 W81／W83／W84 同形）：触发**只由「文档里存在这个方法键」决定**；
/// 方法键集封闭、与 path-item 字段集**不相交**；已实现声明 MUST **无孤儿**；且 MUST 非空洞。
///
/// W86 把同一句话推到**上一层**：上面说完的是「哪些**操作**被校验」，而判据原先只被喂
/// **一份**文档（路径硬编码）。⇒ `declared` 从此是**必需入参**，每份文档带**自己**的已实现表。

test('the real API description passes the operation-set closure', async () => {
  const api = await apiDocument();
  // 非空洞：判据 MUST 真的看到了操作，而且**不只是**已实现的那几条。
  const seen = operations(api);
  assert.ok(
    seen.length > controlDeclared.length,
    `the closure must see operations beyond the implemented subset; saw ${seen.length}`,
  );
  // 闭世界：真实文档零错误（收紧后不引入红）。
  assert.deepEqual(operationErrors(api, controlDeclared), [], 'the real description must already be closed');
  // 前提的另一半：已实现声明**每一条**都能在文档里解析出来——否则上一条可能只是「什么都没看」。
  for (const entry of controlDeclared) {
    assert.ok(
      seen.some(op => `${op.method} ${op.path}` === entry),
      `${entry} must resolve to a real operation`,
    );
  }
});

test('an operation under a method nobody listed is judged, not skipped', async () => {
  const api = await apiDocument();
  // 洞的**现场**：`patch` 不在任何手写清单里，且既无权限、也无状态。
  const probe = structuredClone(api);
  probe.paths['/w85-probe'] = { parameters: [], patch: { responses: { '204': { description: 'No content' } } } };
  assert.ok(
    operationErrors(probe, controlDeclared).some(error => error.includes('patch /w85-probe') && error.includes('x-required-permission')),
    'a patch operation with no permission declaration must be named',
  );
  // 判据**遍历方法键**、不遍历手写子集：同一个缺陷换成 `get` 走的是同一条路径（对照组）。
  const asGet = structuredClone(api);
  asGet.paths['/w85-probe'] = { get: { responses: { '204': { description: 'No content' } } } };
  assert.ok(
    operationErrors(asGet, controlDeclared).some(error => error.includes('get /w85-probe') && error.includes('x-required-permission')),
    'the same defect under a listed method must be caught by the same rule',
  );
});

test('a path-item key that is neither a method nor a field is named', async () => {
  const api = await apiDocument();
  const probe = structuredClone(api);
  probe.paths['/w85-probe'] = { gets: { responses: { '204': { description: 'No content' } } } };
  assert.ok(
    operationErrors(probe, controlDeclared).some(error => error.includes('"gets"') && error.includes('/w85-probe')),
    'a key nobody recognises must be named together with its path',
  );
});

test('a new method done correctly is not rejected, and x- extensions are allowed', async () => {
  // **反向注入**：本轮补的是「**检查**新方法」，MUST NOT 变成「**拒绝**新方法」。
  const api = await apiDocument();
  const probe = structuredClone(api);
  probe.paths['/w85-probe'] = {
    'x-internal': true,
    parameters: [],
    patch: { 'x-required-permission': 'session.manage', responses: { '204': { description: 'No content' } } },
  };
  assert.deepEqual(
    operationErrors(probe, controlDeclared),
    [],
    'a new method with a permission declaration, plus a spec extension, must not fail the judge',
  );
});

test('a declared implementation must have an operation and a matching status', async () => {
  const api = await apiDocument();
  // **孤儿声明**：删掉一个已实现的路径 ⇒ 声明还在，对象没了。
  const orphaned = structuredClone(api);
  delete orphaned.paths['/approvals'];
  assert.ok(
    operationErrors(orphaned, controlDeclared).some(error => error.includes('get /approvals') && error.includes('no such operation')),
    'a declaration with no object must be named',
  );
  // **状态不符**：一条已声明的实现被改成别的状态。
  const mismatched = structuredClone(api);
  mismatched.paths['/health'].get['x-implementation-status'] = 'partial';
  assert.ok(
    operationErrors(mismatched, controlDeclared).some(error => error.includes('get /health') && error.includes('expected')),
    'a declared implementation whose status disagrees must be named',
  );
});

test('an empty or unreadable API description is reported, not silently closed', () => {
  assert.ok(
    operationErrors({ paths: {} }, []).some(error => error.includes('exposes no operations')),
    'a description with no operations makes every per-operation check vacuously true',
  );
  assert.ok(
    operationErrors(null, []).some(error => error.includes('no readable paths')),
    'an unreadable description must fail rather than pass',
  );
});

test('the operation tables are disjoint, closed and non-empty', () => {
  // 真实两张表自洽。
  assert.deepEqual(operationShapeErrors(), [], 'the shipped tables must be consistent');
  // **相邻层不相交**（W82）：一个键不能既是方法又是字段。
  assert.ok(
    operationShapeErrors(HTTP_METHODS, [...PATH_ITEM_FIELDS, 'get']).some(error => error.includes('overlap')),
    'a key claimed by both layers must be named',
  );
  // 非空洞：空方法集／空字段集，各是一种「谁都挡不住」的形状。
  assert.ok(operationShapeErrors([], PATH_ITEM_FIELDS).some(error => error.includes('HTTP_METHODS is empty')));
  assert.ok(operationShapeErrors(HTTP_METHODS, []).some(error => error.includes('PATH_ITEM_FIELDS is empty')));
});

/// ── OpenAPI **文档集**的闭世界（W86，ADR 0064）───────────────────────────────
///
/// W85 修好了「哪些**操作**被校验」，但「哪些**文档**被校验」仍是一张手写清单：
/// `apiDocument()` 把路径硬编码成 `control.yaml`。同目录的 `remote-knowledge.yaml` 是它的补集
/// ——**本地门控从不读它**（唯一读它的 `generate.mjs` 只在 CI 的 `cargo xtask schema --check`
/// 里跑，而 `schema --check` **不在本机门控闭包里**——它的工具链前提是 `protoc`，超出 portable 门控
/// 「只需 Rust 工具链」的既有不变式；成因见 W108 / ADR 0086），它自己的权限检查又是**同一件事的第二处定义**。
/// W86 探针实测：往 remote 加一个无权限的 `patch`，`SwaggerParser` 放行、本机门控全绿。
///
/// 补法仍是闭世界（同族第七处）：文档集由**已跟踪路径**派生，逐文档声明是**单一来源**，
/// 判据逐文档参数化且 `declared` **必需**。

test('every tracked OpenAPI description is declared, and every declaration has a description', async () => {
  const repository = await readTrackedPaths();
  assert.ok(repository.ok, repository.ok ? '' : describeFailure(repository));
  // 非空洞：文档集必须真的非空，否则闭世界对空集恒真。
  const onDisk = openapiDocumentPaths(repository.paths);
  assert.ok(onDisk.length >= 2, `expected the repository's OpenAPI descriptions, saw ${JSON.stringify(onDisk)}`);
  assert.deepEqual(documentSetErrors(repository.paths), [], 'the shipped declaration set must match the repository');
  // 注入一：在册描述**未声明** ⇒ 具名（这正是 W86 的洞）。
  const dropped = { ...OPENAPI_DOCUMENTS };
  delete dropped[REMOTE];
  assert.ok(
    documentSetErrors(repository.paths, dropped).some(error => error.includes(REMOTE) && error.includes('nobody declares')),
    'a tracked description with no declaration must be named',
  );
  // 注入二：**陈旧声明**（指向不存在的文档）⇒ 具名。
  assert.ok(
    documentSetErrors(repository.paths, {
      ...OPENAPI_DOCUMENTS,
      'schemas/openapi/gone.yaml': { status: 'x', implemented: [] },
    }).some(error => error.includes('gone.yaml') && error.includes('no document')),
    'a declaration with no document must be named',
  );
  // 注入三：声明集清空 ⇒ 每份描述都逃逸。
  assert.ok(documentSetErrors(repository.paths, {}).some(error => error.includes('is empty')));
  // 注入四：没有一份文档声明已实现 ⇒ 「声明无孤儿」在每份文档上空转。
  const noneImplemented = Object.fromEntries(
    Object.entries(OPENAPI_DOCUMENTS).map(([docPath, entry]) => [
      docPath,
      { status: entry.status, implemented: [] },
    ]),
  );
  assert.ok(
    documentSetErrors(repository.paths, noneImplemented).some(error =>
      error.includes('vacuously true on every document'),
    ),
    'an all-empty implemented set makes the orphan half vacuous',
  );
  // 非空洞：缺 tracked 输入 ⇒ 具名失败，MUST NOT 被当成「仓库里没有文档」。
  assert.ok(documentSetErrors(undefined).some(error => error.includes('requires the tracked-path set')));
  // 闭世界**只看名字**：目录与扩展名各是一层，别的文件不参与。
  assert.deepEqual(
    openapiDocumentPaths(new Set([
      `${OPENAPI_DIR}/a.yaml`,
      `${OPENAPI_DIR}/b.yml`,
      `${OPENAPI_DIR}/c.json`,
      'other/e.yaml',
    ])),
    [`${OPENAPI_DIR}/a.yaml`, `${OPENAPI_DIR}/b.yml`],
    'the document set is decided by directory and extension, not by content',
  );
});

test('the second description is judged by the same closure as the first', async () => {
  const documents = await apiDocuments();
  assert.deepEqual(
    Object.keys(documents).sort(),
    [CONTROL, REMOTE].sort(),
    'every declared description must be validated',
  );
  // 非空洞：remote 真的暴露了操作（判据的输入不是空的）。
  assert.equal(operations(documents[REMOTE]).length, 6);
  assert.deepEqual(
    operationErrors(documents[REMOTE], OPENAPI_DOCUMENTS[REMOTE].implemented),
    [],
    'the shipped remote description must already be closed',
  );
  // 洞的**现场**：往 remote 加一个无权限的操作 ⇒ 与 control 走**同一条**判据。
  const probe = structuredClone(documents[REMOTE]);
  probe.paths['/w86-probe'] = { patch: { responses: { '204': { description: 'No content' } } } };
  assert.ok(
    operationErrors(probe, OPENAPI_DOCUMENTS[REMOTE].implemented).some(
      error => error.includes('patch /w86-probe') && error.includes('x-required-permission'),
    ),
    'an operation in the second description must be judged, not skipped',
  );
  // **反向注入**：一份合法的新操作 MUST NOT 红——补的是「**覆盖**文档集」，不是「只允许一份文档」。
  const legit = structuredClone(documents[REMOTE]);
  legit.paths['/w86-probe'] = {
    patch: { 'x-required-permission': 'knowledge.read', responses: { '204': { description: 'No content' } } },
  };
  assert.deepEqual(operationErrors(legit, OPENAPI_DOCUMENTS[REMOTE].implemented), []);
  // 反向：声明顺序不承载语义。
  assert.deepEqual(operationErrors(documents[CONTROL], [...controlDeclared].reverse()), []);
});

test('a per-document declaration must be injected, not defaulted', async () => {
  const api = await apiDocument();
  // 缺参 MUST 具名失败：静默退回**另一份文档**的声明会让判据的输入不再等于被断言的对象。
  assert.ok(operationErrors(api).some(error => error.includes('own implemented-declaration array')));
  assert.ok(operationErrors(api, 'get /health').some(error => error.includes('own implemented-declaration array')));
  // 用错文档的声明是**有害的**：control 的八条在 remote 上全是孤儿声明。
  const remote = (await apiDocuments())[REMOTE];
  assert.equal(
    operationErrors(remote, controlDeclared).filter(error => error.includes('no such operation')).length,
    controlDeclared.length,
  );
});

test('the declaration shape of every document is checked and names the document', () => {
  // 逐文档的声明形状与文档集闭世界同住 `documentSetErrors`——错误措辞 MUST 带上「是哪一份文档」。
  const errors = documentSetErrors(new Set([CONTROL]), {
    [CONTROL]: { status: 'partial-loopback-engineering', implemented: ['fetch /x', 'get /a', 'get /a'] },
  });
  assert.ok(errors.some(error => error.includes('not a recognised HTTP method')), 'an unknown method must be named');
  assert.ok(errors.some(error => error.includes('twice')), 'a duplicate declaration must be named');
  assert.ok(errors.every(error => error.includes(CONTROL)), 'every declaration error must name its document');
  assert.ok(
    documentSetErrors(new Set([CONTROL]), { [CONTROL]: { implemented: [] } }).some(error =>
      error.includes('has no status'),
    ),
    'a declaration without a status must be named',
  );
  assert.ok(
    documentSetErrors(new Set([CONTROL]), { [CONTROL]: { status: 'x' } }).some(error =>
      error.includes('no implemented list'),
    ),
    'a declaration without an implemented list must be named',
  );
});

test('the generator reuses the operation closure instead of restating it', async () => {
  const generator = await readFile('scripts/contracts/generate.mjs', 'utf8');
  assert.ok(generator.includes('operationErrors'), 'the generator must call the single judge');
  assert.ok(generator.includes('OPENAPI_DOCUMENTS'), 'the generator must read the per-document declaration');
  // 防回退：**代码里**（整行注释不算，W74）MUST NOT 再有第二份权限规则。
  assert.ok(
    !stripCommentLines(generator, 'slash').includes('x-required-permission'),
    'the permission rule must live in exactly one place',
  );
  // 判据本身也不再硬编码某一份文档的路径。
  const validation = await readFile('scripts/contracts/validation.mjs', 'utf8');
  assert.ok(
    !validation.includes("SwaggerParser.validate('schemas/openapi/control.yaml')"),
    'the judge must not hard-code one document',
  );
});

/// ── 每份 schema 要么被执行体消费，要么被具名声明（ADR 0051 决策 3 / ADR 0052）────────
///
/// 「有一份 schema」MUST NOT 被当作「有一份契约」：`schemas/tests/registry.schema.json`
/// 与 `schemas/rag-golden/case-set.schema.json` 都曾在**无人读取**的状态下存在。
/// 判据必须是**派生**的（扫源码），否则守卫会变成一张会过期的清单。
///
/// w74 修了三处边界，每一处原先都是一个洞：
/// ① **扫描集的形状**——从仓库根递归，**跳过所有以 `.` 开头的目录**（结构性规则），
///    再叠加一份**具名排除**（每条 MUST 被 `.gitignore` 忽略）。原先枚举 `SKIP_DIRS`
///    字符串，那是第二张会过期的清单（`build`/`.venv` 两条既不在 `.gitignore` 也不在磁盘上）。
/// ② **语言闭包**——`CODE_EXTENSIONS` 由 `CODE_LANGUAGES` **派生**，于是「新增一个可扫描
///    扩展名」与「声明它的注释语言」是**同一件事**。原先两者分居两处，`.yaml` 与 `.yml`
///    各漏一半，而**唯一暴露这点的那份文件**恰好因为扩展名叫 `.yaml` 而不在扫描集里。
/// ③ **每条来源自负其责**——判据从「三条来源各自都被行使」换成**独有覆盖**。原先那句断言
///    只由 Ajv 清单满足，删掉整条路径字面量来源，守卫照旧全绿 ⇒ 它不是守卫，是装饰。

/// 可扫描扩展名 → 注释语言。**扫描集从这张表派生**，所以「扫了某个扩展名却没声明它的
/// 注释语言」在结构上不存在。`.yml` 与 `.yaml` 是同一种语言，MUST 一起出现。
const CODE_LANGUAGES = new Map([
  ['.rs', 'slash'],
  ['.mjs', 'slash'],
  ['.js', 'slash'],
  ['.ts', 'slash'],
  ['.tsx', 'slash'],
  ['.yml', 'hash'],
  ['.yaml', 'hash'],
  ['.sh', 'hash'],
]);
const CODE_EXTENSIONS = [...CODE_LANGUAGES.keys()];

/// 具名排除：**为什么这个目录不算仓库内容**。证据就是 `.gitignore`——一个目录被声明为
/// 「不是仓库的一部分」，把它排除掉就不可能藏起仓库内容。⇒ 排除不是豁免，是有证据的陈述。
const EXCLUDED_DIRS = ['node_modules', 'target', 'graphify-out', 'dist'];

async function codeFiles(dir = '.', extensions = CODE_EXTENSIONS, skipDotDirs = true, out = []) {
  for (const entry of await readdir(dir, {withFileTypes: true})) {
    if (entry.isDirectory()) {
      // 结构性规则：`.` 开头的目录一视同仁跳过（含 `.git`——版本控制内部目录，
      // 不是工作区内容）。不跳的那一半只额外放行 `.git` 之外的 dot 目录，好让
      // 「排除无害」那条断言有个可以对比的参照。
      if (skipDotDirs && entry.name.startsWith('.')) continue;
      if (!skipDotDirs && entry.name === '.git') continue;
      if (EXCLUDED_DIRS.includes(entry.name)) continue;
      await codeFiles(`${dir}/${entry.name}`, extensions, skipDotDirs, out);
    } else if (extensions.some(extension => entry.name.endsWith(extension))) {
      // 路径 MUST 是 POSIX 相对路径（`apps/engine/src/lib.rs`）。初始目录是 `'.'` 时若直接
      // 用 `${dir}/${name}` 会得到 `./apps/...`——那个前缀让扫描集与索引里的路径**不可比**，
      // 于是「证据来源在不在仓库里」这条断言会对根目录文件**静默假阴性**。
      const prefix = dir === '.' ? '' : `${dir.replace(/^\.\//, '')}/`;
      out.push(`${prefix}${entry.name}`);
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

/// 被消费的 schema ⇒ 它的消费来源。来源是**具名的两条**，MUST NOT 写死清单。
///
/// **本文件自己被排除在扫描之外**：它**按设计**要逐个写出 schema 路径（声明例外、构造注入），
/// 若把这些线索算作「消费」，判据就会**自我满足**——`NAMED_EXCEPTIONS` 里的那份会被判成
/// 「既被消费又是例外」，而注入样本也会凭空获得一个假的消费者。
///
/// w74 移除了原先的第三条来源（脚本/工作流里的**路径字面量**）。实测它**零独有覆盖**：它找到的
/// 每一份也都在 Ajv 清单里，删掉它没有任何断言变红。而它是**唯一**会读任意扩展名文件的来源
/// ⇒ 一个未跟踪的临时脚本就能凭空造出消费者，把一个真孤儿洗绿。按「假阳性与假阴性不对称，
/// 永远选安全方向」，这条来源被移除：将来若真有 schema 只被某个脚本加载，它会被报成孤儿
/// （假阳性，安全），加进 `AJV_SOURCES` 即可。
const SELF = 'tests/contracts/schema.test.mjs';
const AJV_SOURCE_LABEL = 'the Ajv validator list';
const RUST_SOURCE_LABEL = 'the Rust-side include_str!';
const RUST_INCLUDE = /include_str!\(\s*"(?:\.\.\/)*schemas\/([A-Za-z0-9_./-]+\.schema\.json)"/g;

async function consumptionOf(files) {
  const consumed = new Map();
  /// **提供了消费证据的文件**。判据要断言它们都在仓库里（见 `repositoryErrors`），
  /// 所以来源不能只是「哪条规则找到的」——还得知道「是哪个文件找到的」。
  const evidenceSources = new Set();
  const add = (schema, why) => {
    if (!consumed.has(schema)) consumed.set(schema, []);
    if (!consumed.get(schema).includes(why)) consumed.get(schema).push(why);
  };
  // ① 校验器清单（`schemas/<值>.schema.json`）——注意它不含 `.schema.json` 后缀，
  //    所以文本扫描抓不到它，必须由导出清单派生。
  for (const source of Object.values(AJV_SOURCES)) add(`schemas/${source}.schema.json`, AJV_SOURCE_LABEL);
  // ② Rust 侧 `include_str!`。`config/upgrade-package` 只在这里被消费。
  for (const file of files) {
    if (file === SELF) continue;
    const text = stripCommentLines(await readFile(file, 'utf8'), CODE_LANGUAGES.get(path.extname(file)));
    RUST_INCLUDE.lastIndex = 0;
    const hits = [...text.matchAll(RUST_INCLUDE)];
    for (const match of hits) add(`schemas/${match[1]}`, RUST_SOURCE_LABEL);
    if (hits.length > 0) evidenceSources.add(file);
  }
  return { consumed, evidenceSources };
}

/// 只有某条来源能找到的 schema。空集 ⇒ 这条来源对判据**零贡献**：
/// 删掉它不会有任何断言变红，那它就不是守卫，是装饰。
function exclusiveTo(consumed, label) {
  return [...consumed]
    .filter(([, reasons]) => reasons.length === 1 && reasons[0] === label)
    .map(([schema]) => schema)
    .sort();
}

/// 每条来源 MUST 自负其责：至少找到一份**只有它能找到**的 schema。
function sourceErrors(consumed, labels) {
  return labels
    .filter(label => exclusiveTo(consumed, label).length === 0)
    .map(label => `${label} finds no schema that no other source finds; a source with no exclusive hit changes no verdict — remove it or merge it`);
}

/// 提供消费证据的文件 MUST 是**仓库里已跟踪的文件**。
///
/// 这是 F2 的**精确**修复。W74 的守卫把「扫描工作区」当成「扫描仓库」：一个未跟踪的
/// `.tmp-fake-consumer.mjs`（内含一句 schema 路径）就能把一个**真孤儿**洗绿——而「有一份 schema
/// 没人读」正是本守卫唯一要抓的东西。W74 只能靠**结构性收敛**缩小这个缺口，并把残余风险
/// 登记为「CI 上不存在」。**缺口本身可以关掉**：`.git/index`（DIRC）是纯 JS 可读的，
/// 它给出的路径集就是「已跟踪文件集」，而「未跟踪」正是那个入口。
///
/// 约束的是**证据来源**而不是**整个扫描集**：若要求「扫描集 ⊆ 索引」，那么任何尚未 `git add`
/// 的代码文件都会让门控红——而本程序的闭环顺序是「门控 → … → 提交时才 add」，
/// 新变更目录里的 `.yaml` 就会让门控**假红**。证据来源恒为已提交的老文件，零假阳性，
/// 而攻击面恰好就在证据来源上。
function repositoryErrors(evidenceSources, tracked) {
  return [...evidenceSources]
    .filter(file => !tracked.has(file))
    .sort()
    .map(file => `${file} provides consumption evidence but version control does not track it — only a file the repository has actually taken in can vouch for a schema, otherwise a stray file left in the working tree silently covers for a schema nobody reads`);
}

/// 去掉**整行**注释后再扫描。注释语言由扩展名在 `CODE_LANGUAGES` 里的条目给出——
/// 按**语言**判断，不是按「这一份文件的扩展名是不是恰好叫 `.yml`」。
///
/// 原先那个写法让 `.yaml` 落到「没有 `#` 注释语法」一侧；而 `tests/registry.yaml` 那份文件
/// 又恰好因为扩展名不在扫描集里，连「被误判」都轮不上——两个缺陷就这样互相掩盖，
/// 而它正是 W73 写下「注释里的路径不算消费」那句注释的地方。
///
/// 不这么做的后果是守卫在**危险的方向**上变瞎：注释里提一句 schema 路径就会被算成「有消费者」，
/// 于是删掉真正的加载点、只留一句注释，孤儿照样判不出来——而「没人读的契约」正是本守卫
/// 唯一要抓的东西。整行口径是**成比例**的取舍：它挡住最常见的形态（文档注释/块注释引用路径），
/// 不去冒险解析行尾注释与字符串里的 `//`（`https://` 会因此被误切）。行尾注释仍算消费，
/// 这个假阳性方向是安全的：它只会让孤儿**更难**被漏掉，不会凭空造出消费者。
function stripCommentLines(text, language) {
  const marker = language === 'hash' ? /^\s*(\/\/|\*|\/\*|#)/ : /^\s*(\/\/|\*|\/\*)/;
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

/// 具名排除 MUST 付得起理由：**证据是 `.gitignore`**——一个目录被声明为「不是仓库的
/// 一部分」，把它排除掉就不可能藏起仓库内容。缺了这条证据，排除就退化成豁免。
function exclusionErrors(dirs, ignoreLines) {
  const ignored = ignoreLines.map(line => line.trim().replace(/^\//, '').replace(/\/$/, ''));
  return dirs
    .filter(dir => !ignored.includes(dir))
    .map(dir => `${dir} is excluded from the scan but .gitignore does not ignore it — an exclusion must say why it cannot hide repository content`);
}

/// 生成面与校验面 MUST 读同一份清单：生成面 ⊆ 校验面、basename 互异、生成目录与清单**双向相等**。
function generatorErrors(sources, validated, generated) {
  const errors = [];
  for (const source of sources) {
    if (!validated.includes(source)) errors.push(`${source} is generated into TypeScript but is not in the validator list`);
  }
  const basenames = sources.map(source => path.posix.basename(source));
  if (new Set(basenames).size !== basenames.length) errors.push('two schemas would write the same generated file');
  const expected = [...new Set(basenames)].sort();
  for (const name of expected) {
    if (!generated.includes(name)) errors.push(`the list declares ${name} but the generated directory has no ${name}.ts`);
  }
  for (const name of generated) {
    if (!expected.includes(name)) errors.push(`the generated directory has ${name}.ts but the list does not declare it`);
  }
  return errors;
}

test('every schema under schemas/ is enforced by an execution path or named with a reason', async () => {
  const all = await schemasOnDisk();
  const files = await codeFiles();
  const { consumed, evidenceSources } = await consumptionOf(files);
  assert.deepEqual(coverageErrors(all, consumed, NAMED_EXCEPTIONS), []);
  assert.deepEqual(sourceErrors(consumed, [AJV_SOURCE_LABEL, RUST_SOURCE_LABEL]), []);
  // 判据的输入 MUST 等于它断言的对象：要断言「**仓库**里没有孤儿」，就必须读**仓库**。
  const repository = await readTrackedPaths();
  assert.ok(repository.ok, repository.ok ? '' : describeFailure(repository));
  assert.deepEqual(repositoryErrors(evidenceSources, repository.paths), []);
  // 非空洞：证据来源 MUST 非空，否则「来源 ⊆ 索引」对空集恒真。
  assert.ok(evidenceSources.size > 0, 'the scan must reach at least one file that reads a schema');
  // 注入：把一个**未跟踪**的文件也算作证据来源 ⇒ 立刻被指名（F2 的全部内容就是这个形态）。
  assert.ok(
    repositoryErrors([...evidenceSources, 'scripts/w75-stray-consumer.mjs'], repository.paths).some(
      error => error.includes('scripts/w75-stray-consumer.mjs') && error.includes('does not track it'),
    ),
    'an untracked file that reads a schema must be reported',
  );
  // 非空洞：那个**未跟踪但未被忽略**的位置必须真的不在索引里，否则上面的注入是空转。
  assert.ok(
    !repository.paths.has('scripts/w75-stray-consumer.mjs'),
    'the injected stray path must not exist in the index',
  );
  // 非空洞：扫描必须真的走完了全部来源与全部可扫描扩展名，而不是「什么都没找到所以没有孤儿」。
  assert.ok(all.length > 1, `expected more than one schema on disk, found ${all.length}`);
  assert.ok(
    files.includes('tests/registry.yaml'),
    'the scan must reach .yaml, not just .yml — the two are one language',
  );
  // 每条来源的自负其责，**逐条钉死成员**（不是「非空就行」——那正是原先那句空洞断言的形式）。
  assert.deepEqual(
    exclusiveTo(consumed, RUST_SOURCE_LABEL),
    ['schemas/config/upgrade-package.schema.json'],
    'the Rust-side include_str! must be the only source that finds upgrade-package',
  );
  assert.ok(
    exclusiveTo(consumed, AJV_SOURCE_LABEL).includes('schemas/tests/registry.schema.json') &&
      exclusiveTo(consumed, AJV_SOURCE_LABEL).includes('schemas/platform-matrix/platforms.schema.json'),
    'the Ajv validator list must be the only source for the registry and platform-matrix schemas',
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
  // 注入五：**来源自负其责**。把 Ajv 清单整条抽掉 ⇒ 两侧同时开火：它自己变成「零独有覆盖」，
  // 且那 5 份只有它能找到的 schema 立刻变成孤儿。
  const noAjv = new Map(
    [...consumed]
      .map(([schema, reasons]) => [schema, reasons.filter(reason => reason !== AJV_SOURCE_LABEL)])
      .filter(([, reasons]) => reasons.length > 0),
  );
  assert.ok(
    sourceErrors(noAjv, [AJV_SOURCE_LABEL, RUST_SOURCE_LABEL]).some(error => error.includes(AJV_SOURCE_LABEL)),
    'a source with no exclusive hit must be named',
  );
  assert.ok(
    coverageErrors(all, noAjv, NAMED_EXCEPTIONS).some(
      error => error.includes('schemas/tests/registry.schema.json') && error.includes('nobody reads'),
    ),
    'the schemas only the Ajv list finds must become orphans when it is dropped',
  );
  // 注入六：把 Rust 来源整条抽掉 ⇒ `upgrade-package` 变成孤儿（它只由 Rust 找到）。
  const noRust = new Map(
    [...consumed]
      .map(([schema, reasons]) => [schema, reasons.filter(reason => reason !== RUST_SOURCE_LABEL)])
      .filter(([, reasons]) => reasons.length > 0),
  );
  assert.ok(
    coverageErrors(all, noRust, NAMED_EXCEPTIONS).some(
      error => error.includes('schemas/config/upgrade-package.schema.json') && error.includes('nobody reads'),
    ),
    'dropping the Rust source must leave upgrade-package an orphan',
  );
  // 注入七：注释里的路径 MUST NOT 被算作消费——否则「删掉真正的加载点、只留一句注释」就能骗过守卫，
  // 而这正是本守卫唯一要抓的东西（没人读的契约）。同时确认代码里的路径仍然算数。
  const pathLike = /schemas\/[A-Za-z0-9_./-]+\.schema\.json/;
  assert.ok(
    !pathLike.test(stripCommentLines('// schemas/rag-golden/case-set.schema.json', 'slash')),
    'a path mentioned in a whole-line comment must not count as a consumer',
  );
  assert.ok(
    !pathLike.test(stripCommentLines(' * see schemas/rag-golden/case-set.schema.json', 'slash')),
    'a path mentioned in a block-comment continuation must not count either',
  );
  assert.ok(
    !pathLike.test(stripCommentLines('# schemas/rag-golden/case-set.schema.json', 'hash')),
    'a path mentioned in a workflow comment must not count either',
  );
  assert.ok(
    !pathLike.test(stripCommentLines('# schemas/rag-golden/case-set.schema.json', CODE_LANGUAGES.get('.yaml'))),
    'the .yaml extension must resolve to the hash comment language through the table',
  );
  assert.ok(
    pathLike.test(stripCommentLines("await readFile('schemas/catalog/catalog.schema.json', 'utf8')", 'slash')),
    'a path used in code must still count as a consumer',
  );
});

test('the scan set is structural, paid for, and closed over the extensions it reads', async () => {
  // ① 结构性规则 ＋ 具名排除是**无害**的：跳 dot 段与不跳 dot 段的消费集合 MUST 相同。
  //    这就是排除理由的可执行形式——若某个 dot 目录里藏着真消费，两侧立刻不等。
  const withoutDot = await codeFiles('.', CODE_EXTENSIONS, true);
  const withDot = await codeFiles('.', CODE_EXTENSIONS, false);
  assert.deepEqual([...(await consumptionOf(withDot)).consumed], [...(await consumptionOf(withoutDot)).consumed]);
  // 路径 MUST 是 POSIX 相对路径：`./` 前缀会让扫描集与索引里的路径**不可比**，
  // 于是「证据来源在不在仓库里」那条断言会对根目录文件静默假阴性（F6）。
  assert.ok(
    withoutDot.every(file => !file.startsWith('./')),
    'the scan set must not carry a "./" prefix — it is compared against the index, which never has one',
  );
  assert.ok(
    withoutDot.some(file => !file.includes('/')),
    'a root-level file must be reachable without a "./" prefix',
  );
  // 非空洞：这个「相等」必须对输入**敏感**。加一个含 `include_str!` 的文件，集合必须变化
  // （探针写进 `dist/`，它本身是被排除的目录，不会污染别的断言）。
  const probe = 'dist/.w75-scan-probe.rs';
  await mkdir('dist', { recursive: true });
  await writeFile(probe, 'const S: &str = include_str!("../schemas/config/w75-probe.schema.json");');
  try {
    const perturbed = (await consumptionOf([...withoutDot, probe])).consumed;
    assert.ok(
      perturbed.has('schemas/config/w75-probe.schema.json'),
      'a file that includes a schema must register as a consumer',
    );
    assert.notDeepEqual([...perturbed], [...(await consumptionOf(withoutDot)).consumed]);
  } finally {
    await rm(probe, { force: true });
  }
  // ② 具名排除 MUST 付得起理由：`.gitignore` 声明「这个目录不是仓库的一部分」。
  const ignoreLines = (await readFile('.gitignore', 'utf8')).split('\n');
  assert.deepEqual(exclusionErrors(EXCLUDED_DIRS, ignoreLines), []);
  // 注入：加一条不在 `.gitignore` 里的排除 ⇒ 立刻被指名（否则「排除」退化成豁免）。
  assert.ok(
    exclusionErrors([...EXCLUDED_DIRS, 'vendor'], ignoreLines).some(error => error.includes('vendor')),
    'an exclusion that .gitignore does not account for must fail',
  );
  // ③ 语言闭包：扫描集**由语言表派生** ⇒「新增扩展名」与「声明它的注释语言」是同一件事。
  for (const extension of CODE_EXTENSIONS) {
    assert.ok(CODE_LANGUAGES.has(extension), `${extension} is scanned without a declared comment language`);
  }
  assert.equal(CODE_LANGUAGES.get('.yml'), CODE_LANGUAGES.get('.yaml'), '.yml and .yaml are one language');
  // 注入：把 `.yaml` 从语言表里删掉 ⇒ 扫描集不再收录 `tests/registry.yaml`——
  // 原实现里唯一暴露扩展名缺口的地方，正是这份文件。
  const withoutYaml = CODE_EXTENSIONS.filter(extension => extension !== '.yaml');
  assert.ok(
    !(await codeFiles('.', withoutYaml, true)).includes('tests/registry.yaml'),
    'dropping .yaml from the language table must drop tests/registry.yaml from the scan',
  );
});

test('the generator and the validator read one list of schemas', async () => {
  const validated = Object.values(AJV_SOURCES);
  const generated = (await readdir('packages/contracts-ts/src/generated/config'))
    .filter(name => name.endsWith('.ts'))
    .map(name => name.slice(0, -3))
    .sort();
  assert.deepEqual(generatorErrors(TS_GENERATED_SOURCES, validated, generated), []);
  // 非空洞：清单与生成目录都必须真的非空，否则上面那句会退化成「两边都是空的」。
  assert.ok(TS_GENERATED_SOURCES.length > 0, 'the generator list must not be empty');
  assert.ok(generated.length > 0, 'the generated directory must not be empty');
  // 防回退：生成器 MUST 从这份清单派生，而不是自己再抄一份。
  const generator = await readFile('scripts/contracts/generate.mjs', 'utf8');
  assert.ok(generator.includes('TS_GENERATED_SOURCES'), 'the generator must derive its list from validation.mjs');
  assert.ok(!/const\s+jsonSources\s*=/.test(generator), 'the generator must not keep a second copy of the list');
  // 注入一：清单里有、生成目录里没有 ⇒ 双向对齐开火。
  assert.ok(
    generatorErrors([...TS_GENERATED_SOURCES, 'config/w74-probe'], validated, generated).some(
      error => error.includes('declares w74-probe') && error.includes('no w74-probe.ts'),
    ),
    'a schema in the list with no generated file must fail',
  );
  // 注入二：生成目录里有、清单里没有 ⇒ 另一个方向开火。
  assert.ok(
    generatorErrors(TS_GENERATED_SOURCES, validated, [...generated, 'w74-probe']).some(
      error => error.includes('w74-probe.ts') && error.includes('does not declare it'),
    ),
    'a generated file the list does not declare must fail',
  );
  // 注入三：某份 schema 只生成不校验 ⇒「生成面 ⊆ 校验面」开火。
  assert.ok(
    generatorErrors([...TS_GENERATED_SOURCES, 'config/w74-probe'], validated, [...generated, 'w74-probe']).some(
      error => error.includes('config/w74-probe') && error.includes('not in the validator list'),
    ),
    'a schema generated but never validated must fail',
  );
  // 注入四：两条 schema 撞 basename ⇒ 生成物会互相覆盖，必须失败。
  assert.ok(
    generatorErrors([...TS_GENERATED_SOURCES, 'other/runtime'], validated, generated).some(error =>
      error.includes('same generated file'),
    ),
    'two schemas writing the same generated file must fail',
  );
});

/// ── 仓库文件集的读取器（`.git/index`，DIRC）────────────────────────────────────
///
/// 这个读取器**替掉了** W74 的「结构性收敛 + 登记残余风险」：它让「判据的输入 == 仓库」
/// 从一句理想变成一条**可判定的前提**。所以它自己 MUST 被钉死——包括**失败也说得清是哪一类**：
/// 「索引读不了」与「索引是空的」是两件完全不同的事，后者会让判据对一切 schema 恒真。
function encodeVarint(value) {
  const bytes = [value & 0x7f];
  let rest = value >> 7;
  while (rest > 0) {
    rest -= 1;
    bytes.unshift(0x80 | (rest & 0x7f));
    rest >>= 7;
  }
  return Buffer.from(bytes);
}

function v2Entry(name, { extended = false } = {}) {
  const header = Buffer.alloc(62);
  header.writeUInt16BE(extended ? 0x4000 : 0, 60);
  const tail = Buffer.concat([extended ? Buffer.alloc(2) : Buffer.alloc(0), Buffer.from(`${name}\0`, 'utf8')]);
  const consumed = 62 + tail.length;
  return Buffer.concat([header, tail, Buffer.alloc(Math.ceil(consumed / 8) * 8 - consumed)]);
}

function v4Entry(name, previous) {
  let common = 0;
  while (common < previous.length && common < name.length && previous[common] === name[common]) common += 1;
  return Buffer.concat([Buffer.alloc(62), encodeVarint(common), Buffer.from(`${name.slice(common)}\0`, 'utf8')]);
}

function indexBuffer(version, entries) {
  const head = Buffer.alloc(12);
  head.write('DIRC', 0, 'ascii');
  head.writeUInt32BE(version, 4);
  head.writeUInt32BE(entries.length, 8);
  return Buffer.concat([head, ...entries]);
}

test('the tracked-path reader parses DIRC v2/v3/v4 and names the way it failed', () => {
  // v2：名字长度不是 8 的倍数时也要正确走过补齐（短名与长名混排最容易暴露补齐全错）。
  const mixed = indexBuffer(2, [
    v2Entry('a'),
    v2Entry('packages/nai-contracts/src/validation.rs'),
    v2Entry('b'),
  ]);
  assert.deepEqual(parseIndex(mixed), {
    ok: true,
    version: 2,
    paths: ['a', 'packages/nai-contracts/src/validation.rs', 'b'],
  });
  // v3：扩展 flags 置位 ⇒ 条目多 2 字节，仍按 8 字节对齐。
  assert.deepEqual(parseIndex(indexBuffer(3, [v2Entry('a.rs', { extended: true }), v2Entry('bb.rs')])), {
    ok: true,
    version: 3,
    paths: ['a.rs', 'bb.rs'],
  });
  // v4：名字前缀压缩，且条目不补齐。
  assert.deepEqual(
    parseIndex(
      indexBuffer(4, [
        v4Entry('apps/engine/src/lib.rs', ''),
        v4Entry('apps/engine/src/main.rs', 'apps/engine/src/lib.rs'),
        v4Entry('xtask/src/lib.rs', 'apps/engine/src/main.rs'),
        v4Entry('xtask/src/main.rs', 'xtask/src/lib.rs'),
      ]),
    ),
    {
      ok: true,
      version: 4,
      paths: ['apps/engine/src/lib.rs', 'apps/engine/src/main.rs', 'xtask/src/lib.rs', 'xtask/src/main.rs'],
    },
  );
  // 四类失败各自可辨——「读不了」MUST NOT 与「是空的」混为一谈。
  assert.equal(parseIndex(Buffer.concat([Buffer.from('NOPE'), Buffer.alloc(8)])).reason, 'bad-magic');
  assert.equal(parseIndex(indexBuffer(5, [])).reason, 'unsupported-version');
  assert.equal(parseIndex(indexBuffer(2, [v2Entry('a.rs')]).subarray(0, 40)).reason, 'truncated');
  assert.equal(parseIndex(indexBuffer(2, [v2Entry('a.rs')]).subarray(0, 4)).reason, 'truncated');
  // 名字里的首字节是控制字符 ⇒ 那是「把别的东西当成了路径」（例如按 SHA-1 的 20 字节去读
  // 一个 SHA-256 的索引）⇒ MUST 报告为 corrupt，而不是把一个假的路径集当成真的。
  const corrupted = indexBuffer(2, [v2Entry('a'.repeat(40))]);
  corrupted.writeUInt8(1, 74); // 12 字节头 + 62 字节条目头 = 名字首字节
  assert.equal(parseIndex(corrupted).reason, 'corrupt');
  // 空索引是合法的（空仓库）：返回空集，而不是失败。
  assert.deepEqual(parseIndex(indexBuffer(2, [])), { ok: true, version: 2, paths: [] });
});

test('the tracked-path reader agrees with the repository it is asked about', async () => {
  const repository = await readTrackedPaths();
  assert.ok(repository.ok, repository.ok ? '' : describeFailure(repository));
  assert.ok([2, 3, 4].includes(repository.version), `unexpected DIRC version ${repository.version}`);
  // 真实索引 MUST 含这些具名文件——否则「解析成功」可能只是解出了一个空集或别的东西。
  for (const known of ['package.json', 'tests/registry.yaml', 'schemas/tests/registry.schema.json', 'xtask/src/lib.rs']) {
    assert.ok(repository.paths.has(known), `${known} must be in the tracked-path set`);
  }
  // 被忽略的目录 MUST NOT 在索引里（它们不是仓库内容）。
  assert.ok(![...repository.paths].some(file => file.startsWith('dist/')), 'ignored directories are not tracked');
  assert.ok(repository.paths.size > 100, `the tracked-path set looks too small: ${repository.paths.size}`);
});

/// ── 配置面的**键级**消费状态（W90 / ADR 0068；W91 / ADR 0069 扩面）────────────
///
/// W73 的守卫管的是「**文件**级」：每份 schema 要么被执行体消费、要么具名例外。本组断言把
/// 同一件事下推**一层**：`schemas/config/` 下**每份** schema 的每个受判 `required` 键
/// MUST 落在三类的**恰一个**里——[A] 有派生出来的读取点、[B] 无读取点但其值被 `const` 钉死、
/// [C] 无读取点也未被钉死但**逐键在册并附成因**。
///
/// 母题来自 W89 的 F1：`session_defaults` 里 `allow_auto_without_platform_grant` 有读取点、
/// `viewer_memory_enabled` 没有——「**同层字段一个有主、一个无主**」。本判据把那条对照
/// 一般化到整个配置面。⇒「配置里有这个键」MUST NOT 被当作「运行时看得见它的值」。
///
/// **W91 把受判面从一份 schema 扩到 `schemas/config/` 全部**（6 份 / 96 键）。裸键名形状
/// 一旦跨对象就污染（`schema_version`/`mode`/`currency` 三键在不同 schema 里重名，实测
/// `["path"]` 命中 `crates/nai-plugin-sdk` 的是**另一个对象**的 path）⇒ 判定单元改成
/// `(schema, keyPath)`，且读取点 MUST 落在**归因文件集**内（`include_str!` 该 schema 的文件
/// ∪ 具名外部读者）。`runtime` 的 37 键在扩面后 MUST 逐项不变（10 / 4 / 23）——旧口径不许被顺手改写。

/// 扫描面：非测试、非生成、非判据自身的代码文本，且**按语言去掉整行注释**。
/// 去注释这一步是判据的一部分：注释里 `// value["viewer_memory_enabled"]` 一句话就能
/// 凭空造出一个读者，而「没人读的配置键」正是本判据唯一要抓的东西（W73 的第二次适用）。
async function configSources(files) {
  const out = [];
  for (const file of files) {
    if (inScan(file, SCAN_EXCLUSIONS)) continue;
    out.push({ file, text: stripCommentLines(await readFile(file, 'utf8'), CODE_LANGUAGES.get(path.extname(file))) });
  }
  return out;
}

test('every required config key is read, pinned, or attributed', async () => {
  const schemas = new Map();
  for (const schemaPath of CONFIG_SCHEMAS) schemas.set(schemaPath, await readJson(schemaPath));
  const files = await codeFiles();
  const sources = await configSources(files);
  const consumers = consumersOf(sources, schemas);
  const scanned = readSites(schemas, sources, consumers, EXTERNAL_READERS);
  // 判据的输入 MUST 等于它断言的对象：要断言「**仓库**里没有读者」，证据文件就必须在仓库里。
  const repository = await readTrackedPaths();
  assert.ok(repository.ok, repository.ok ? '' : describeFailure(repository));
  assert.deepEqual(partitionErrors(schemas, scanned, WIRED, UNWIRED), []);
  assert.deepEqual(evidenceErrors(WIRED, sources, repository.paths), []);
  assert.deepEqual(causeErrors(UNWIRED, UNWIRED_CAUSES), []);
  assert.deepEqual(attributionErrors(schemas, EXTERNAL_READERS, consumers, sources), []);
  assert.deepEqual(scanExclusionErrors(SCAN_EXCLUSIONS, files), []);

  // 非空洞①：**受判面 MUST 派生自磁盘** —— 受判的是 `schemas/config/` 下**全部** schema，
  // MUST NOT 是一份会过期的手写清单（新增一份 config schema 却没纳入 ⇒ 立刻红）。
  const onDisk = (await schemasOnDisk('schemas/config')).sort();
  assert.deepEqual([...schemas.keys()].sort(), onDisk, 'the config schemas under judgement must be exactly the ones on disk');
  assert.ok(onDisk.length > 1, `expected more than one config schema on disk, found ${onDisk.length}`);

  // 非空洞②：三类都非空 —— 任一类为空，对它的断言就在空集上空转（「非空洞」≠「数量 > 0」，
  // 所以下面还有逐类的**具体**断言：读取点必须落在具名的文件上、在册键必须指向真实成因）。
  const pinned = [...schemas].flatMap(([schemaPath, schema]) =>
    guardedKeys(schema)
      .filter(key => !scanned.has(keyLabel(schemaPath, key.path)) && key.node.const !== undefined)
      .map(key => keyLabel(schemaPath, key.path)),
  );
  const guardedTotal = [...schemas.values()].reduce((total, schema) => total + guardedKeys(schema).length, 0);
  assert.ok(guardedTotal > 0, 'the guard must have keys to judge');
  assert.ok(sources.length > 100, `the scan set looks too small: ${sources.length}`);
  for (const [label, size] of [
    ['read', scanned.size],
    ['pinned', pinned.length],
    ['registered', UNWIRED.size],
  ]) {
    assert.ok(size > 0, `the ${label} class is empty; an assertion over an empty class cannot fail`);
  }
  // 非空洞③：**三种读取形状都要真的出现** —— 少一种，那一族键就会整体掉进「无读者」，
  // 而判据仍然全绿（「只覆盖一种写法」时发生的就是这件事）。
  const shapesSeen = new Set();
  for (const shapes of scanned.values()) for (const shape of shapes.keys()) shapesSeen.add(shape);
  assert.deepEqual([...shapesSeen].sort(), ['helper', 'index', 'serde'], 'all three read shapes must actually occur');
  // 非空洞④：归因集必须真的被派生。除 `binding-plan`（只有 Ajv 校验、无运行时读取）外，
  // 每份 config schema 都 MUST 有 `include_str!` 消费者。
  const withoutConsumers = [...schemas.keys()].filter(schemaPath => (consumers.get(schemaPath) ?? new Set()).size === 0);
  assert.deepEqual(withoutConsumers, ['schemas/config/binding-plan.schema.json'], 'binding-plan is the only config schema expected to have no include_str! consumer');

  // **回归**：`runtime` 的 37 个受判键在**扩面之后** MUST 逐项不变（W90 = 10 / 4 / 23）。
  // 这一条是「扩面没有顺手改动老口径」的证据；它不绿，说明扩面把既有判定改了。
  const runtimePath = 'schemas/config/runtime.schema.json';
  const runtimeKeys = guardedKeys(schemas.get(runtimePath));
  const count = predicate => runtimeKeys.filter(predicate).length;
  assert.deepEqual(
    [
      runtimeKeys.length,
      count(key => scanned.has(keyLabel(runtimePath, key.path))),
      count(key => !scanned.has(keyLabel(runtimePath, key.path)) && key.node.const !== undefined),
      count(key => UNWIRED.has(keyLabel(runtimePath, key.path))),
    ],
    [37, 10, 4, 23],
    'the runtime schema must keep the exact classification it had before the surface was widened',
  );
  // 非空洞⑤：定义面凭**语言闭包**就在扫描集之外（`.json` 不在语言表里）——所以排除表里没有它。
  // 一旦有人把 `.json` 加进 `CODE_LANGUAGES`，这条会开火，逼他在 `SCAN_EXCLUSIONS` 里补一条有理由的排除。
  assert.ok(
    !CODE_EXTENSIONS.includes('.json'),
    'JSON must stay outside the scan set: the schema is the asserted object, it MUST NOT also be its own evidence',
  );
  for (const schemaPath of CONFIG_SCHEMAS) assert.ok(!files.includes(schemaPath), 'a schema file must not be a scanned source');
  // 非空洞⑥：两张台账都**承重** —— 清空任何一张都必须产生错误，否则它只是注释。
  assert.ok(
    partitionErrors(schemas, scanned, new Map(), UNWIRED).length > 0,
    'emptying the wired ledger must be reported — otherwise the ledger proves nothing',
  );
  assert.ok(
    partitionErrors(schemas, scanned, WIRED, new Map()).length > 0,
    'emptying the registration ledger must be reported — otherwise the ledger proves nothing',
  );

  // 注入 F1：真跑那一行——把**真实源码文本**里的一条读取点挖掉（不是造桩）。
  // 挖的是 `store.rs` 的 `["max_buffer_ms"]` ⇒ 该键从 [A] 掉出来，而它既未被钉死也未在册。
  const store = 'crates/nai-storage/src/store.rs';
  const gutted = sources.map(source =>
    source.file === store ? { ...source, text: source.text.replace('["max_buffer_ms"]', '["max_buffer_ms_"]') } : source,
  );
  assert.notEqual(gutted.find(source => source.file === store).text, sources.find(source => source.file === store).text, 'the injection must actually change the text');
  const guttedScanned = readSites(schemas, gutted, consumers, EXTERNAL_READERS);
  const maxBuffer = keyLabel(runtimePath, 'media.max_buffer_ms');
  assert.ok(!guttedScanned.has(maxBuffer), 'the injected read site must be gone');
  assert.ok(
    partitionErrors(schemas, guttedScanned, WIRED, UNWIRED).some(error => error.startsWith(maxBuffer)),
    'a key whose only reader was removed must be reported',
  );
  assert.ok(
    evidenceErrors(WIRED, gutted, repository.paths).some(error => error.includes('media.max_buffer_ms')),
    'the ledger must also be caught naming a reader that no longer reads',
  );

  // 注入 F2：把 [A] 的证据文件指到一个**不含**该读取形状的文件 ⇒ 自清理开火（读取点搬家/被删）。
  const runtimeEdition = keyLabel(runtimePath, 'edition');
  assert.ok(
    evidenceErrors(new Map([[runtimeEdition, { shape: 'index', evidence: 'crates/nai-kernel/src/session.rs' }]]), sources, repository.paths).some(error =>
      error.includes(runtimeEdition),
    ),
    'a named reader that does not read the key must be reported',
  );
  // 注入 F3：在册遮蔽真消费者 ⇒ 报「这是让判据自我满足的那条路」。
  assert.ok(
    partitionErrors(schemas, scanned, WIRED, new Map([...UNWIRED, [maxBuffer, 'surface-absent']])).some(error =>
      error.includes(maxBuffer) && error.includes('shadows a real reader'),
    ),
    'registering a key that does have a reader must be reported',
  );
  // 注入 F4：陈旧在册（schema 里没有这个键）⇒ 报错。台账 MUST 自清理。
  const stale = keyLabel(runtimePath, 'cloud.gone');
  assert.ok(
    partitionErrors(schemas, scanned, WIRED, new Map([...UNWIRED, [stale, 'surface-absent']])).some(error => error.includes(stale)),
    'a stale registration must be reported',
  );
  // 注入 F5：缺成因 / 成因不在成因表里 ⇒ 两条都报（理由就是台账的全部价值）。
  const bindings = keyLabel(runtimePath, 'bindings');
  assert.ok(
    partitionErrors(schemas, scanned, WIRED, new Map([...UNWIRED, [bindings, '   ']])).some(error => error.includes('without a cause')),
    'a registration without a cause must fail',
  );
  assert.ok(
    partitionErrors(schemas, scanned, WIRED, new Map([...UNWIRED, [bindings, 'unknown-cause']])).some(error => error.includes('not in the cause table')),
    'an unknown cause is a label, not a reason',
  );
  // 注入 F6：把一个**被钉死**的键登记成死键 ⇒ 报「值不可能不同，不需要读者」。
  const pinnedLabel = keyLabel(runtimePath, 'schema_version');
  assert.ok(
    partitionErrors(schemas, scanned, WIRED, new Map([...UNWIRED, [pinnedLabel, 'surface-absent']])).some(error => error.includes('pins its value')),
    'registering a pinned key as unwired must be reported',
  );
  // 注入 F7：新增一个 required 键、不接线也不登记 ⇒ 立刻被指名（这条就是「不许新增死键」）。
  const grown = new Map(schemas);
  const grownRuntime = structuredClone(grown.get(runtimePath));
  grownRuntime.properties.cloud.required.push('relay_region');
  grownRuntime.properties.cloud.properties.relay_region = { type: 'string' };
  grown.set(runtimePath, grownRuntime);
  assert.ok(
    partitionErrors(grown, readSites(grown, sources, consumers, EXTERNAL_READERS), WIRED, UNWIRED).some(error => error.includes('cloud.relay_region')),
    'a new required key that nobody wired must be named',
  );
  // 注入 F7b：**新增一份 config schema** 却把它留在受判面之外 ⇒ 立刻红（受判面派生自磁盘）。
  assert.ok(
    ![...schemas.keys()].includes('schemas/config/nine.schema.json'),
    'the injected schema must not already be under judgement',
  );
  assert.ok(
    JSON.stringify([...schemas.keys()].sort()) !== JSON.stringify([...onDisk, 'schemas/config/nine.schema.json'].sort()),
    'a config schema that never enters the judgement must not pass the derived-surface assertion',
  );
  // 注入 F8：**取消钉死**（`const` → `boolean`）⇒ 它从 [B] 掉到「未登记」⇒ 证明 [B] 是**派生**的，
  // 不是一份手写清单（若 [B] 是硬写的，这一条不会开火）。
  const unpinned = new Map(schemas);
  const unpinnedRuntime = structuredClone(unpinned.get(runtimePath));
  unpinnedRuntime.properties.cache.properties.invalidate_on_fact_change = { type: 'boolean' };
  unpinned.set(runtimePath, unpinnedRuntime);
  assert.ok(
    partitionErrors(unpinned, readSites(unpinned, sources, consumers, EXTERNAL_READERS), WIRED, UNWIRED).some(error => error.includes('cache.invalidate_on_fact_change')),
    'unpinning a key must drop it out of [B] and be reported',
  );
  // 注入 F9：容器传递的**健全性**（这一条决定了「受判键」这个集合是否真的封闭）——
  // 有 `required` 子键的容器不受判（`cloud`），无 `required` 子键的 object 键必须自己受判（`bindings`）。
  assert.ok(!runtimeKeys.some(entry => entry.path === 'cloud'), 'a container is covered by its required children');
  assert.ok(runtimeKeys.some(entry => entry.path === 'bindings'), 'an object key with no required children cannot be covered — it must be guarded');
  const escaped = new Map(schemas);
  const escapedRuntime = structuredClone(escaped.get(runtimePath));
  escapedRuntime.properties.runtime.required.push('empty_bag');
  escapedRuntime.properties.runtime.properties.empty_bag = { type: 'object', additionalProperties: { type: 'string' } };
  escaped.set(runtimePath, escapedRuntime);
  assert.ok(
    partitionErrors(escaped, readSites(escaped, sources, consumers, EXTERNAL_READERS), WIRED, UNWIRED).some(error => error.includes('runtime.empty_bag')),
    'a required object with no required children must not escape the guard by looking like a container',
  );

  // 注入 F10：**index 形状的两个方向**。顶层形状要求 `[` 前是表达式字符 ⇒ Rust 的**数组字面量**
  // MUST NOT 算（实测 `xtask/src/deps.rs` 的 `&["knowledge"]` 骗过过一版宽松正则），真索引 MUST 算。
  assert.ok(indexShape('schema_version').test('value["schema_version"]'), 'a real top-level index must be recognised');
  assert.ok(!indexShape('schema_version').test('&["schema_version"]'), 'a Rust array literal must not count as a read site');
  // 注入 F11：**index 形状 MUST 精确到父键锚**。实测 `snapshot.rs` 的 `entry["path"]` 是 artifacts
  // 数组元素的 path，MUST NOT 被算作 `database.path` 的读者；而**中间变量**写法 MUST 仍被算作
  // `media.lease_renew_ms` 的读者（只用完整路径会漏掉它 —— 假阴性）。
  assert.ok(indexShape('database.path').test('manifest["database"]["path"]'), 'a parent-anchored index must be recognised');
  assert.ok(!indexShape('database.path').test('entry["path"]'), 'an index under a different object must not count');
  assert.ok(indexShape('media.lease_renew_ms').test('media["lease_renew_ms"].as_u64()'), 'the intermediate-variable form must still count');
  // 注入 F12：**serde 形状的判定域**。`plugins/avatar-2d/src/lib.rs` 里 `Manifest` 与 `Config`
  // **同在**该文件、且都被反序列化 ⇒ 「同文件」不承重；拦下 Manifest 字段的是
  // 「字段名 ∩ 受判键名」。逐键钉死 avatar-2d 的 [A] = `Config` 的 7 个字段，多一个少一个都红。
  const avatarPath = 'schemas/config/avatar-2d.schema.json';
  const avatarFile = 'plugins/avatar-2d/src/lib.rs';
  const avatarSerde = serdeFieldsByFile(sources).get(avatarFile) ?? new Set();
  assert.ok(avatarSerde.has('output_width'), 'the Manifest fields are in the decode set — otherwise this injection proves nothing');
  assert.deepEqual(
    guardedKeys(schemas.get(avatarPath))
      .filter(key => scanned.has(keyLabel(avatarPath, key.path)))
      .map(key => key.path)
      .sort(),
    ['attack_ms', 'full_open_rms', 'layers', 'max_buffer_samples', 'noise_gate', 'release_ms', 'sample_rate'],
    'only the Config fields that are also guarded keys may be attributed to this schema',
  );
  // 注入 F13：**helper 形状**。`manifest.rs` 的读法是 `amount(&value, "install_bytes")`，键名是
  // **字符串实参**，`["install_bytes"]` 在该文件里零命中。少这一态，`upgrade-package` 会被误判成
  // 「22 个键只有 7 个有读者」—— 那正是本轮要堵的假阴性。
  const manifestFile = 'crates/nai-upgrade-core/src/manifest.rs';
  const manifestHelpers = keyHelpersByFile(sources).get(manifestFile) ?? new Set();
  assert.ok(manifestHelpers.has('text') && manifestHelpers.has('amount'), 'the read-key helpers must be derived (amount calls text)');
  assert.ok(helperShape(manifestHelpers, 'install_bytes').test('install_bytes: amount(&value, "install_bytes")?,'), 'a helper call with the key as a string argument must count as a read');
  assert.ok(!helperShape(manifestHelpers, 'install_bytes').test('Err(error(ErrorCode::Internal, "install_bytes"))'), 'a call that is not a read-key helper must not count');
  assert.ok(
    !indexShape('install_bytes').test(sources.find(source => source.file === manifestFile).text),
    'the helper form is invisible to the index shape — that is exactly why the third shape exists',
  );
  // 注入 F14：**归因集外的裸名命中 MUST NOT 授权**。实测 `binding-plan#session_id` 的裸名在
  // `crates/nai-control-api` 命中，那是**另一个对象**的同名键；`binding-plan` 没有归因文件。
  const bindingSession = keyLabel('schemas/config/binding-plan.schema.json', 'session_id');
  assert.ok(!scanned.has(bindingSession), 'a hit outside the attribution set must not be a reader');
  assert.ok(
    indexShape('session_id').test(sources.find(source => source.file === 'crates/nai-control-api/src/lib.rs').text),
    'the out-of-attribution hit must really exist, or this injection proves nothing',
  );
  // 注入 F15：**归因集 MUST 自清理** —— 声明了外部读者却不再读 ⇒ 红。
  assert.ok(
    attributionErrors(
      schemas,
      new Map([['schemas/config/avatar-2d.schema.json', new Set(['crates/nai-kernel/src/session.rs'])]]),
      consumers,
      sources,
    ).some(error => error.includes('reads none of its keys')),
    'an external reader that stopped reading must be reported',
  );
  assert.ok(
    attributionErrors(schemas, new Map([['schemas/config/gone.schema.json', new Set(['x.rs'])]]), consumers, sources).some(error =>
      error.includes('not a config schema'),
    ),
    'an attribution for a schema that is not under judgement must be reported',
  );
  // 注入 F16：整行注释里的读取 MUST NOT 算（`configSources` 已按语言去注释）。
  assert.ok(
    !indexShape('http_enabled').test(stripCommentLines('// value["http_enabled"]', 'slash')),
    'a read mentioned in a comment must not count as a reader',
  );
  // 注入 F17：陈旧排除 ⇒ 排除不是豁免，一条匹配不到任何文件的排除必须被指名。
  assert.ok(
    scanExclusionErrors(new Map([...SCAN_EXCLUSIONS, ['vendor/', '一个不存在的目录']]), files).some(error => error.includes('vendor/')),
    'an exclusion that matches no file must be reported',
  );
});

