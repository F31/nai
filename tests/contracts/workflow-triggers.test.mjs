import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { readFileSync } from 'node:fs';

import { readTrackedPaths, describeFailure } from '../../scripts/contracts/git-index.mjs';
import {
  TEST_REGISTRATION_SITES,
  LOCALLY_UNREACHABLE,
  declaredMarkerPaths,
  reachabilityErrors,
} from '../../scripts/contracts/test-reachability.mjs';
import { WORKFLOW_STEPS, workflowStepErrors } from '../../scripts/contracts/workflow-steps.mjs';
import { stepConfigErrors } from '../../scripts/contracts/step-config.mjs';
import { stepTargetErrors } from '../../scripts/contracts/step-target.mjs';
import {
  WORKFLOW_TRIGGERS,
  TRIGGER_FIELDS,
  TRIGGER_KINDS,
  AUTOMATIC_EVENTS,
  MANUAL_EVENTS,
  FILTER_KEYS,
  triggerKindOf,
  filterKeyOf,
  workflowTriggersOf,
  workflowTriggerErrors,
} from '../../scripts/contracts/workflow-triggers.mjs';

const HOST = '.github/workflows/foundation.yml';
const GATE = 'xtask/src/lib.rs';

/// 判据的输入**全部显式注入**：本模块自己不读磁盘（遍历工作区是 W75 的 F2 根因）。
async function load() {
  const index = await readTrackedPaths('.');
  assert.ok(index.ok, describeFailure(index));
  return {
    tracked: index.paths,
    workflowText: await readFile(HOST, 'utf8'),
    gateSources: new Map([[GATE, readFileSync(GATE, 'utf8')]]),
  };
}

const readHost = host => {
  try {
    return readFileSync(host, 'utf8');
  } catch {
    return null;
  }
};

/// 注入必须是**真注入**：改不到就断言失败（否则用例会假装通过）。
const patch = (text, from, to) => {
  const next = text.replace(from, to);
  assert.notEqual(next, text, `the injection did not apply: ${JSON.stringify(from)}`);
  return next;
};

const named = (errors, ...needles) =>
  errors.some(error => needles.every(needle => error.includes(needle)));

/// 从出厂文件里**去掉 `pull_request` 触发** —— 覆盖 `on:` 的两种常见写法。
///
/// ⚠️ 这是**前提构造**，不是判据。W102 探针 P-A8 实测：把 `on:` **等价改写**成列表写法之后，
/// 「只改触发面 ⇒ 另外三个判据沉默」的两条正交性用例**因锚点匹配不到而红**——红的不是判据，
/// 是**前提没了**（W101 §2.2 的同型教训；铁律一：断言对、前提错 ⇒ **修前提，别改断言**）。
/// 第三种写法出现时它**具名失败**，MUST NOT 静默变成一个 no-op。
function withoutPullRequest(text) {
  const candidates = [
    ['on:\n  push:\n  pull_request:\n', 'on:\n  push:\n'],
    ['on: [push, pull_request]\n', 'on: [push]\n'],
  ];
  for (const [from, to] of candidates) if (text.includes(from)) return patch(text, from, to);
  assert.fail('the shipped "on:" block must be one of the known spellings, or this premise is vacuous');
}

// ---------------------------------------------------------------------------
// 合成世界：两事件，其中一条**带过滤器**（否则 R2 的对账对空集恒真）
// ---------------------------------------------------------------------------

const SYNTHETIC_WORKFLOW = [
  'name: Synthetic',
  'on:',
  '  push:',
  '    branches: [main]',
  '  pull_request:',
  'jobs:',
  '  j:',
  '    runs-on: ubuntu-24.04',
  '    steps:',
  '      - run: echo hi',
  '',
].join('\n');

const SYNTHETIC_REGISTRY = [
  {host: HOST, event: 'push', filters: {branches: ['main']}},
  {host: HOST, event: 'pull_request', filters: {}},
];

const synthetic = (workflowText = SYNTHETIC_WORKFLOW, registered = SYNTHETIC_REGISTRY) =>
  workflowTriggerErrors({host: HOST, workflowText, registered});

const registryOf = mutation => {
  const copy = structuredClone(SYNTHETIC_REGISTRY);
  mutation(copy);
  return copy;
};

const entry = (registry, event) => registry.find(item => item.event === event);

// ---------------------------------------------------------------------------
// F 级：合成世界
// ---------------------------------------------------------------------------

test('F0 the synthetic world is green and each of its rules is actually exercised', () => {
  assert.deepEqual(synthetic(), [], 'the synthetic world must be green');
  // 非空洞 ①：三个闭集都非空。
  assert.ok(TRIGGER_KINDS.length > 0, 'the kind closure must not be empty');
  assert.ok(AUTOMATIC_EVENTS.length > 0 && MANUAL_EVENTS.length > 0, 'both event tables must be non-empty');
  assert.ok(FILTER_KEYS.length > 0, 'the filter-key closure must not be empty');
  // 非空洞 ②：两个事件表**不相交**（否则「自动 vs 手动」没有单一回答），且并集**全覆盖**类别闭集。
  const classified = new Map([...AUTOMATIC_EVENTS, ...MANUAL_EVENTS].map(event => [event, triggerKindOf(event)]));
  assert.deepEqual(
    AUTOMATIC_EVENTS.filter(event => MANUAL_EVENTS.includes(event)),
    [],
    'the two event tables must be disjoint',
  );
  assert.deepEqual(
    [...new Set(classified.values())].sort(),
    [...TRIGGER_KINDS].sort(),
    'every kind in the closure must have a live event, otherwise one derivation branch is vacuous',
  );
  for (const [event, kind] of classified) {
    assert.notEqual(kind, null, `the event ${event} must be classified`);
  }
  // 非空洞 ③：合成世界**必须**既有一个过滤器单元、又有一次「裸事件」——否则 R2 与 R1 各缺一半。
  const parsed = workflowTriggersOf(SYNTHETIC_WORKFLOW);
  assert.ok(
    parsed.some(trigger => Object.keys(trigger.filters).length > 0),
    'the synthetic world must apply at least one filter, or the reconciliation is vacuous',
  );
  assert.ok(
    parsed.some(trigger => Object.keys(trigger.filters).length === 0),
    'the synthetic world must also carry a bare event, or "no filter" is never exercised',
  );
  // 非空洞 ④：合成注册表**不写** `kind` —— 类别由事件表推导，MUST NOT 再具名一次。
  for (const item of WORKFLOW_TRIGGERS) {
    assert.ok(!('kind' in item), 'the kind must be derived from the event table, not stored on the entry');
    assert.ok('filters' in item, 'the filter map must be declared out loud, even when empty');
    // 条目字段闭世界：**逐键比对**，MUST NOT 只钉「有没有 kind」——一个错拼的键（`filter`）
    // 同样是一条没人读的声明。
    assert.deepEqual(
      Object.keys(item).sort(),
      [...TRIGGER_FIELDS].sort(),
      'a trigger entry must carry exactly the declared fields — a field nobody reads has no consequence',
    );
  }
});

test('F0b a trigger field outside the closed set is named', () => {
  // P-B5 的形状：手写一个 `kind` —— 它由事件表推导，判据不读它，于是它是一条**没有代价的声明**。
  const handWrittenKind = registryOf(copy => {
    entry(copy, 'push').kind = 'automatic';
  });
  assert.ok(
    named(synthetic(SYNTHETIC_WORKFLOW, handWrittenKind), 'trigger field "kind"', 'no consequence'),
    'a hand-written kind must be named — a field nobody reads must not sit there quietly',
  );
  // 错拼的键同理（`filter` 少了 `s`）：它看起来像 `filters`，但没有任何东西读它。
  const misspelled = registryOf(copy => {
    entry(copy, 'push').filter = entry(copy, 'push').filters;
    delete entry(copy, 'push').filters;
  });
  assert.ok(
    named(synthetic(SYNTHETIC_WORKFLOW, misspelled), 'trigger field "filter"'),
    'a misspelled field must be named rather than silently ignored',
  );
});

test('F1 an event removed from the trigger surface is named', () => {
  const withoutPr = patch(SYNTHETIC_WORKFLOW, '  pull_request:\n', '');
  const errors = synthetic(withoutPr);
  assert.ok(
    named(errors, '"pull_request"', 'declared to be triggered', 'but it is not'),
    `a vanished trigger must be named, saw ${JSON.stringify(errors)}`,
  );
});

test('F2 a workflow with no automatic trigger is named, on both sides', () => {
  // 整条 `on:` 缩成仅 `workflow_dispatch`，在册表也只留它 —— P-D 的形状。
  const manualOnly = ['on:', '  workflow_dispatch:', 'jobs: {}', ''].join('\n');
  const registry = [{host: HOST, event: 'workflow_dispatch', filters: {}}];
  const errors = synthetic(manualOnly, registry);
  assert.ok(
    named(errors, 'declares no automatically triggered event'),
    `the registry side must be named, saw ${JSON.stringify(errors)}`,
  );
  assert.ok(
    named(errors, 'no event that triggers it automatically'),
    'the workflow side must be named too — bidirectional equality makes the two sides usually agree, but not always',
  );
  // 反向：把 `push` 补回在册表、`on:` 里也补回 ⇒ 不再红。
  const restored = ['on:', '  push:', '  workflow_dispatch:', 'jobs: {}', ''].join('\n');
  assert.deepEqual(
    synthetic(restored, [registry[0], {host: HOST, event: 'push', filters: {}}]),
    [],
    'an automatic event must clear both sides of the rule',
  );
});

test('F3 a trigger restriction nobody declares is named, with both value sets', () => {
  // 事件集一个字没变（都只有 push / pull_request），但 push 被限定到不存在的分支 —— P-E 的形状。
  const restricted = patch(SYNTHETIC_WORKFLOW, '    branches: [main]', '    branches: [nope]');
  const errors = synthetic(restricted);
  assert.ok(
    named(errors, '"push"', 'declares branches=main', 'applies branches=nope'),
    `a rewritten filter value must be named, saw ${JSON.stringify(errors)}`,
  );
  // 新增一个**没人声明**的过滤器键。
  const extra = patch(SYNTHETIC_WORKFLOW, '  pull_request:\n', '  pull_request:\n    paths: ["src/**"]\n');
  assert.ok(
    named(synthetic(extra), '"pull_request"', 'applies paths=src/**', 'nobody declares'),
    'a filter applied but not declared must be named',
  );
});

test('F4 a declared filter that is not applied is named', () => {
  const relaxed = registryOf(copy => {
    entry(copy, 'push').filters = {branches: ['main'], tags: ['v*']};
  });
  const errors = synthetic(SYNTHETIC_WORKFLOW, relaxed);
  assert.ok(
    named(errors, '"push"', 'declared with tags=v*', 'does not apply'),
    `a declaration with no effect must be named, saw ${JSON.stringify(errors)}`,
  );
});

test('F5 a filter key outside the closed set is named on both sides', () => {
  // 解析侧：`branches` 写错成 `branch`。
  const typo = patch(SYNTHETIC_WORKFLOW, '    branches: [main]', '    branch: [main]');
  assert.ok(
    named(synthetic(typo), 'applies the filter "branch"', 'not a filter key'),
    'a misspelled filter key on the workflow side must be named',
  );
  // 在册侧：同一个笔误。
  const declaredTypo = registryOf(copy => {
    entry(copy, 'push').filters = {branch: ['main']};
  });
  assert.ok(
    named(synthetic(SYNTHETIC_WORKFLOW, declaredTypo), 'declares the filter "branch"', 'not a filter key'),
    'a misspelled filter key on the registry side must be named',
  );
  // 取值形状域外：空数组。
  const emptyValues = registryOf(copy => {
    entry(copy, 'push').filters = {branches: []};
  });
  assert.ok(
    named(synthetic(SYNTHETIC_WORKFLOW, emptyValues), 'branches', 'with no values'),
    'a filter without values must be named',
  );
  // `filters` 字段缺失 —— 「没有过滤器」也必须说出来。
  const missing = registryOf(copy => {
    delete entry(copy, 'pull_request').filters;
  });
  assert.ok(
    named(synthetic(SYNTHETIC_WORKFLOW, missing), '"pull_request"', 'declares no filter map'),
    'a missing filter map must be named, not silently treated as empty',
  );
});

test('F6 an event outside the closed set is named on both sides', () => {
  // 在册侧：一个拼错的事件名 MUST NOT 被当成「自动触发」。
  const typo = registryOf(copy => {
    entry(copy, 'push').event = 'pus';
  });
  assert.ok(
    named(synthetic(SYNTHETIC_WORKFLOW, typo), '"pus"', 'not a classified event'),
    'an unclassified event on the registry side must be named',
  );
  // 解析侧同理。
  const workflowTypo = patch(SYNTHETIC_WORKFLOW, '  push:', '  pus:');
  assert.ok(
    named(synthetic(workflowTypo), 'triggered by "pus"', 'not a classified event'),
    'an unclassified event on the workflow side must be named',
  );
  // 在册侧：同一个事件声明两次。
  const twice = registryOf(copy => {
    copy.push({host: HOST, event: 'push', filters: {branches: ['main']}});
  });
  assert.ok(
    named(synthetic(SYNTHETIC_WORKFLOW, twice), 'declares the trigger "push" twice'),
    'a doubly declared event must be named',
  );
});

test('F7 a missing input, an empty registry, or an unreadable trigger surface is named', () => {
  for (const input of [undefined, '', 42]) {
    assert.ok(
      named(workflowTriggerErrors({host: HOST, workflowText: input}), 'was not provided'),
      'a missing workflow must be named',
    );
  }
  assert.ok(
    named(workflowTriggerErrors({host: HOST, workflowText: SYNTHETIC_WORKFLOW, registered: []}), 'empty'),
    'an empty registry must be named — otherwise every rule is vacuously true',
  );
  const unreadable = {
    'no `on:` key': 'name: x\njobs: {}\n',
    '`on:` is null': 'on:\njobs: {}\n',
    '`on:` is an empty map': 'on: {}\njobs: {}\n',
    '`on:` is an empty list': 'on: []\njobs: {}\n',
    'the event is duplicated': 'on: [push, push]\njobs: {}\n',
    'the YAML is broken': 'jobs: [1, 2\n',
    'the config shape is foreign': 'on:\n  workflow_dispatch:\n    inputs:\n      a:\n        type: string\njobs: {}\n',
  };
  for (const [label, text] of Object.entries(unreadable)) {
    assert.ok(
      named(synthetic(text), 'trigger surface could not be read'),
      `${label} must be named rather than silently passing`,
    );
  }
});

test('F8 a legitimate registration is not reported', () => {
  // 多事件、多过滤器、多取值；声明顺序不是声明的一部分。
  const rich = ['on:', '  push:', '    branches: [release, main]', '    paths: ["src/**"]', '  pull_request:', 'jobs: {}', ''].join('\n');
  const declared = [
    {host: HOST, event: 'push', filters: {branches: ['release', 'main'], paths: ['src/**']}},
    {host: HOST, event: 'pull_request', filters: {}},
  ];
  assert.deepEqual(synthetic(rich, declared), [], 'a fully declared trigger surface must not be reported');
  const reordered = [
    {host: HOST, event: 'push', filters: {paths: ['src/**'], branches: ['main', 'release']}},
    {host: HOST, event: 'pull_request', filters: {}},
  ];
  assert.deepEqual(synthetic(rich, reordered), [], 'the order of events and of filter values must not matter');
});

// ---------------------------------------------------------------------------
// 解析规则本身
// ---------------------------------------------------------------------------

test('the parser normalises the three ways of writing `on:`', () => {
  // 三种写法都归一化到**同一个形状**；列表与映射两种承载同一组事件，必须产出**逐字相同**的结果。
  assert.deepEqual(
    workflowTriggersOf('on: push\njobs: {}\n'),
    [{event: 'push', filters: {}}],
    'the bare string form must normalise to a single trigger',
  );
  const both = [{event: 'push', filters: {}}, {event: 'pull_request', filters: {}}];
  assert.deepEqual(workflowTriggersOf('on: [push, pull_request]\njobs: {}\n'), both, 'the list form');
  assert.deepEqual(workflowTriggersOf('on:\n  push:\n  pull_request:\njobs: {}\n'), both, 'the mapping form');
  assert.deepEqual(
    workflowTriggersOf('on:\n  push:\n    branches: [main, release]\njobs: {}\n'),
    [{event: 'push', filters: {branches: ['main', 'release']}}],
    'filters must be carried through unchanged',
  );
  // 实测：`on` 是**字符串键**，不是布尔 `true`（`yaml` v2 默认 YAML 1.2 core schema）。
  assert.deepEqual(workflowTriggersOf('on:\n  push:\n').map(t => t.event), ['push']);
});

test('a trigger is identified by the (event, filter-key) pair, not by the bare key', () => {
  assert.notEqual(filterKeyOf('push', 'branches'), filterKeyOf('pull_request', 'branches'));
  assert.equal(filterKeyOf('push', 'branches'), filterKeyOf('push', 'branches'));
  // 实测：同一个键名在不同事件上互换 ⇒ 必须被看见（裸键名比较看不见）。
  const swapped = ['on:', '  push:', '    branches: [main]', '  pull_request:', '    branches: [release]', 'jobs: {}', ''].join('\n');
  const declared = [
    {host: HOST, event: 'push', filters: {branches: ['release']}},
    {host: HOST, event: 'pull_request', filters: {branches: ['main']}},
  ];
  const errors = synthetic(swapped, declared);
  assert.ok(named(errors, '"push"', 'branches=release', 'applies branches=main'), 'the push unit must be named');
  assert.ok(
    named(errors, '"pull_request"', 'branches=main', 'applies branches=release'),
    'the pull_request unit must be named too — same key name, different object',
  );
});

// ---------------------------------------------------------------------------
// 真实仓库
// ---------------------------------------------------------------------------

test('the shipped workflow pays for its trigger surface', async () => {
  const { workflowText } = await load();
  assert.deepEqual(workflowTriggerErrors({host: HOST, workflowText}), [], 'the shipped trigger surface must be paid for');
  // 前提的另一半：空数组不能只是「什么都没解析出来」。
  const parsed = workflowTriggersOf(workflowText);
  assert.ok(parsed.length > 0, 'the workflow must carry at least one trigger');
  assert.ok(
    parsed.some(trigger => triggerKindOf(trigger.event) === 'automatic'),
    'the shipped workflow must have an automatic trigger, or the gate never runs',
  );
  // 在册表与解析出的**事件集合**一致（身份层；那是本判据 R1 的一半，这里把前提写清）。
  assert.deepEqual(
    parsed.map(trigger => trigger.event).sort(),
    WORKFLOW_TRIGGERS.map(item => item.event).sort(),
    'the shipped registry must name exactly the events the workflow carries',
  );
  // ⚠️ 过滤器单元的**实例数**不在这里钉（W92 的教训）。
  // 实测（W102）：当前 workflow 的 `on:` **没有任何过滤器** ⇒ R2 的单元集合在真实仓库上是空的，
  // 它的非空洞由合成世界（F0）与注入用例（F3/F4）证明。这是「判据非空洞」与「在册数据恰好简单」
  // 两件事——MUST NOT 靠「反正现在没过滤器」把这条判据当成装饰。
});

test('the trigger judge is orthogonal to W95, W100 and W101', async () => {
  const { workflowText, gateSources } = await load();
  // 只改触发面。步集、执行配置、目标绑定一个字都没动 ⇒ 它们必须全部沉默。
  const dropPr = withoutPullRequest(workflowText);
  assert.deepEqual(
    workflowStepErrors(HOST, dropPr, WORKFLOW_STEPS, gateSources),
    [],
    'the step-set judge must not read the trigger surface',
  );
  assert.deepEqual(
    stepConfigErrors({host: HOST, workflowText: dropPr}),
    [],
    'the execution-config judge must not read the trigger surface',
  );
  assert.deepEqual(
    stepTargetErrors({host: HOST, workflowText: dropPr}),
    [],
    'the target-binding judge must not read the trigger surface',
  );
  assert.ok(
    named(workflowTriggerErrors({host: HOST, workflowText: dropPr}), '"pull_request"', 'but it is not'),
    'the trigger judge must be the one that notices a dropped trigger',
  );
  // 反向：只改目标绑定 / 步名 ⇒ 触发面判据必须沉默。
  const retargeted = patch(workflowText, 'run: cargo xtask test --suite provider', 'run: cargo xtask test --suite storage');
  assert.deepEqual(
    workflowTriggerErrors({host: HOST, workflowText: retargeted}),
    [],
    'the trigger judge must not restate the target-binding verdict',
  );
  const renamed = patch(workflowText, '      - name: Kernel registration and binding\n', '      - name: Kernel registration and naming\n');
  assert.deepEqual(
    workflowTriggerErrors({host: HOST, workflowText: renamed}),
    [],
    'the trigger judge must not restate the step-set verdict either',
  );
});

test('the judge is orthogonal to W84, which reads the tracked set and not the workflow body', async () => {
  const { tracked, workflowText } = await load();
  // 铁律一：本用例的断言依赖「本轮新测试文件在册」，而提交前它还没进 `.git/index`
  // ⇒ 由**构造**使前提成立（并集）。**构造 MUST 遍历站点表自己的键**（W101 订正），
  // MUST NOT 只补本轮那一个文件。
  const withNew = new Set([...tracked, ...declaredMarkerPaths()]);
  const dropPr = withoutPullRequest(workflowText);
  assert.deepEqual(
    reachabilityErrors({
      tracked: withNew,
      sites: TEST_REGISTRATION_SITES,
      unreachable: [...LOCALLY_UNREACHABLE],
      readHost,
    }),
    [],
    'the reachability judge must not read the workflow body',
  );
  assert.ok(
    named(workflowTriggerErrors({host: HOST, workflowText: dropPr}), '"pull_request"'),
    'the trigger judge must be the one that notices a dropped trigger',
  );
});
