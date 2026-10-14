import YAML from 'yaml';

/// workflow 的**触发面**（`on:`）——「门控什么时候跑」的闭世界。
///
/// 为什么需要它：W95/W96/W97/W98/W99/W100/W101 补的全是「workflow **里面**」的层（步集、命令面、
/// 包集合、npm 脚本执行面、站点层级、执行配置面、目标绑定）。而「workflow **什么时候跑**」——
/// `on:` ——**不在任何判定域内**：`workflowJobsOf` 只读 `doc?.jobs`，**从不读 `doc?.on`**；
/// 全仓 `grep`「`.on` / `trigger` / `pull_request` / `branches:`」在 `scripts/contracts/*.mjs`
/// 与 `tests/**` 下**零命中**。于是它是上面**全部判定的前提**，而这个前提**没有任何判据**。
///
/// 实测（W102 探针，改真文件、真跑 10 个 portable 套件、逐字节还原、`sha256` 逐字一致）：
/// - 删掉 `pull_request`（PR 不再被门控）⇒ `[0,…]` **零信号**；
/// - 把整个 `on:` 缩成仅 `workflow_dispatch`（push/PR 都不再触发）⇒ **零信号**；
/// - 把 `push` 限定到一个**不存在**的分支（push 事实上不再触发）⇒ **零信号**。
///
/// 这三条不是「漏了一个字段」：它们让**整套门控静默地不再运行**，而所有步层判据照样全绿。
/// 仓库里对此只有一句散文：「workflow 有 `on: push`，每次 push 都已自动触发」
/// （`AGENTS.md:651`、`docs/development-plan.md:541`）。散文不是判据。
///
/// 三条纪律照抄：**「已声明」≠「已执行」**（W75）、**「判据的输入 MUST 等于被断言对象」**、
/// **「能由结构蕴含的 MUST NOT 再具名一次」**（⇒ `kind` 只由事件表推导，MUST NOT 在在册条目里手写）。

/// **触发器类别的闭集**。它是 `triggerKindOf` 的值域；由**结构**保证（两个事件表互不相交），
/// 因此判据**不**再对推导结果做一次「∈ 闭集」的断言（能由结构蕴含的 MUST NOT 再具名一次）。
/// 它仍然非空洞：测试断言两个事件表的每一个成员都被推导出一个落在此集合里的类别，且两类都出现。
export const TRIGGER_KINDS = ['automatic', 'manual'];

/// **自动触发**的事件：由仓库自身的活动（推送／PR／合并队列／发布／定时／上游 workflow 结束）驱动。
/// 闭集，域外 ⇒ 具名失败（W90：一个未知取值是一个标签，不是一个分类）。
///
/// ⚠️ **为什么必须是闭集，而不是「除了手动表以外的都算自动」**：那样一个**拼错的事件名**
/// （`pus`）会被推导成 `automatic` ⇒ 一条「至少一个自动触发」的断言会在一个**永不触发**的
/// workflow 上通过——**假阴性**，最危险的方向（W92：假阳性与假阴性不对称）。
export const AUTOMATIC_EVENTS = [
  'push',
  'pull_request',
  'pull_request_target',
  'merge_group',
  'release',
  'schedule',
  'workflow_run',
];

/// **只能被显式启动**的事件：人点按钮（`workflow_dispatch`）、另一个 workflow 调用
/// （`workflow_call`）、外部 API 调用（`repository_dispatch`）。
/// 与 `AUTOMATIC_EVENTS` **不相交**——两个表一起构成事件名的闭世界。
export const MANUAL_EVENTS = ['workflow_dispatch', 'workflow_call', 'repository_dispatch'];

/// **过滤器键的闭集**：会决定「这个事件**是否真的会**触发」的键。
/// 与非过滤器（事件配置：`workflow_dispatch.inputs`、`schedule.cron`）的边界见 `workflowTriggersOf`。
/// 域外 ⇒ 具名失败——一个域外的键可能是笔误，也可能是一条**没人会去读**的抑制条件。
export const FILTER_KEYS = [
  'branches',
  'branches-ignore',
  'tags',
  'tags-ignore',
  'paths',
  'paths-ignore',
];

/// **在册触发面**：`{host, event, filters}`。
/// - `host`：workflow 文件（相对仓库根）；
/// - `event`：事件名，MUST ∈ `AUTOMATIC_EVENTS` ∪ `MANUAL_EVENTS`（**域外 ⇒ 具名失败**）；
///   **类别不写成字段**——它由 `triggerKindOf(event)` 推导（能由结构蕴含的 MUST NOT 再具名一次）；
/// - `filters`：`{<过滤器键>: [<非空字符串>…]}`，MUST **显式存在**（`{}` 表示「这个事件不施加任何过滤器」）。
///   「没有过滤器」也必须说出来：一个缺失的字段与一个空对象在可读性上不同。
///
/// **为什么单独一张表，而不塞进 `WORKFLOW_STEPS`**（对照 W101 决定 1）：W101 把 `target` 放在
/// step 自己的条目上，是因为 target 是 **step 这个对象**的属性。触发器**不是任何一个 step 的属性**，
/// 而是**整个 job 集的前置条件**——它有自己的对象、自己的表。W101 纪律的正确形式在这里是
/// 「同一件事 MUST NOT 两处定义」：「这个 workflow 有哪些触发器」只有本表一处。
export const WORKFLOW_TRIGGERS = [
  {host: '.github/workflows/foundation.yml', event: 'push', filters: {}},
  {host: '.github/workflows/foundation.yml', event: 'pull_request', filters: {}},
];

/// **在册触发面的字段闭世界**。条目上的键 MUST 恰好落在这一集里。
///
/// 为什么必须有它（W102 探针 P-B5 实测逼出来的一条）：给一个在册条目**手写** `kind: 'automatic'`
/// 时，判据**一个字都不说**——因为 `kind` 由事件表推导、判据不读它。于是一条**没人会读**的声明
/// 静静地挂在表上：写它的人以为它有意义（甚至以为改成 `manual` 就改变了语义）。
/// 这正是母题「**声明 MUST 付代价**」的反面，也是「闭世界 MUST 在每层成立」在这一层的落点。
export const TRIGGER_FIELDS = ['host', 'event', 'filters'];

/// 一个事件名属于哪一类。域外 ⇒ `null`（由调用方**具名失败**，MUST NOT 静默归到「自动」）。
///
/// 这是「自动 / 手动」的**唯一一处定义**：类别 MUST NOT 在在册条目里手写。
/// 手写一定会与表漂移，而漂移的方向恰好是「一个手动触发被当成自动触发」⇒ 假阴性。
export function triggerKindOf(event) {
  if (typeof event !== 'string') return null;
  if (MANUAL_EVENTS.includes(event)) return 'manual';
  if (AUTOMATIC_EVENTS.includes(event)) return 'automatic';
  return null;
}

/// 过滤器对账的**判定单元**：`(event, filter-key)`。
///
/// MUST NOT 用**裸键名**（W91：「同名」≠「同一个」）：`branches` 在 `push` 与 `pull_request` 上
/// 语义相同但**归属不同对象**——把 `pull_request` 的 `branches` 挪给 `push`，按裸键名比较
/// **看不出差别**。用 `\u0000` 分隔（不可能出现在事件名或键名里）避免拼接歧义。
export function filterKeyOf(event, key) {
  return `${event}\u0000${key}`;
}

function isPlainObject(value) {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isNonEmptyStringList(value) {
  return Array.isArray(value)
    && value.length > 0
    && value.every(item => typeof item === 'string' && item.trim() !== '');
}

/// 把一个事件的配置**按形状**归一化成过滤器映射。
///
/// 切分（与 W101 的 `targetsOf` / 判据同形）：**解析侧只认形状，判定侧才认域**。
/// 这里**不检查键名域**（`FILTER_KEYS` 由判据检查）——把域检查塞进解析器会让一个笔误
/// （`branch:` 少个 `s`）把**整个触发面**判成「读不懂」，丢失粒度、也说不清是哪种错。
///
/// 形状域外（例如 `workflow_dispatch: {inputs: …}` 的取值是对象、`schedule: [{cron: …}]`
/// 的取值是数组）⇒ **抛**一个具名错误。域外 ⇒ 具名失败，MUST NOT 静默降级——把一份
/// 「读不懂的配置」当成「没有配置」正是本模块要堵的那个洞。
function normalizeFilters(event, config) {
  if (config === null || config === undefined) return {};
  if (!isPlainObject(config)) {
    throw new Error(`the "${event}" trigger carries a configuration that is not a filter map`);
  }
  const filters = {};
  for (const [key, value] of Object.entries(config)) {
    if (!isNonEmptyStringList(value)) {
      throw new Error(`the "${event}" trigger's "${key}" entry is not a non-empty list of strings`);
    }
    filters[key] = [...value];
  }
  return filters;
}

/// 把一份 workflow 文本**解析**成触发面：`[{event, filters}]`。
///
/// 纯函数、只读文本：不执行任何命令、不读磁盘（铁律二：宿主瞬态 MUST NOT 改变结论）。
/// `on:` 的**三种写法**都归一化到同一个形状（实测，`yaml` v2.9.1）：
/// - `on: push`（字符串）、`on: [push, pull_request]`（数组）、`on:\n  push:\n  pull_request:`（映射）；
/// - ⚠️ `on` 是**字符串键**，不是布尔 `true`（`yaml` v2 默认 YAML 1.2 core schema，实测
///   `Object.keys(parse('on:\n  push:'))` 为 `['on']`）⇒ `doc.on` 可靠，不需要防御 `doc.true`。
///
/// 不可读的情形**一律抛具名错误**（由判据转成一条具名失败）：YAML 不可解析、`on:` 键缺失、
/// `on:` 为空（`null` / `{}` / `[]` / 空串）、事件名不是字符串、同一事件出现两次、事件配置形状域外。
export function workflowTriggersOf(text) {
  const doc = YAML.parse(text);
  if (!isPlainObject(doc) || !Object.prototype.hasOwnProperty.call(doc, 'on')) {
    throw new Error('the workflow declares no "on:" trigger surface');
  }
  const raw = doc.on;
  if (raw === null || raw === undefined) {
    throw new Error('the "on:" trigger surface is empty');
  }

  const entries = [];
  if (typeof raw === 'string') {
    if (raw.trim() === '') throw new Error('the "on:" trigger surface is empty');
    entries.push([raw.trim(), null]);
  } else if (Array.isArray(raw)) {
    if (raw.length === 0) throw new Error('the "on:" trigger surface is empty');
    for (const item of raw) {
      if (typeof item !== 'string' || item.trim() === '') {
        throw new Error('the "on:" trigger list carries an entry that is not an event name');
      }
      entries.push([item.trim(), null]);
    }
  } else if (isPlainObject(raw)) {
    for (const [event, config] of Object.entries(raw)) entries.push([event, config]);
    if (entries.length === 0) throw new Error('the "on:" trigger surface is empty');
  } else {
    throw new Error('the "on:" trigger surface is not a shape this judge recognises');
  }

  const out = [];
  const seen = new Set();
  for (const [event, config] of entries) {
    if (event.trim() === '') throw new Error('the "on:" trigger surface names an empty event');
    if (seen.has(event)) {
      throw new Error(`the "on:" trigger surface names the event "${event}" twice — one event with two configurations has no single answer`);
    }
    seen.add(event);
    out.push({event, filters: normalizeFilters(event, config)});
  }
  return out;
}

function sameValues(left, right) {
  const a = [...new Set(left)].sort();
  const b = [...new Set(right)].sort();
  return a.length === b.length && a.every((value, index) => value === b[index]);
}

/// 主判据。入参**显式注入**：`workflowText` 少了它，整条判据会退化成恒真。
///
/// 判分由**在册条目**驱动（W88：改前/改后都要真跑），且**不重复**报别的判据已报的东西——
/// 步集（W95）、执行配置（W100）、目标绑定（W101）的事一律不在这里出现。
export function workflowTriggerErrors({host, workflowText, registered = WORKFLOW_TRIGGERS} = {}) {
  if (typeof workflowText !== 'string' || workflowText === '') {
    // 缺输入 MUST 具名，MUST NOT 静默返回空错误——那会让判据在被削弱时保持沉默。
    return ['the CI workflow was not provided, so its trigger surface cannot be judged'];
  }
  if (!Array.isArray(registered) || registered.length === 0) {
    return ['the trigger registry is empty, so every trigger rule would be vacuously true'];
  }

  const errors = [];
  const declared = [];
  const seenEvents = new Set();
  for (const entry of registered) {
    if (entry?.host !== host) continue;
    // 条目层闭世界：字段集 MUST 封闭。**位置在读取面之前**（先问「这个条目对得上吗」，
    // 再问「它的取值合不合法」）。一条没人读的字段是一条**没有代价的声明**（见 `TRIGGER_FIELDS`）。
    for (const key of Object.keys(entry ?? {}).sort()) {
      if (!TRIGGER_FIELDS.includes(key)) {
        errors.push(
          `${host} carries the trigger field "${key}", which is not a trigger field — a field nobody reads is a declaration with no consequence`,
        );
      }
    }
    const event = entry?.event;
    if (typeof event !== 'string' || event === '') {
      errors.push(`${host} declares a trigger with no event name — an unnamed trigger names nothing`);
      continue;
    }
    // 事件名闭世界：域外 ⇒ 具名失败。MUST NOT 把未知事件当成「自动」——
    // 那样一个拼错的事件名会通过「至少一个自动触发」的断言（假阴性方向）。
    if (triggerKindOf(event) === null) {
      errors.push(
        `${host} declares the trigger "${event}", which is not a classified event — an event outside the closed set is a label, not a classification`,
      );
      continue;
    }
    if (seenEvents.has(event)) {
      errors.push(`${host} declares the trigger "${event}" twice — one event with two declarations has no single answer`);
    }
    seenEvents.add(event);
    // `filters` MUST 显式存在：「没有过滤器」也要说出来（「可选」就是判据的缝）。
    if (!isPlainObject(entry.filters)) {
      errors.push(
        `${host} trigger "${event}" declares no filter map — "this event carries no filter" has to be said out loud`,
      );
      continue;
    }
    const filters = {};
    let shaped = true;
    for (const [key, value] of Object.entries(entry.filters)) {
      if (!FILTER_KEYS.includes(key)) {
        errors.push(
          `${host} trigger "${event}" declares the filter "${key}", which is not a filter key — a key outside the closed set is a label, not a filter`,
        );
        shaped = false;
        continue;
      }
      if (!isNonEmptyStringList(value)) {
        errors.push(
          `${host} trigger "${event}" declares the filter "${key}" with no values — a filter without values filters nothing`,
        );
        shaped = false;
        continue;
      }
      filters[key] = [...value];
    }
    if (!shaped) continue;
    declared.push({event, kind: triggerKindOf(event), filters});
  }

  let produced;
  try {
    produced = workflowTriggersOf(workflowText);
  } catch (error) {
    // 不可解析 / `on:` 缺失 / `on:` 空 / 事件重名 / 配置形状域外 ⇒ 具名失败。
    // MUST NOT 静默降级——把「读不懂」当成「没问题」正是本模块存在的理由。
    return [...errors, `${host} trigger surface could not be read, so it cannot be judged: ${error.message}`];
  }

  // 解析侧：事件名闭世界 + 过滤器键形状闭集。
  for (const trigger of produced) {
    if (triggerKindOf(trigger.event) === null) {
      errors.push(
        `${host} is triggered by "${trigger.event}", which is not a classified event — an event outside the closed set is a label, not a classification`,
      );
    }
    for (const key of Object.keys(trigger.filters).sort()) {
      if (!FILTER_KEYS.includes(key)) {
        errors.push(
          `${host} trigger "${trigger.event}" applies the filter "${key}", which is not a filter key — a key outside the closed set is a label, not a filter`,
        );
      }
    }
  }

  // R3：**前提层**。两侧各断言一次，MUST NOT 合并——双向闭世界（R1）让它们**大多数情况**同真同假，
  // 但不是所有情况：把 `workflow_dispatch` 也补进在册表、同时删掉 `push`，R1 成立而解析侧仍须红。
  if (!declared.some(trigger => trigger.kind === 'automatic')) {
    errors.push(
      `${host} declares no automatically triggered event, so the rule that this workflow runs at all is vacuous`,
    );
  }
  if (!produced.some(trigger => triggerKindOf(trigger.event) === 'automatic')) {
    errors.push(
      `${host} has no event that triggers it automatically, so a push never reaches the gate — a gate that never runs is not a gate`,
    );
  }

  // R1：事件集双向闭世界。判定单元是**事件的身份**（事件名），MUST NOT 是它出现的位置。
  const declaredEvents = new Set(declared.map(trigger => trigger.event));
  const producedEvents = new Set(produced.map(trigger => trigger.event));
  for (const event of [...producedEvents].sort()) {
    if (!declaredEvents.has(event)) {
      errors.push(
        `${host} is triggered by "${event}", which nobody declares — an undeclared trigger is a way into the gate that nobody judges`,
      );
    }
  }
  for (const event of [...declaredEvents].sort()) {
    if (!producedEvents.has(event)) {
      errors.push(
        `${host} is declared to be triggered by "${event}", but it is not — a declared trigger that vanished means the gate may never run`,
      );
    }
  }

  // R2：过滤器双向对账，判定单元 = `(event, filter-key)`。这是**假阴性**方向必须堵的洞：
  // `push: {branches: [main]}` 与 `push: {branches: [nope]}` 的**事件集完全相同**（都只有 `push`），
  // 只做 R1 看不见 ⇒ 而 `branches` 恰好决定 push **是否真的会触发**。
  const unitsOf = triggers => {
    const map = new Map();
    for (const trigger of triggers) {
      for (const [key, values] of Object.entries(trigger.filters)) {
        map.set(filterKeyOf(trigger.event, key), {event: trigger.event, key, values: [...values]});
      }
    }
    return map;
  };
  const provided = unitsOf(produced);
  const requested = unitsOf(declared);
  for (const unit of [...new Set([...provided.keys(), ...requested.keys()])].sort()) {
    const applied = provided.get(unit);
    const claimed = requested.get(unit);
    if (applied === undefined) {
      errors.push(
        `${host} trigger "${claimed.event}" is declared with ${claimed.key}=${[...claimed.values].sort().join(', ')}, which the workflow does not apply — a declared filter with no effect is a claim, not a fact`,
      );
    } else if (claimed === undefined) {
      errors.push(
        `${host} trigger "${applied.event}" applies ${applied.key}=${[...applied.values].sort().join(', ')}, which nobody declares — a filter nobody declares can silently stop the gate from running`,
      );
    } else if (!sameValues(applied.values, claimed.values)) {
      errors.push(
        `${host} trigger "${applied.event}" declares ${applied.key}=${[...claimed.values].sort().join(', ')} but applies ${applied.key}=${[...applied.values].sort().join(', ')} — the declared filter and the applied one must be the same, because this key decides whether the event fires at all`,
      );
    }
  }

  return errors;
}
