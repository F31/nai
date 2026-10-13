/// 运行时配置面的**消费状态**：受判 `required` 键的三分类闭世界。
///
/// 「配置里有这个键」MUST NOT 被当作「运行时看得见它的值」。W89 的 F1 抓到的形状是
/// 「**同层字段一个有主、一个无主**」——`session_defaults` 里 `allow_auto_without_platform_grant`
/// 有读取点、`viewer_memory_enabled` 没有。本模块把**那一条对照一般化到整个配置面**：
/// 每一个受判键都必须落在三类里的**恰一个**上，且三类 MUST 两两不相交、并集**恰等于**受判键集合。
///
/// 三类：
/// - **[A] 有读取点**——由**派生**得到（扫描非测试、非生成、已跟踪的源码），台账逐键具名写出
///   **证据文件**，并自清理（证据文件里 MUST 真的含有那个读取形状）。
/// - **[B] 值被 schema 钉死**（`const`）——值不可能不同 ⇒ 不需要读者。这一类是**派生**的，
///   不是台账：钉死是 schema 的性质，不是谁的主张。
/// - **[C] 既无读取点、也未被钉死**——改它的值**不改变任何行为**。这一族**逐键在册**并附成因，
///   成因表本身也 MUST 付得起（每条成因 MUST 至少被一个键引用）。
///
/// 三类**都非空**是本判据的前提：任一类为空，对它的断言就在空集上空转（非空洞）。
///
/// ⚠️ 两张台账是**手写**的（A 的键→证据文件、C 的键→成因 id）。若把 C 派生为「A∪B 的补集」，
/// 判据就成了同义反复：新键会**自动**落进「未接线」而不需要任何人签名。手写 + 与派生结果
/// **双向相等**，才是「有人为这个状态负责」的形状（同 W73 对具名例外的处置）。
///
/// ⇒ **A 与 C 两侧都是双向的**：有读取点的键 MUST 在册（观测不能替人签名），在册的键 MUST
/// 真的没有读取点（签名不能遮住观测）。少了前者，台账对闭世界不承重——闭世界只要求
/// 「落在三类之一」，而 [A] 由扫描自动成立 ⇒ 清空台账判据照旧全绿（本判据的第一版实测如此）。

export const RUNTIME_CONFIG_SCHEMA = 'schemas/config/runtime.schema.json';

/// 递归枚举 `required` 键。返回的每一项都带 `container` 标记：
/// **自己有 `required` 子键的容器**（`runtime`/`session_defaults`/`scheduler`/`media`/`knowledge`/
/// `budgets`/`cache`/`cloud`）不受判——JSON 里要读到 `config["media"]["lease_renew_ms"]`
/// 必须先读到 `config["media"]`，所以容器的消费**由其受判子键蕴含**。
///
/// 「容器」是**派生**的（`type === 'object'` 且 `required.length > 0`），MUST NOT 写成键名清单。
/// 一个 `required` 的 object 键若**没有** `required` 子键（如 `bindings`），它不能被任何子键蕴含
/// ⇒ 它**必须自己受判**。这条边界就是「无 required 子键的 object 键」的全部内容。
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

/// 读取形状：`EXPR["<name>"]`。
///
/// 井号前面**必须**是一个表达式字符（标识符 / `)` / `]`）。少了这一条，Rust 的**数组字面量**
/// 会被误判：实测 `xtask/src/deps.rs:441` 的 `&["knowledge"]` 让 `knowledge` 这个容器键
/// 凭空获得一个「读者」，而它一个读者都没有。同族的假阳性还有 `vec!["a"]`、`= ["a"]`、`(["a"])`
/// ——它们的共同点是**前面不是表达式**。
///
/// ⚠️ 这个形状**只认这一种**读取写法。若将来有人用反序列化到结构体的方式读配置，本判据会
/// 把该键报成「未接线」（**假阳性方向**）。按「假阳性与假阴性不对称 ⇒ 永远选安全方向」，
/// 这个方向是对的：它只会让接线**更难**被漏掉，不会凭空造出消费者。
export function readShape(name) {
  return new RegExp(`[A-Za-z0-9_)\\]]\\["${name}"\\]`);
}

/// 扫描面的**具名排除**。每条 MUST 附理由：排除不是豁免，是一条可核实的陈述。
///
/// 定义面（`schemas/config/runtime.schema.json`）**不在这张表里**，理由不是「我们记得排除它」，
/// 而是它凭**语言闭包**就已经在扫描集之外：`.json` 不在 `CODE_LANGUAGES` 里（W73 立下的口径是
/// 「扫描 MUST 只认代码文件」）。本判据在测试侧另有一条断言钉住这件事——一旦有人把 `.json`
/// 加进语言表，那条断言会开火，逼他在这里补一条**有理由**的排除。
///
/// ⚠️ 「测试面」MUST NOT 只按路径前缀排除：`crates/*/tests/*.rs`、`apps/*/tests/common/mod.rs`
/// 与 `src/tests.rs` 都住在生产源码的旁边，按前缀排会**漏掉它们**（实测第一版就漏了
/// `crates/nai-runtime/tests/composition.rs`）。⇒ 匹配规则是**路径段**，见 `inScan`。
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
    '生成面。生成物由 schema 派生（`RuntimeConfig` 接口的字段名逐字来自 `required`/`properties`），' +
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
///
/// 「路径段」这一条是必须的：`tests/` 作为前缀只挡得住仓库根的 `tests/`，
/// 而 `crates/nai-runtime/tests/composition.rs` 里的读取在生产读数面前同样是噪声。
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

/// 从源码里派生**读取点**：键路径 → 命中的文件集。
///
/// `sources` 是 `{file, text}` 列表——**已经**排除了测试面、生成面与判据自身
/// （排除由 `SCAN_EXCLUSIONS` 具名给出，见 `exclusionErrors`）。本函数不做排除，
/// 排除是调用方的事：判据的输入 MUST 等于它断言的对象。
export function readSites(keys, sources) {
  const sites = new Map();
  for (const key of keys) {
    const shape = readShape(key.name);
    for (const { file, text } of sources) {
      if (!shape.test(text)) continue;
      if (!sites.has(key.path)) sites.set(key.path, new Set());
      sites.get(key.path).add(file);
    }
  }
  return sites;
}


/// **[A] 有读取点的键 → 证据文件与说明**（逐键具名，手写）。
///
/// `evidence` 是**一个**文件：判据会断言它真的含有该键的读取形状（读取点消失 ⇒ 立刻红）。
/// `note` 说明**读它做什么**——只写「有人读」不足以让人判断这条接线是否成立。
export const WIRED = new Map([
  ['edition', { evidence: 'crates/nai-storage/src/store.rs', note: '写库时把 profile 的 edition 落进 runtime_profiles.edition' }],
  ['deployment', { evidence: 'crates/nai-storage/src/store.rs', note: '同上：deployment 随 profile 落库' }],
  ['workspace_id', { evidence: 'crates/nai-storage/src/store.rs', note: '写库前断言配置声明的 workspace 与目标一致（否则拒绝）' }],
  [
    'runtime.max_active_sessions',
    { evidence: 'crates/nai-runtime/src/composition.rs', note: '转成 registry::Limits.sessions，越界即拒绝组合' },
  ],
  [
    'session_defaults.allow_auto_without_platform_grant',
    { evidence: 'apps/engine/src/pipeline.rs', note: 'AutoSupervised bring-up 之前的门：不允许就直接拒绝' },
  ],
  [
    'media.lease_renew_ms',
    { evidence: 'crates/nai-runtime/src/composition.rs', note: '租约续期间隔（同时被 store 的区间校验读）' },
  ],
  [
    'media.lease_expire_ms',
    { evidence: 'crates/nai-runtime/src/composition.rs', note: '租约过期上限（同时被 store 的区间校验读）' },
  ],
  ['media.startup_buffer_ms', { evidence: 'crates/nai-storage/src/store.rs', note: '写库前断言启播缓冲不大于最大缓冲' }],
  ['media.max_buffer_ms', { evidence: 'crates/nai-storage/src/store.rs', note: '同上：区间语义校验的另一端' }],
  ['budgets.per_session_limit_minor', { evidence: 'crates/nai-storage/src/store.rs', note: '写库前断言它可解析为 u64（字符串形态的十进制）' }],
]);

/// **[C] 既无读取点、也未被钉死的键 → 成因 id**（逐键具名，手写）。
///
/// 这不是豁免清单：每条都必须能追溯到 `UNWIRED_CAUSES` 里一条**可核实**的成因，
/// 且一旦某键出现读取点或被钉死，判据会立刻指名它（在册状态过期）。
export const UNWIRED = new Map([
  // scheduler 段：值以构造器参数注入，而生产调用方**不存在**。
  ['scheduler.max_pending_utterances', 'constructor-injected'],
  ['scheduler.max_pending_speech_ms', 'constructor-injected'],
  ['scheduler.qa_ttl_ms', 'constructor-injected'],
  ['scheduler.welcome_ttl_ms', 'constructor-injected'],
  ['scheduler.thanks_ttl_ms', 'constructor-injected'],
  // knowledge 段：同上。
  ['knowledge.mode', 'constructor-injected'],
  ['knowledge.fts_enabled', 'constructor-injected'],
  ['knowledge.semantic_enabled', 'constructor-injected'],
  ['knowledge.remote_source_refs', 'constructor-injected'],
  ['knowledge.index_during_live', 'constructor-injected'],
  ['media.output_sample_rate', 'constructor-injected'],
  ['budgets.currency', 'constructor-injected'],
  // 默认值在代码里硬编码，配置面从未接线。
  ['session_defaults.mode', 'hardcoded-default'],
  // 该面在生产代码里连键名都不出现。
  ['runtime.plugin_catalog', 'surface-absent'],
  ['runtime.control_listener', 'surface-absent'],
  ['runtime.http_enabled', 'surface-absent'],
  ['cloud.control_enabled', 'same-name-other-object'],
  ['cloud.relay_enabled', 'same-name-other-object'],
  ['cloud.render_enabled', 'same-name-other-object'],
  ['cache.tts_max_bytes', 'surface-absent'],
  ['budgets.soft_threshold_ratio', 'surface-absent'],
  // 绑定的运行时消费者尚未存在。
  ['bindings', 'binding-consumer-absent'],
  // 接线是未决的产品决策。
  ['session_defaults.viewer_memory_enabled', 'awaiting-decision'],
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
      '`tts_max_bytes` 命中 0 次；`budgets-core` 的 `Ceiling`/`Limits` 里没有软阈值字段）。' +
      '⇒ 不是「读法不同」，是这一面尚未接线。',
  ],
  [
    'same-name-other-object',
    '**同名不同物**：唯一带 `cloud` 的生产代码是 `crates/nai-upgrade-core/src/cloud.rs`（A10 的云断连依赖方向），' +
      '它读的是**升级/驻留清单**的 `cloud` 字段，不是本配置段。⇒ 运行时的云控制/中转/渲染三个开关没有消费者。',
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
]);

/// 判据一：三分类闭世界。`scanned` 是派生出来的读取点键集。
export function partitionErrors(guarded, scanned, wired, unwired) {
  const errors = [];
  const guardedPaths = new Set(guarded.map(entry => entry.path));
  const pinned = new Set(guarded.filter(entry => !scanned.has(entry.path) && entry.node.const !== undefined).map(entry => entry.path));
  for (const path of wired.keys()) {
    if (!guardedPaths.has(path)) errors.push(`${path} is named as wired but the schema does not require it — a stale entry is a claim nobody can act on`);
    if (unwired.has(path)) errors.push(`${path} is both wired and registered as unwired; one key, one state — drop one side`);
    if (pinned.has(path)) errors.push(`${path} is named as wired but nothing in the repository reads it; a named reader with no read site is exactly the state this guard exists to catch`);
  }
  for (const [path, cause] of unwired) {
    if (!guardedPaths.has(path)) errors.push(`${path} is registered as unwired but the schema does not require it — drop the stale entry`);
    if (wired.has(path)) errors.push(`${path} is registered as unwired but a read site was found in ${[...(scanned.get(path) ?? [])].join(', ')} — a registration that shadows a real reader makes the guard self-satisfying`);
    if (pinned.has(path)) errors.push(`${path} is registered as unwired but the schema pins its value with \`const\`; a value that cannot differ needs no reader — move it out of the ledger`);
    if (cause === undefined || String(cause).trim() === '') errors.push(`${path} is registered without a cause; a registration is exactly its cause`);
    else if (!UNWIRED_CAUSES.has(cause)) errors.push(`${path} names cause "${cause}", which is not in the cause table — an unknown cause is a label, not a reason`);
  }
  for (const key of guarded) {
    if (scanned.has(key.path) || pinned.has(key.path) || unwired.has(key.path)) continue;
    errors.push(
      `${key.path} is required by the schema, nothing reads it, the schema does not pin it, and it is not registered — ` +
        `a configuration key nobody reads and nobody named is a value the runtime silently ignores`,
    );
  }
  // [A] 这一侧也要**双向**：有读取点的键 MUST 具名在册。
  //
  // 少了这一条，台账对闭世界**不承重**——闭世界只要求「键落在三类之一」，而 [A] 由扫描自动成立，
  // 于是清空 `WIRED` 判据照旧全绿（实测：本判据的第一版就是如此）。那意味着一个键可以被**任何**
  // 文件读到（一个临时脚本、一次实验）却不经任何人签名，而「谁来复核这个读者」无人可问。
  // ⇒ 具名是**主张**，扫描是**观测**；两者 MUST 相等。
  for (const path of scanned.keys()) {
    if (wired.has(path)) continue;
    errors.push(
      `${path} has a read site in ${[...scanned.get(path)].join(', ')} but is not named in the wired ledger — ` +
        `an unnamed reader cannot be re-checked when it moves, and a read site found by accident is not a decision`,
    );
  }
  return errors;
}

/// 判据二：**[A] 的每条证据 MUST 自清理**——具名的证据文件里真的要有那个读取形状，且文件 MUST 被仓库跟踪。
export function evidenceErrors(wired, scanned, tracked) {
  const errors = [];
  for (const [path, { evidence }] of wired) {
    if (evidence === undefined || evidence.trim() === '') {
      errors.push(`${path} is named as wired without an evidence file; an unnamed reader cannot be re-checked`);
      continue;
    }
    const hits = scanned.get(path);
    if (hits === undefined || !hits.has(evidence)) {
      errors.push(`${path} names ${evidence} as its reader, but that file has no \`["…"]\` read of the key — the reader moved or was deleted`);
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

/// 判据四：具名排除 MUST 命中对象、且理由非空。陈旧排除会被指名（排除不是豁免）。
///
/// 名字里带 `scan` 是为了与 W74 那份**目录级**排除（`tests/contracts/schema.test.mjs` 的
/// `EXCLUDED_DIRS` + `.gitignore` 证据）分开：两份排除回答的是两个不同的问题。
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
