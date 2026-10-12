import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, readdir, writeFile, mkdir, rm } from 'node:fs/promises';
import path from 'node:path';
import { validators, readJson, validateSemantic, apiDocument, AJV_SOURCES, TS_GENERATED_SOURCES } from '../../scripts/contracts/validation.mjs';

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
    for (const match of text.matchAll(RUST_INCLUDE)) add(`schemas/${match[1]}`, RUST_SOURCE_LABEL);
  }
  return consumed;
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
  const consumed = await consumptionOf(files);
  assert.deepEqual(coverageErrors(all, consumed, NAMED_EXCEPTIONS), []);
  assert.deepEqual(sourceErrors(consumed, [AJV_SOURCE_LABEL, RUST_SOURCE_LABEL]), []);
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
  assert.deepEqual([...await consumptionOf(withDot)], [...await consumptionOf(withoutDot)]);
  // 非空洞：这个「相等」必须对输入**敏感**。加一个含 `include_str!` 的文件，集合必须变化
  // （探针写进 `dist/`，它本身是被排除的目录，不会污染别的断言）。
  const probe = 'dist/.w74-scan-probe.rs';
  await mkdir('dist', { recursive: true });
  await writeFile(probe, 'const S: &str = include_str!("../schemas/config/w74-probe.schema.json");');
  try {
    const perturbed = await consumptionOf([...withoutDot, probe]);
    assert.ok(
      perturbed.has('schemas/config/w74-probe.schema.json'),
      'a file that includes a schema must register as a consumer',
    );
    assert.notDeepEqual([...perturbed], [...await consumptionOf(withoutDot)]);
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
