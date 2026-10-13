/// 配置面的**键级消费状态**：`schemas/config/` 下每份 schema 的受判 `required` 键的**三分类闭世界**。
///
/// 「配置里有这个键」MUST NOT 被当作「运行时看得见它的值」。W89 的 F1 抓到的形状是
/// 「**同层字段一个有主、一个无主**」；W90 把那条对照一般化到 `runtime.schema.json` 的 37 个受判键；
/// 本模块把它扩到**整个 `schemas/config/`**（6 份 schema、96 个受判键）。
///
/// ## 为什么不能只是「扩面」
///
/// W90 的读取形状是**裸键名** `EXPR["<key>"]`，它**不带对象上下文**。96 个受判键里有 3 个裸名
/// 跨 schema 重名（`schema_version` ×5、`mode` ×3、`currency` ×2），且通用名词会**跨对象污染**
/// ——实测 `["sample_rate"]` 命中 `apps/render-worker`、`["session_id"]` 命中 `crates/nai-control-api`、
/// `["path"]`/`["sha256"]`/`["id"]` 命中 `crates/nai-plugin-sdk`。照裸名规则，这些键会**凭空获得读者**。
///
/// ⇒ 读取点 MUST 通过**归因文件集**绑定到 schema，且形状 MUST 覆盖**三态**（见 `readShapes`）。
///
/// ## 三类
///
/// - **[A] 有读取点**——由**派生**得到（三种形状任一命中），台账逐键具名写出**证据文件与形状**。
/// - **[B] 值被 schema 钉死**（`const`）——值不可能不同 ⇒ 不需要读者。这一类是**派生**的，不是台账。
/// - **[C] 既无读取点、也未被钉死**——改它的值**不改变任何行为**。这一族**逐键在册**并附成因。
///
/// 三类都非空、两两不相交、并集**恰等于**受判键集合。
///
/// ⚠️ **A 与 C 两侧都是双向的**：有读取点的键 MUST 具名在册（观测不能替人签名），在册的键 MUST
/// 真的没有读取点（签名不能遮住观测）。少了前者，台账对闭世界不承重——闭世界只要求「落在三类之一」，
/// 而 [A] 由扫描自动成立 ⇒ 清空台账判据照旧全绿（W90 的第一版实测如此）。

/// `schemas/config/` 下的全部 schema。受判面 = 这一整个目录，MUST NOT 只列一份。
export const CONFIG_SCHEMAS = [
  'schemas/config/avatar-2d.schema.json',
  'schemas/config/backup.schema.json',
  'schemas/config/binding-plan.schema.json',
  'schemas/config/provider.schema.json',
  'schemas/config/runtime.schema.json',
  'schemas/config/upgrade-package.schema.json',
];

/// 受判单元的标签：`<schema 路径>#<键路径>`。
///
/// **判定单元 MUST 是 `(schema, key)`，MUST NOT 是裸键名**：`schema_version` 在 5 份 schema 的
/// 顶层同形 ⇒ 「同名」MUST NOT 被当作「同一个键」。
export function keyLabel(schemaPath, keyPath) {
  return `${schemaPath}#${keyPath}`;
}

/// 递归枚举 `required` 键。返回的每一项都带 `container` 标记：
/// **自己有 `required` 子键的容器**不受判——JSON 里要读到 `config["media"]["lease_renew_ms"]`
/// 必须先读到 `config["media"]`，所以容器的消费**由其受判子键蕴含**。
///
/// 「容器」是**派生**的（`type === 'object'` 且 `required.length > 0`），MUST NOT 写成键名清单。
/// 一个 `required` 的 object 键若**没有** `required` 子键（如 `bindings`），它不能被任何子键蕴含
/// ⇒ 它**必须自己受判**。
export function requiredKeys(schema, prefix = '') {
  const out = [];
  for (const name of schema.required ?? []) {
    const node = schema.properties?.[name] ?? {};
    const path = prefix === '' ? name : `${prefix}.${name}`;
    const container = node.type === 'object' && Array.isArray(node.required) && node.required.length > 0;
    out.push({ path, name, node, container });
    if (container) out.push(...requiredKeys(node, path));
  }
  return out;
}

/// 受判键 = 全部 `required` 键减去不受判的容器。
export function guardedKeys(schema) {
  return requiredKeys(schema).filter(entry => !entry.container);
}

/// **形状一 `index`**：`EXPR["<name>"]`。
///
/// - **顶层键**（键路径无 `.`）：`[` 前**必须**是表达式字符（标识符 / `)` / `]`）。少了这一条，
///   Rust 的**数组字面量**会被误判：实测 `xtask/src/deps.rs:441` 的 `&["knowledge"]` 让一个
///   零读者的容器键凭空获得「读者」。同族的假阳性还有 `vec!["a"]`、`= ["a"]`、`(["a"])`。
/// - **嵌套键**：锚 MUST 是紧邻的 `["<parent>"]`（连续索引）**或** `<parent>`（标识符，即
///   `let media = &config["media"]; media["lease_renew_ms"]` 这种**中间变量**写法）。
///   `parent` = 键路径的**倒数第二段**（派生，MUST NOT 写死）。
///
/// ⚠️ 只用「完整路径」（`["media"]["lease_renew_ms"]`）会**漏掉中间变量写法**（实测
/// `crates/nai-runtime/src/composition.rs`），那是**假阴性**（危险方向）；只用裸名又会把
/// `entry["path"]` 算作 `database.path` 的读者（实测 `crates/nai-storage/src/snapshot.rs`，
/// 那是 artifacts 数组元素的 path）。父键锚是这两者之间**唯一**能同时避开两个方向的位置。
export function indexShape(keyPath) {
  const segments = String(keyPath).split('.');
  const name = segments[segments.length - 1];
  if (segments.length === 1) return new RegExp(`[A-Za-z0-9_)\\]]\\["${name}"\\]`);
  const parent = segments[segments.length - 2];
  return new RegExp(`(?:\\["${parent}"\\]|\\b${parent})\\s*\\["${name}"\\]`);
}

/// W90 的单段兼容入口（顶层键）。
export function readShape(name) {
  return indexShape(name);
}

/// **形状二 `serde`**：结构体解码。
///
/// 识别方式：文件里出现 `serde_json::from_value(...)` / `from_str` / `from_slice`，其目标类型
/// **`struct T` 定义在同一个文件**（`let x: T = ...` 或 `...::<T>` 两种写法）。
/// ⇒ `T` 的字段名 ∩ 该 schema 的受判键名，算有读取点。
///
/// ⚠️ **「同文件定义」这一条是必须的**：实测 `plugins/avatar-2d/src/lib.rs` 的 `Avatar::admit` 里
/// 既有 `let manifest: Manifest = ...`（插件 SDK 的 **Manifest**，定义在**另一个文件**）又有
/// `let config: Config = ...`（**同文件**的 `Config`）。不限定的话，插件清单的字段
/// （`kind` / `model_binding` / `output_width` …）会**凭空**成为配置面的读者。
export function serdeFieldsByFile(sources) {
  const decodePatterns = [
    /from_(?:value|str|slice)\s*::\s*<([A-Za-z_][A-Za-z0-9_]*)>/g,
    /let\s+[a-z_][A-Za-z0-9_]*\s*:\s*([A-Z][A-Za-z0-9_]*)\s*=\s*(?:serde_json::)?from_(?:value|str|slice)/g,
  ];
  const out = new Map();
  for (const { file, text } of sources) {
    const fields = new Set();
    for (const pattern of decodePatterns)
      for (const match of text.matchAll(pattern)) {
        const struct = text.match(new RegExp(`(?:pub\\s+)?struct ${match[1]}\\s*\\{([\\s\\S]*?)\\n\\}`));
        if (!struct) continue;
        for (const field of struct[1].matchAll(/^\s*(?:pub\s+)?([a-z_][a-z0-9_]*)\s*:/gm)) fields.add(field[1]);
      }
    if (fields.size) out.set(file, fields);
  }
  return out;
}

/// **形状三 `helper`**：经「**读键辅助函数**」读。
///
/// 实测 `crates/nai-upgrade-core/src/manifest.rs` 的读法是
/// `text(&value, "package_id")` / `strings(&value, "hosts")` / `amount(&value, "install_bytes")`
/// ——键名是**字符串实参**，`EXPR["<key>"]` 一个都扫不到（`["package_id"]` 在仓库里**零命中**）。
/// 不认这一态，`upgrade-package` 会被误判成「22 个键只有 7 个有读者」——**假阴性**。
///
/// 判定一个函数是「读键辅助函数」当且仅当它的**函数体**里出现**变量下标** `x[key]`
/// （`[` 后是标识符而非字面量），**或**它传递调用了已判定的辅助函数（实测 `amount` → `text`）。
/// ⇒ 「任何带字符串实参的调用」MUST NOT 被算作读键，否则 `error(Code, "message")` 会凭空造出读者。
export function keyHelpersByFile(sources) {
  const functions = new Map();
  for (const { file, text } of sources) {
    for (const match of text.matchAll(/\bfn\s+([a-z_][a-z0-9_]*)\s*(?:<[^>]*>)?\s*\(([^)]*)\)[^{;]*\{/g)) {
      const rest = text.slice(match.index + match[0].length);
      const end = rest.search(/\n\}/);
      functions.set(`${file}\u0000${match[1]}`, { file, body: rest.slice(0, end >= 0 ? end : rest.length) });
    }
  }
  const isReader = new Set();
  for (let pass = 0; pass < 8; pass += 1) {
    let grew = false;
    for (const [id, { file, body }] of functions) {
      if (isReader.has(id)) continue;
      const variableSubscript = /[A-Za-z_][A-Za-z0-9_]*\s*\[\s*[a-z_][A-Za-z0-9_]*\s*\]/.test(body);
      const callsReader = [...isReader].some(
        other => other.startsWith(`${file}\u0000`) && new RegExp(`\\b${other.split('\u0000')[1]}\\s*\\(`).test(body),
      );
      if (variableSubscript || callsReader) {
        isReader.add(id);
        grew = true;
      }
    }
    if (!grew) break;
  }
  const out = new Map();
  for (const id of isReader) {
    const [file, name] = id.split('\u0000');
    if (!out.has(file)) out.set(file, new Set());
    out.get(file).add(name);
  }
  return out;
}

/// `helper` 形状的匹配式：`HELPER(… "name" …)`。
export function helperShape(helpers, name) {
  return new RegExp(`\\b(?:${[...helpers].join('|')})\\s*\\([^;)]*"${name}"`);
}

/// 扫描面的**具名排除**。每条 MUST 附理由：排除不是豁免，是一条可核实的陈述。
///
/// 定义面（`schemas/**/*.schema.json`）**不在这张表里**：`.json` 不在 `CODE_LANGUAGES` 里，
/// 它凭**语言闭包**就在扫描集之外。测试侧另有一条断言钉住这件事。
///
/// ⚠️ 「测试面」MUST NOT 只按路径前缀排除：`crates/*/tests/*.rs`、`apps/*/tests/` 与 `src/tests.rs`
/// 都住在生产源码的旁边（实测第一版就漏了 `crates/nai-runtime/tests/composition.rs`）
/// ⇒ 匹配规则是**路径段**，见 `inScan`。
///
/// ⚠️ Rust 生成面（`crates/nai-contracts/src/generated/`）**不设排除**：实测它对 `index` 形状
/// **零命中**（prost 生成的是结构体字段，不是 `["…"]` 索引）⇒ 加一条不承重的排除违反
/// 「排除表每条都必须是承重的」。将来它真产生命中时再加。
const SCAN_EXCLUSIONS = new Map([
  [
    'tests/',
    '测试面可以任意读配置：把测试里的读取算作「生产消费」，守卫就会在**危险方向**上变瞎——' +
      '删掉唯一的生产读取点、只留一行测试，键照样判成「有主」。按**路径段**匹配，' +
      '所以 `crates/*/tests/`、`apps/*/tests/` 与 `tests/` 是同一件事。',
  ],
  [
    'tests.rs',
    '同上的另一种落点：`apps/output-keeper/src/tests.rs`、`crates/nai-kernel/src/registry/tests.rs` ' +
      '把测试放进 `src/` 里。扩展名是 `.rs`，靠前缀排不掉。',
  ],
  [
    '*.test.mjs',
    '同上的第三种落点：`tests/contracts/*.test.mjs` 之外的任何 `*.test.mjs` 也属测试面。',
  ],
  [
    'packages/contracts-ts/src/generated/',
    '生成面。生成物由 schema 派生（接口的字段名逐字来自 `required`/`properties`），' +
      '它出现键名只是**同一份定义的第二次书写**，不是消费者——把它算作读者，' +
      '任意一个 schema 键都会凭空获得一个「读者」，判据恒真。',
  ],
  [
    'scripts/contracts/config-consumption.mjs',
    '判据自身：两张台账**逐键写出键名**。若把判据的输入算作证据，判据就**自我满足**——' +
      '台账里每一条在册的键都会立刻获得一个读者，于是「在册」永远无法被证伪。',
  ],
]);

/// 扫描面的匹配规则：排除一条路径，当且仅当它是那条模式本身、以它为前缀、
/// **某个路径段**等于它，或（`*.ext` 形态）以该后缀结尾。
export function inScan(file, exclusions) {
  return [...exclusions.keys()].some(pattern => {
    const bare = pattern.replace(/\/$/, '');
    return (
      file === bare ||
      file.startsWith(`${bare}/`) ||
      file.split('/').includes(bare) ||
      (pattern.startsWith('*.') && file.endsWith(pattern.slice(1)))
    );
  });
}

/// **归因文件集**：schema → 读它的文件。
///
/// - `consumers`：Rust `include_str!` 该 schema 的文件（**派生**）。
/// - `externalReaders`：**具名外部读者**——经 `composition.config()` 拿 `Value`、因而不
///   `include_str!` 的文件。实测 `runtime` 有两条：`composition.rs`（读 `max_active_sessions`/
///   `lease_*`）与 `pipeline.rs`（读 `allow_auto_without_platform_grant`），
///   正是 W90 的 [A] 里**唯二**跨出 `store.rs` 的条目。
///
/// ⚠️ 归因集不是豁免：每条外部读者 MUST **自清理**（见 `attributionErrors`）。
export function attributionOf(schemaPath, consumers, externalReaders) {
  return new Set([...(consumers.get(schemaPath) ?? []), ...(externalReaders.get(schemaPath) ?? [])]);
}

/// 派生**读取点**：`<schema>#<键路径>` → `{ files:Set, shapes:Set }`。
///
/// `sources` 是 `{file, text}` 列表，**已经**排除了测试面、生成面与判据自身。本函数不做排除，
/// 排除是调用方的事：判据的输入 MUST 等于它断言的对象。
export function readSites(schemas, sources, consumers, externalReaders) {
  const helpersByFile = keyHelpersByFile(sources);
  const serdeByFile = serdeFieldsByFile(sources);
  const sites = new Map();
  for (const [schemaPath, schema] of schemas) {
    const attribution = attributionOf(schemaPath, consumers, externalReaders);
    const serde = new Set();
    for (const file of attribution) for (const field of serdeByFile.get(file) ?? []) serde.add(field);
    for (const key of guardedKeys(schema)) {
      const label = keyLabel(schemaPath, key.path);
      const index = indexShape(key.path);
      const shapes = new Map();
      for (const { file, text } of sources) {
        if (!attribution.has(file)) continue;
        if (index.test(text)) shapes.set('index', new Set([...(shapes.get('index') ?? []), file]));
        else {
          const helpers = helpersByFile.get(file);
          if (helpers && helperShape(helpers, key.name).test(text)) {
            shapes.set('helper', new Set([...(shapes.get('helper') ?? []), file]));
          }
        }
      }
      if (serde.has(key.name)) shapes.set('serde', new Set([...attribution].filter(f => serdeByFile.get(f)?.has(key.name))));
      if (shapes.size) sites.set(label, shapes);
    }
  }
  return sites;
}

/// **[A] 有读取点的键 → 形状、证据文件与说明**（逐键具名，手写）。
///
/// `evidence` 是**一个**文件：判据会断言它真的含有该形状的读取点（读取点消失 ⇒ 立刻红）。
/// `note` 说明**读它做什么**——只写「有人读」不足以让人判断这条接线是否成立。
export const WIRED = new Map([
  // avatar-2d：整个配置走结构体解码（`Config`，定义在同文件 `plugins/avatar-2d/src/lib.rs`）。
  ['schemas/config/avatar-2d.schema.json#layers', { shape: 'serde', evidence: 'plugins/avatar-2d/src/lib.rs', note: '图层清单的解码与许可校验' }],
  ['schemas/config/avatar-2d.schema.json#sample_rate', { shape: 'serde', evidence: 'plugins/avatar-2d/src/lib.rs', note: '音频采样率进入 PlaybackSpec' }],
  ['schemas/config/avatar-2d.schema.json#max_buffer_samples', { shape: 'serde', evidence: 'plugins/avatar-2d/src/lib.rs', note: '缓冲上限参与尺寸/容量校验' }],
  ['schemas/config/avatar-2d.schema.json#noise_gate', { shape: 'serde', evidence: 'plugins/avatar-2d/src/lib.rs', note: '与 full_open_rms 一起断言噪声门区间（noise_gate < full_open_rms）' }],
  ['schemas/config/avatar-2d.schema.json#full_open_rms', { shape: 'serde', evidence: 'plugins/avatar-2d/src/lib.rs', note: '同上：区间语义校验的另一端' }],
  ['schemas/config/avatar-2d.schema.json#attack_ms', { shape: 'serde', evidence: 'plugins/avatar-2d/src/lib.rs', note: '包络起振时长进入渲染参数' }],
  ['schemas/config/avatar-2d.schema.json#release_ms', { shape: 'serde', evidence: 'plugins/avatar-2d/src/lib.rs', note: '包络释放时长进入渲染参数' }],
  // backup：写入/恢复两侧读清单。
  ['schemas/config/backup.schema.json#runtime_version', { shape: 'index', evidence: 'crates/nai-storage/src/snapshot.rs', note: '校验清单的运行时版本 == 当前包版本（否则拒绝）' }],
  ['schemas/config/backup.schema.json#contract_catalog_sha256', { shape: 'index', evidence: 'crates/nai-storage/src/snapshot.rs', note: '校验契约目录哈希，防跨版本恢复' }],
  ['schemas/config/backup.schema.json#database.sha256', { shape: 'index', evidence: 'crates/nai-storage/src/snapshot.rs', note: '恢复前比对数据库哈希' }],
  ['schemas/config/backup.schema.json#database.bytes', { shape: 'index', evidence: 'crates/nai-storage/src/snapshot.rs', note: '恢复前比对数据库字节数' }],
  ['schemas/config/backup.schema.json#artifacts', { shape: 'index', evidence: 'crates/nai-storage/src/snapshot.rs', note: '恢复时比对受管工件清单' }],
  // provider：store 在写入前读端点与身份做校验。
  ['schemas/config/provider.schema.json#id', { shape: 'index', evidence: 'crates/nai-storage/src/store.rs', note: '标识参与 provider 记录的落库' }],
  ['schemas/config/provider.schema.json#protocol', { shape: 'index', evidence: 'crates/nai-storage/src/store.rs', note: '协议决定端点校验分支' }],
  ['schemas/config/provider.schema.json#endpoint', { shape: 'index', evidence: 'crates/nai-storage/src/store.rs', note: '解析 host/authority 做端点形状校验' }],
  ['schemas/config/provider.schema.json#model', { shape: 'index', evidence: 'crates/nai-storage/src/store.rs', note: '模型标识落库' }],
  ['schemas/config/provider.schema.json#credential_ref', { shape: 'index', evidence: 'crates/nai-storage/src/store.rs', note: '凭据**引用**（非明文）落库前的校验' }],
  ['schemas/config/provider.schema.json#price_revision', { shape: 'index', evidence: 'crates/nai-storage/src/store.rs', note: '价格版本落库，绑定计费口径' }],
  // runtime：沿用 W90 的十条（扩面后逐项不变，见报告 §回归）。
  ['schemas/config/runtime.schema.json#edition', { shape: 'index', evidence: 'crates/nai-storage/src/store.rs', note: '写库时把 profile 的 edition 落进 runtime_profiles.edition' }],
  ['schemas/config/runtime.schema.json#deployment', { shape: 'index', evidence: 'crates/nai-storage/src/store.rs', note: '同上：deployment 随 profile 落库' }],
  ['schemas/config/runtime.schema.json#workspace_id', { shape: 'index', evidence: 'crates/nai-storage/src/store.rs', note: '写库前断言配置声明的 workspace 与目标一致（否则拒绝）' }],
  ['schemas/config/runtime.schema.json#runtime.max_active_sessions', { shape: 'index', evidence: 'crates/nai-runtime/src/composition.rs', note: '转成 registry::Limits.sessions，越界即拒绝组合' }],
  ['schemas/config/runtime.schema.json#session_defaults.allow_auto_without_platform_grant', { shape: 'index', evidence: 'apps/engine/src/pipeline.rs', note: 'AutoSupervised bring-up 之前的门：不允许就直接拒绝' }],
  ['schemas/config/runtime.schema.json#media.lease_renew_ms', { shape: 'index', evidence: 'crates/nai-runtime/src/composition.rs', note: '租约续期间隔（同时被 store 的区间校验读）' }],
  ['schemas/config/runtime.schema.json#media.lease_expire_ms', { shape: 'index', evidence: 'crates/nai-runtime/src/composition.rs', note: '租约过期上限（同时被 store 的区间校验读）' }],
  ['schemas/config/runtime.schema.json#media.startup_buffer_ms', { shape: 'index', evidence: 'crates/nai-storage/src/store.rs', note: '写库前断言启播缓冲不大于最大缓冲' }],
  ['schemas/config/runtime.schema.json#media.max_buffer_ms', { shape: 'index', evidence: 'crates/nai-storage/src/store.rs', note: '同上：区间语义校验的另一端' }],
  ['schemas/config/runtime.schema.json#budgets.per_session_limit_minor', { shape: 'index', evidence: 'crates/nai-storage/src/store.rs', note: '写库前断言它可解析为 u64（字符串形态的十进制）' }],
  // upgrade-package：清单解析。直接索引 7 条 + 经辅助函数 14 条。
  ['schemas/config/upgrade-package.schema.json#requires', { shape: 'index', evidence: 'crates/nai-upgrade-core/src/manifest.rs', note: '依赖能力清单，逐项构造 Requirement' }],
  ['schemas/config/upgrade-package.schema.json#runtime', { shape: 'index', evidence: 'crates/nai-upgrade-core/src/manifest.rs', note: '运行时矩阵，逐项构造 RuntimeEntry' }],
  ['schemas/config/upgrade-package.schema.json#files', { shape: 'index', evidence: 'crates/nai-upgrade-core/src/manifest.rs', note: '文件声明（path/sha256/bytes），并做规范化与去重校验' }],
  ['schemas/config/upgrade-package.schema.json#cost.cloud', { shape: 'index', evidence: 'crates/nai-upgrade-core/src/manifest.rs', note: '成本构成之一，进入 CostSpec' }],
  ['schemas/config/upgrade-package.schema.json#cost.download', { shape: 'index', evidence: 'crates/nai-upgrade-core/src/manifest.rs', note: '同上：下载成本' }],
  ['schemas/config/upgrade-package.schema.json#cost.storage', { shape: 'index', evidence: 'crates/nai-upgrade-core/src/manifest.rs', note: '同上：存储成本' }],
  ['schemas/config/upgrade-package.schema.json#cost.runtime_delta', { shape: 'index', evidence: 'crates/nai-upgrade-core/src/manifest.rs', note: '同上：预估运行增量' }],
  ['schemas/config/upgrade-package.schema.json#package_id', { shape: 'helper', evidence: 'crates/nai-upgrade-core/src/manifest.rs', note: 'text(&value, "package_id")：包身份' }],
  ['schemas/config/upgrade-package.schema.json#package_version', { shape: 'helper', evidence: 'crates/nai-upgrade-core/src/manifest.rs', note: 'text(&value, "package_version")：版本，并断言可解析' }],
  ['schemas/config/upgrade-package.schema.json#kind', { shape: 'helper', evidence: 'crates/nai-upgrade-core/src/manifest.rs', note: 'text(&value, "kind")：包类型解析' }],
  ['schemas/config/upgrade-package.schema.json#kernel_range', { shape: 'helper', evidence: 'crates/nai-upgrade-core/src/manifest.rs', note: 'text(&value, "kernel_range")：目标内核范围' }],
  ['schemas/config/upgrade-package.schema.json#hosts', { shape: 'helper', evidence: 'crates/nai-upgrade-core/src/manifest.rs', note: 'strings(&value, "hosts")：支持的宿主' }],
  ['schemas/config/upgrade-package.schema.json#os', { shape: 'helper', evidence: 'crates/nai-upgrade-core/src/manifest.rs', note: 'strings(&value, "os")：支持的操作系统' }],
  ['schemas/config/upgrade-package.schema.json#architectures', { shape: 'helper', evidence: 'crates/nai-upgrade-core/src/manifest.rs', note: 'strings(&value, "architectures")：支持的架构' }],
  ['schemas/config/upgrade-package.schema.json#install_bytes', { shape: 'helper', evidence: 'crates/nai-upgrade-core/src/manifest.rs', note: 'amount(&value, "install_bytes")：安装字节数' }],
  ['schemas/config/upgrade-package.schema.json#migrated_bytes', { shape: 'helper', evidence: 'crates/nai-upgrade-core/src/manifest.rs', note: 'amount(&value, "migrated_bytes")：迁移字节数' }],
  ['schemas/config/upgrade-package.schema.json#signer_ref', { shape: 'helper', evidence: 'crates/nai-upgrade-core/src/manifest.rs', note: 'text(&value, "signer_ref")：签名者引用' }],
  ['schemas/config/upgrade-package.schema.json#license_ref', { shape: 'helper', evidence: 'crates/nai-upgrade-core/src/manifest.rs', note: 'text(&value, "license_ref")：许可引用' }],
  ['schemas/config/upgrade-package.schema.json#migration_ref', { shape: 'helper', evidence: 'crates/nai-upgrade-core/src/manifest.rs', note: 'text(&value, "migration_ref")：迁移引用' }],
  ['schemas/config/upgrade-package.schema.json#rollback_range', { shape: 'helper', evidence: 'crates/nai-upgrade-core/src/manifest.rs', note: 'text(&value, "rollback_range")：回滚兼容范围' }],
  ['schemas/config/upgrade-package.schema.json#cost.currency', { shape: 'helper', evidence: 'crates/nai-upgrade-core/src/manifest.rs', note: 'text(cost, "currency")：成本币种' }],
]);

/// **[C] 既无读取点、也未被钉死的键 → 成因 id**（逐键具名，手写）。
export const UNWIRED = new Map([
  // runtime：沿用 W90 的 23 条（扩面后逐项不变）。
  ['schemas/config/runtime.schema.json#scheduler.max_pending_utterances', 'constructor-injected'],
  ['schemas/config/runtime.schema.json#scheduler.max_pending_speech_ms', 'constructor-injected'],
  ['schemas/config/runtime.schema.json#scheduler.qa_ttl_ms', 'constructor-injected'],
  ['schemas/config/runtime.schema.json#scheduler.welcome_ttl_ms', 'constructor-injected'],
  ['schemas/config/runtime.schema.json#scheduler.thanks_ttl_ms', 'constructor-injected'],
  ['schemas/config/runtime.schema.json#knowledge.mode', 'constructor-injected'],
  ['schemas/config/runtime.schema.json#knowledge.fts_enabled', 'constructor-injected'],
  ['schemas/config/runtime.schema.json#knowledge.semantic_enabled', 'constructor-injected'],
  ['schemas/config/runtime.schema.json#knowledge.remote_source_refs', 'constructor-injected'],
  ['schemas/config/runtime.schema.json#knowledge.index_during_live', 'constructor-injected'],
  ['schemas/config/runtime.schema.json#media.output_sample_rate', 'constructor-injected'],
  ['schemas/config/runtime.schema.json#budgets.currency', 'constructor-injected'],
  ['schemas/config/runtime.schema.json#session_defaults.mode', 'hardcoded-default'],
  ['schemas/config/runtime.schema.json#runtime.plugin_catalog', 'surface-absent'],
  ['schemas/config/runtime.schema.json#runtime.control_listener', 'surface-absent'],
  ['schemas/config/runtime.schema.json#runtime.http_enabled', 'surface-absent'],
  ['schemas/config/runtime.schema.json#cloud.control_enabled', 'same-name-other-object'],
  ['schemas/config/runtime.schema.json#cloud.relay_enabled', 'same-name-other-object'],
  ['schemas/config/runtime.schema.json#cloud.render_enabled', 'same-name-other-object'],
  ['schemas/config/runtime.schema.json#cache.tts_max_bytes', 'surface-absent'],
  ['schemas/config/runtime.schema.json#budgets.soft_threshold_ratio', 'surface-absent'],
  ['schemas/config/runtime.schema.json#bindings', 'binding-consumer-absent'],
  ['schemas/config/runtime.schema.json#session_defaults.viewer_memory_enabled', 'awaiting-decision'],
  // backup：写入侧生成、读侧从不读。
  ['schemas/config/backup.schema.json#created_at_utc_ms', 'write-only'],
  // binding-plan：整份 schema 只有 Ajv 校验（+ 生成 TS 类型），无任何运行时读取，且**没有归因文件**。
  ['schemas/config/binding-plan.schema.json#session_id', 'validator-only'],
  ['schemas/config/binding-plan.schema.json#mode', 'validator-only'],
  ['schemas/config/binding-plan.schema.json#requirements', 'validator-only'],
  ['schemas/config/binding-plan.schema.json#selections', 'validator-only'],
  ['schemas/config/binding-plan.schema.json#emits_output', 'validator-only'],
  // provider：两个面在生产代码里连键名都不出现；三个命中**别的对象**的同名键。
  ['schemas/config/provider.schema.json#region', 'surface-absent'],
  ['schemas/config/provider.schema.json#data_policy_ref', 'surface-absent'],
  ['schemas/config/provider.schema.json#timeout_ms', 'same-name-other-object'],
  ['schemas/config/provider.schema.json#max_retries', 'same-name-other-object'],
  ['schemas/config/provider.schema.json#max_concurrency', 'same-name-other-object'],
]);

/// **成因表**。每条 MUST 是可核实的陈述，且 MUST 至少被一个键引用（否则它是装饰）。
export const UNWIRED_CAUSES = new Map([
  [
    'constructor-injected',
    '值以构造器/参数由调用方注入，而该调用方在生产代码里**不存在**（实测：`Book::new(workspace, limits)`、' +
      '`EventIngress::new(…, capacity, state_capacity, dedup_capacity)`、`Scheduler::new(…, policy)`、' +
      '`RemoteGateway::new(policy)` 的调用方只有 `tests/`；`nai-media-core` 的采样率由 `PlaybackSpec` 给）。' +
      '⇒ 配置面这一段的接线从未发生，值由调用方直接给。',
  ],
  [
    'hardcoded-default',
    '默认值在代码里**硬编码**：`Session::new` 把初始模式写成 `SessionMode::Assisted`（`crates/nai-kernel/src/session.rs`），' +
      '不读配置。⇒ 配置面的 `mode` 改了不会有任何效果。',
  ],
  [
    'surface-absent',
    '该面在生产代码里**连键名都不出现**（实测：非测试源码里 `plugin_catalog`/`control_listener`/`http_enabled`/' +
      '`tts_max_bytes` 命中 0 次；`budgets-core` 的 `Ceiling`/`Limits` 里没有软阈值字段；' +
      '`provider.region`/`provider.data_policy_ref` 在**非测试非生成**的 `.rs` 里零命中）。' +
      '⇒ 不是「读法不同」，是这一面尚未接线。',
  ],
  [
    'same-name-other-object',
    '**同名不同物**：裸键名在生产代码里出现，但读的是**另一个对象**的键。实测：`cloud` 的唯一生产代码是' +
      '`crates/nai-upgrade-core/src/cloud.rs`（A10 云断连依赖方向），读的是**升级/驻留清单**的字段；' +
      '`timeout_ms` 命中 `crates/nai-kernel/src/evaluate/skill.rs` 的**技能**超时；`max_concurrency` 同样命中' +
      '`skill.rs`；`max_retries` 命中 `crates/nai-knowledge-core/src/remote.rs` 的**知识源**重试。' +
      '⇒ 本 schema 的这几个键没有消费者。',
  ],
  [
    'binding-consumer-absent',
    '绑定的运行时消费者尚未存在：`ConfigKind` 只有 `Runtime`/`Provider` 两种（`crates/nai-storage/src/store.rs`），' +
      '`capability_bindings` 表在生产代码里只被测试提及。⇒ 配置面的 `bindings` 没有读者。',
  ],
  [
    'awaiting-decision',
    '**接线本身是一个未决的产品决策**（不是遗漏）：W89/ADR 0067 裁定的是「判定面收口」，' +
      '把配置面的键接到 `MemoryPolicy` 会**改变运行时行为**（默认关闭的开关第一次在配置面生效），' +
      '超出那一轮的范围。⇒ 本键在册，状态就是「待裁定」。',
  ],
  [
    'validator-only',
    '该键**只有校验、没有消费**：`binding-plan.schema.json` 由 `scripts/contracts/validation.mjs` 的' +
      '`AJV_SOURCES` 校验并生成 TS 类型，**没有任何运行时读取**，该 schema 也**没有归因文件**' +
      '（全仓无 `include_str!` 它）。⇒ 「契约被校验」≠「契约的值被运行时看见」；' +
      '这是**事实登记**，不是豁免（文件级守卫由 W73 覆盖）。',
  ],
  [
    'write-only',
    '该键是**写入侧生成的**：`crates/nai-storage/src/snapshot.rs` 在构造备份清单时用 `crate::utc_ms()` 写入' +
      '`created_at_utc_ms`，而**读取侧从不读它**（恢复流程只比对 runtime 版本、契约哈希与工件）。' +
      '⇒ 它有生产者、没有消费者。',
  ],
]);

/// 判据一：三分类闭世界。
export function partitionErrors(schemas, scanned, wired, unwired) {
  const errors = [];
  const guarded = new Map();
  for (const [schemaPath, schema] of schemas) {
    for (const key of guardedKeys(schema)) guarded.set(keyLabel(schemaPath, key.path), key);
  }
  const pinned = new Set();
  for (const [label, key] of guarded) {
    if (!scanned.has(label) && key.node.const !== undefined) pinned.add(label);
  }
  for (const label of wired.keys()) {
    if (!guarded.has(label)) errors.push(`${label} is named as wired but no schema requires it — a stale entry is a claim nobody can act on`);
    if (unwired.has(label)) errors.push(`${label} is both wired and registered as unwired; one key, one state — drop one side`);
    if (pinned.has(label)) errors.push(`${label} is named as wired but nothing in the repository reads it; a named reader with no read site is exactly the state this guard exists to catch`);
  }
  for (const [label, cause] of unwired) {
    if (!guarded.has(label)) errors.push(`${label} is registered as unwired but no schema requires it — drop the stale entry`);
    if (wired.has(label)) errors.push(`${label} is registered as unwired but a read site was found — a registration that shadows a real reader makes the guard self-satisfying`);
    if (pinned.has(label)) errors.push(`${label} is registered as unwired but the schema pins its value with \`const\`; a value that cannot differ needs no reader — move it out of the ledger`);
    if (cause === undefined || String(cause).trim() === '') errors.push(`${label} is registered without a cause; a registration is exactly its cause`);
    else if (!UNWIRED_CAUSES.has(cause)) errors.push(`${label} names cause "${cause}", which is not in the cause table — an unknown cause is a label, not a reason`);
  }
  for (const [label] of guarded) {
    if (scanned.has(label) || pinned.has(label) || unwired.has(label)) continue;
    errors.push(
      `${label} is required by its schema, nothing reads it, the schema does not pin it, and it is not registered — ` +
        `a configuration key nobody reads and nobody named is a value the runtime silently ignores`,
    );
  }
  // [A] 这一侧也要**双向**：有读取点的键 MUST 具名在册。
  //
  // 少了这一条，台账对闭世界**不承重**——闭世界只要求「键落在三类之一」，而 [A] 由扫描自动成立，
  // 于是清空 `WIRED` 判据照旧全绿（W90 的第一版实测如此）。⇒ 具名是**主张**，扫描是**观测**。
  for (const [label, shapes] of scanned) {
    if (wired.has(label)) continue;
    const where = [...shapes].map(([shape, files]) => `${shape}@${[...files].join(',')}`).join(' + ');
    errors.push(
      `${label} has a read site (${where}) but is not named in the wired ledger — ` +
        `an unnamed reader cannot be re-checked when it moves, and a read site found by accident is not a decision`,
    );
  }
  return errors;
}

/// 判据二：**[A] 的每条证据 MUST 自清理**——具名的证据文件里真的要有所声明的**形状**，
/// 且文件 MUST 被仓库跟踪。
export function evidenceErrors(wired, sources, tracked) {
  const errors = [];
  const byFile = new Map(sources.map(s => [s.file, s.text]));
  const serdeByFile = serdeFieldsByFile(sources);
  const helpersByFile = keyHelpersByFile(sources);
  for (const [label, { shape, evidence }] of wired) {
    if (evidence === undefined || evidence.trim() === '') {
      errors.push(`${label} is named as wired without an evidence file; an unnamed reader cannot be re-checked`);
      continue;
    }
    const text = byFile.get(evidence);
    if (text === undefined) {
      errors.push(`${label} names ${evidence} as its reader, but that file is not in the scan set`);
      continue;
    }
    const keyPath = label.slice(label.indexOf('#') + 1);
    const name = keyPath.split('.').pop();
    const found =
      shape === 'serde'
        ? (serdeByFile.get(evidence) ?? new Set()).has(name)
        : shape === 'helper'
          ? helperShape(helpersByFile.get(evidence) ?? new Set(), name).test(text)
          : indexShape(keyPath).test(text);
    if (!found) {
      errors.push(`${label} names ${evidence} as its ${shape} reader, but that file has no such read of the key — the reader moved or was deleted`);
    }
    if (tracked !== undefined && !tracked.has(evidence)) {
      errors.push(`${evidence} provides a read site but version control does not track it`);
    }
  }
  return errors;
}

/// 判据三：成因表 MUST 付得起——每条成因至少被一个在册键引用，否则它是装饰。
export function causeErrors(unwired, causes) {
  const used = new Set(unwired.values());
  return [...causes.keys()]
    .filter(cause => !used.has(cause))
    .map(cause => `cause "${cause}" is never cited by a registered key; a cause nobody cites changes no verdict — remove it or use it`);
}

/// 派生 **`include_str!` 消费者**：schema → 在 Rust 源码里 `include_str!` 它的文件。
///
/// 只认 Rust 的 `include_str!`，MUST NOT 把注释里的路径或其它语言的字符串算作消费
/// （`sources` 已由调用方做过注释剥离）。
export function consumersOf(sources, schemas) {
  const out = new Map([...schemas.keys()].map(schema => [schema, new Set()]));
  for (const { file, text } of sources) {
    for (const match of text.matchAll(/include_str!\([^)]*schemas\/([A-Za-z0-9_./-]+\.schema\.json)/g)) {
      out.get(`schemas/${match[1]}`)?.add(file);
    }
  }
  return out;
}

/// **具名外部读者**：读该 schema 但不 `include_str!` 它的文件（经 `composition.config()` 拿 `Value`）。
///
/// 实测 `runtime` 有两条，正是 W90 的 [A] 里**唯二**跨出 `store.rs` 的条目。其余 schema 为 0——
/// 它们的读者都在 `include_str!` 的文件里（`binding-plan` 则连消费者都没有）。
export const EXTERNAL_READERS = new Map([
  [
    'schemas/config/runtime.schema.json',
    new Set(['crates/nai-runtime/src/composition.rs', 'apps/engine/src/pipeline.rs']),
  ],
]);

/// 判据四：**归因集 MUST 自清理**——每条具名外部读者 MUST 真的有该 schema 受判键的读取点。
///
/// 归因集不是豁免：它是「谁在 schema 之外读这份配置」的**声明**，而声明 MUST 可被复核。
/// 声明了却不再读 ⇒ 报陈旧（与 `WIRED` 的证据自清理同形）。
export function attributionErrors(schemas, externalReaders, consumers, sources) {
  const errors = [];
  const byFile = new Map(sources.map(source => [source.file, source.text]));
  for (const [schemaPath, files] of externalReaders) {
    const schema = schemas.get(schemaPath);
    if (schema === undefined) {
      errors.push(`${schemaPath} declares external readers but is not a config schema under judgement — drop the stale entry`);
      continue;
    }
    const builtin = consumers.get(schemaPath) ?? new Set();
    for (const file of files) {
      if (builtin.has(file)) {
        errors.push(`${file} is declared an external reader of ${schemaPath} but it already include_str!'s that schema — the declaration is redundant`);
        continue;
      }
      const text = byFile.get(file);
      if (text === undefined) {
        errors.push(`${file} is declared an external reader of ${schemaPath} but it is not in the scan set — an excluded or untracked file cannot be a named reader`);
        continue;
      }
      if (!guardedKeys(schema).some(key => indexShape(key.path).test(text))) {
        errors.push(`${file} is declared an external reader of ${schemaPath} but reads none of its keys — an attribution that stopped being true must be reported, not silently kept`);
      }
    }
  }
  return errors;
}

/// 判据五：具名排除 MUST 命中对象、且理由非空。陈旧排除会被指名（排除不是豁免）。
export function scanExclusionErrors(exclusions, files) {
  const errors = [];
  for (const [pattern, reason] of exclusions) {
    if (reason === undefined || String(reason).trim() === '') errors.push(`${pattern} is excluded from the scan without a reason`);
    if (!files.some(file => inScan(file, new Map([[pattern, reason]])))) {
      errors.push(`${pattern} is excluded from the scan but matches no file — a stale exclusion hides nothing and proves nothing`);
    }
  }
  return errors;
}

export { SCAN_EXCLUSIONS };
