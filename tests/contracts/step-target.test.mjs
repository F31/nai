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
import { WORKFLOW_STEPS, STEP_KINDS, CI_ONLY_REASON_PAYMENTS, workflowStepsOf, workflowStepErrors } from '../../scripts/contracts/workflow-steps.mjs';
import { ciXtaskCommands } from '../../scripts/contracts/gate-coverage.mjs';
import { TARGET_SHAPES, targetsOf, targetKey, stepTargetErrors } from '../../scripts/contracts/step-target.mjs';

const HOST = '.github/workflows/foundation.yml';
const GATE = 'xtask/src/lib.rs';

/// 凭据文件的**内容**从兑付表自己派生（不写第二份文件清单）。读不到的不抛——
/// 让判据具名报告缺输入，而不是让 `ENOENT` 把「判据该报的缺陷」伪装成「测试自己坏了」。
const paymentSources = payments => {
  const sources = new Map();
  for (const file of new Set(Object.values(payments).flatMap(p => p.evidence.map(i => i.file)))) {
    try {
      sources.set(file, readFileSync(file, 'utf8'));
    } catch {
      // 判据会具名报告它。
    }
  }
  return sources;
};

/// 判据的输入**全部显式注入**：本模块自己不读磁盘（遍历工作区是 W75 的 F2 根因）。
async function load() {
  const index = await readTrackedPaths('.');
  assert.ok(index.ok, describeFailure(index));
  return {
    tracked: index.paths,
    workflowText: await readFile(HOST, 'utf8'),
    gateSources: new Map([[GATE, readFileSync(GATE, 'utf8')]]),
    // `ci-only` 的成因凭据（W108）：两个必填输入从**兑付表自己派生**——表改了输入自动跟上，
    // 不写第二份文件清单。读不到的不抛：让**判据**去具名报告缺输入，而不是让 `ENOENT` 掩盖它。
    reasonInput: {sources: paymentSources(CI_ONLY_REASON_PAYMENTS), tracked: index.paths},
  };
}

/// `reachabilityErrors` 要一个**同步**读盘函数。
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

// ---------------------------------------------------------------------------
// 合成世界：七步覆盖 1 个 infra、5 个判定步（**三个形状都有**）、1 个 diagnostic
// ---------------------------------------------------------------------------

const SYNTHETIC_WORKFLOW = [
  'name: Synthetic',
  'on:',
  '  push:',
  'jobs:',
  '  j:',
  '    runs-on: ubuntu-24.04',
  '    steps:',
  '      - name: Setup',
  '        run: pnpm install --frozen-lockfile',
  '      - name: Gate',
  '        shell: bash',
  '        run: |',
  '          set -o pipefail',
  '          cargo xtask check --profile full 2>&1 | tee log',
  '      - name: Suites',
  '        run: cargo xtask test --suite kernel',
  '      - name: Compact',
  '        run: node --test tests/a.test.mjs tests/b.test.mjs',
  '      - name: Spec',
  '        run: pnpm spec:validate',
  '      - name: OpenSpec',
  '        run: pnpm exec openspec validate --archived --no-interactive',
  '      - name: Diagnose',
  '        if: failure()',
  '        run: node scripts/ci-diagnose.mjs log',
  '        continue-on-error: true',
  '',
].join('\n');

const SYNTHETIC_REGISTRY = [
  {host: HOST, identity: 'Setup', kind: 'infra'},
  {
    host: HOST,
    identity: 'Gate',
    kind: 'local',
    localEvidence: {file: GATE, marker: 'gate'},
    target: [{shape: 'xtask', value: 'check --profile full'}],
  },
  {
    host: HOST,
    identity: 'Suites',
    kind: 'local',
    localEvidence: {file: GATE, marker: 'suite'},
    target: [{shape: 'xtask', value: 'test --suite kernel'}],
  },
  {
    host: HOST,
    identity: 'Compact',
    kind: 'ci-only',
    reason: 'compiled-artifact',
    target: [{shape: 'node-test', value: '--test tests/a.test.mjs tests/b.test.mjs'}],
  },
  {
    host: HOST,
    identity: 'Spec',
    kind: 'ci-only',
    reason: 'not-in-profile',
    target: [{shape: 'pnpm', value: 'spec:validate'}],
  },
  {
    host: HOST,
    identity: 'OpenSpec',
    kind: 'ci-only',
    reason: 'not-in-profile',
    target: [{shape: 'pnpm', value: 'exec openspec validate --archived --no-interactive'}],
  },
  {host: HOST, identity: 'Diagnose', kind: 'diagnostic'},
];

const synthetic = (workflowText = SYNTHETIC_WORKFLOW, registered = SYNTHETIC_REGISTRY) =>
  stepTargetErrors({host: HOST, workflowText, registered});

const registryOf = mutation => {
  const copy = structuredClone(SYNTHETIC_REGISTRY);
  mutation(copy);
  return copy;
};

const entry = (registry, identity) => registry.find(item => item.identity === identity);

// ---------------------------------------------------------------------------
// F 级：合成世界
// ---------------------------------------------------------------------------

test('F0 the synthetic world is green and each of its steps is actually judged', () => {
  assert.deepEqual(synthetic(), [], 'the synthetic world must be green');
  // 非空洞：三个目标形状都在合成世界里出现过，否则某一条解析规则是凭空的。
  const shapes = new Set(workflowStepsOf(SYNTHETIC_WORKFLOW).flatMap(step => targetsOf(step.run).map(target => target.shape)));
  assert.deepEqual(
    [...shapes].sort(),
    [...TARGET_SHAPES].sort(),
    'the synthetic world must exercise every target shape, otherwise a parse rule is vacuous',
  );
  // 非空洞：`infra` 与 `diagnostic` 两类都必须有实例，否则 R2 的两个分支对空集恒真。
  const kinds = new Set(SYNTHETIC_REGISTRY.map(item => item.kind));
  for (const kind of ['infra', 'diagnostic', 'local', 'ci-only']) {
    assert.ok(kinds.has(kind), `the synthetic world must exercise the ${kind} kind`);
  }
});

test('F1 a target swapped to another suite is named', () => {
  const swapped = patch(SYNTHETIC_WORKFLOW, 'run: cargo xtask test --suite kernel', 'run: cargo xtask test --suite hosts');
  const errors = synthetic(swapped);
  assert.ok(
    named(errors, '"Suites"', 'test --suite hosts', 'does not declare'),
    `a swapped target must be named, saw ${JSON.stringify(errors)}`,
  );
  assert.ok(
    named(errors, '"Suites"', 'test --suite kernel', 'does not run'),
    'the vanished target must be named too — both directions',
  );
});

test('F2 a step whose whole judgement was replaced is named', () => {
  // 「像不像一条判定」与「跑的是哪个目标」是两件事：这条命令仍然是一条合法判定命令。
  const replaced = patch(SYNTHETIC_WORKFLOW, 'run: cargo xtask test --suite kernel', 'run: cargo xtask check --secrets');
  const errors = synthetic(replaced);
  assert.ok(named(errors, '"Suites"', 'xtask:check --secrets'), 'the new target must be named');
  assert.ok(named(errors, '"Suites"', 'xtask:test --suite kernel'), 'the declared target must be named as unmet');
});

test('F3 a declared target with no command is named', () => {
  const emptied = patch(SYNTHETIC_WORKFLOW, '      - name: Suites\n        run: cargo xtask test --suite kernel\n', '      - name: Suites\n        run: echo "no judgement here"\n');
  const errors = synthetic(emptied);
  assert.ok(
    named(errors, '"Suites"', 'yields none'),
    `a declaration with no command must be named, saw ${JSON.stringify(errors)}`,
  );
});

test('F4 a step that declares no target is named', () => {
  const bare = registryOf(copy => {
    delete entry(copy, 'Suites').target;
  });
  assert.ok(named(synthetic(SYNTHETIC_WORKFLOW, bare), '"Suites"', 'names no target'), 'a judging step without a target must be named');
});

test('F5 a judgement hidden in an infrastructure step is named, in both directions', () => {
  // 注入一：`infra` 的 `run` 里塞一条判定（W95 只剥首行、只认 5 个正则，本条是整段解析）。
  const hidden = patch(SYNTHETIC_WORKFLOW, 'run: pnpm install --frozen-lockfile', 'run: node --test tests/sneaky.test.mjs');
  assert.ok(
    named(synthetic(hidden), '"Setup"', 'back door'),
    'a judgement placed in an infra step must be named',
  );
  // 注入二：`diagnostic` 的 `run` 里塞一条判定。
  const sneaky = patch(SYNTHETIC_WORKFLOW, 'run: node scripts/ci-diagnose.mjs log', 'run: cargo xtask check --registry');
  assert.ok(named(synthetic(sneaky), '"Diagnose"', 'back door'), 'a judgement placed in a diagnostic step must be named');
  // 注入三：`infra` 条目**声明**了目标。
  const declared = registryOf(copy => {
    entry(copy, 'Setup').target = [{shape: 'pnpm', value: 'install --frozen-lockfile'}];
  });
  assert.ok(named(synthetic(SYNTHETIC_WORKFLOW, declared), '"Setup"', 'target belongs to a step that judges'), 'an infra entry must not declare a target');
});

test('F6 a target shape outside the closed set, or an empty value, is named', () => {
  const foreign = registryOf(copy => {
    entry(copy, 'Gate').target = [{shape: 'shell', value: 'check --profile full'}];
  });
  assert.ok(named(synthetic(SYNTHETIC_WORKFLOW, foreign), 'shell', 'not a target shape'), 'an unknown shape must be named');
  const blank = registryOf(copy => {
    entry(copy, 'Gate').target = [{shape: 'xtask', value: ''}];
  });
  assert.ok(named(synthetic(SYNTHETIC_WORKFLOW, blank), '"Gate"', 'with no value'), 'a shape without a value must be named');
});

test('F7 a missing input or an empty registry is named, never skipped', () => {
  for (const input of [undefined, '', 42]) {
    assert.ok(
      named(stepTargetErrors({host: HOST, workflowText: input}), 'was not provided'),
      'a missing workflow must be named',
    );
  }
  assert.ok(
    named(stepTargetErrors({host: HOST, workflowText: SYNTHETIC_WORKFLOW, registered: []}), 'empty'),
    'an empty registry must be named — otherwise every rule is vacuously true',
  );
});

test('F8 an unparseable workflow is named rather than silently passing', () => {
  const broken = stepTargetErrors({host: HOST, workflowText: 'jobs: [1, 2', registered: SYNTHETIC_REGISTRY});
  assert.ok(named(broken, 'could not be parsed'), `an unparseable workflow must be named, saw ${JSON.stringify(broken)}`);
});

test('F9 a legitimate registration is not reported', () => {
  // 一个步可以有多条命令、多个目标 —— 全部声明齐了就不红。
  const two = patch(SYNTHETIC_WORKFLOW, 'run: cargo xtask test --suite kernel', 'run: cargo xtask test --suite kernel && cargo xtask schema --check');
  const declared = registryOf(copy => {
    entry(copy, 'Suites').target = [
      {shape: 'xtask', value: 'test --suite kernel'},
      {shape: 'xtask', value: 'schema --check'},
    ];
  });
  assert.deepEqual(synthetic(two, declared), [], 'a fully declared multi-command step must not be reported');
  // 声明顺序不是声明的一部分。
  const reordered = registryOf(copy => {
    entry(copy, 'Suites').target = [
      {shape: 'xtask', value: 'schema --check'},
      {shape: 'xtask', value: 'test --suite kernel'},
    ];
  });
  assert.deepEqual(synthetic(two, reordered), [], 'the order of declared targets must not matter');
  // 一个步可以声明**不止一个形状**。
  const mixed = patch(SYNTHETIC_WORKFLOW, 'run: pnpm spec:validate', 'run: pnpm spec:validate && node --test tests/c.test.mjs');
  const mixedRegistry = registryOf(copy => {
    entry(copy, 'Spec').target = [
      {shape: 'pnpm', value: 'spec:validate'},
      {shape: 'node-test', value: '--test tests/c.test.mjs'},
    ];
  });
  assert.deepEqual(synthetic(mixed, mixedRegistry), [], 'a step may pay for more than one shape');
});

// ---------------------------------------------------------------------------
// 解析规则本身
// ---------------------------------------------------------------------------

test('the parser reads the step\'s own run, not the whole workflow', () => {
  // 三形状的解析规则各自在合成世界里有实例（F0 已证）；这里钉它们的**归一化**形状。
  assert.deepEqual(targetsOf('cargo xtask check --profile full 2>&1 | tee log'), [
    {shape: 'xtask', value: 'check --profile full'},
  ]);
  assert.deepEqual(targetsOf('node --test a.test.mjs   b.test.mjs'), [
    {shape: 'node-test', value: '--test a.test.mjs b.test.mjs'},
  ]);
  assert.deepEqual(targetsOf('node --experimental-strip-types --test tests/ui/*.test.mjs'), [
    {shape: 'node-test', value: '--experimental-strip-types --test tests/ui/*.test.mjs'},
  ]);
  assert.deepEqual(targetsOf('pnpm exec openspec validate --archived --no-interactive'), [
    {shape: 'pnpm', value: 'exec openspec validate --archived --no-interactive'},
  ]);
  // `pnpm install` 是环境准备，不是目标——与 W95 的 `(?!install\b)` 同一条纪律。
  assert.deepEqual(targetsOf('pnpm install --frozen-lockfile'), []);
  // 空 / 非字符串 ⇒ 空列表（不是抛，也不是「一个空目标」）。
  assert.deepEqual(targetsOf(''), []);
  assert.deepEqual(targetsOf(undefined), []);
  assert.deepEqual(targetsOf('   '), []);
  // 一条 run 里的**第二、第三条**命令都会被解析出来（多出来的由「解析出而没声明」具名）。
  assert.deepEqual(targetsOf('cargo xtask test --suite kernel\ncargo xtask schema --check'), [
    {shape: 'xtask', value: 'test --suite kernel'},
    {shape: 'xtask', value: 'schema --check'},
  ]);
  // ⚠️ 假阴性守卫：`&&` / `||` / `;` 串起来的第二条命令**真的会被 shell 执行**，
  // 因此 MUST 也被解析出来——否则第二条判定静默逃出目标绑定（危险方向）。
  assert.deepEqual(targetsOf('cargo xtask test --suite kernel && cargo xtask check --secrets'), [
    {shape: 'xtask', value: 'test --suite kernel'},
    {shape: 'xtask', value: 'check --secrets'},
  ]);
  assert.deepEqual(targetsOf('pnpm spec:validate; node --test tests/d.test.mjs'), [
    {shape: 'pnpm', value: 'spec:validate'},
    {shape: 'node-test', value: '--test tests/d.test.mjs'},
  ]);
  assert.deepEqual(targetsOf('cargo xtask schema --check || cargo xtask schema'), [
    {shape: 'xtask', value: 'schema --check'},
    {shape: 'xtask', value: 'schema'},
  ]);
  // 纯环境准备的两段仍然一个目标都不产出。
  assert.deepEqual(targetsOf('sudo apt-get update && sudo apt-get install -y libprotobuf-dev'), []);
});

test('a target is identified by its (shape, value), not by its value', () => {
  assert.notEqual(targetKey({shape: 'xtask', value: 'check --secrets'}), targetKey({shape: 'pnpm', value: 'check --secrets'}));
  assert.equal(targetKey({shape: 'xtask', value: 'a b'}), targetKey({shape: 'xtask', value: 'a b'}));
  assert.ok(TARGET_SHAPES.length > 0, 'the shape closure must not be empty');
  assert.ok(STEP_KINDS.includes('local') && STEP_KINDS.includes('ci-only'));
});

// ---------------------------------------------------------------------------
// 真实仓库
// ---------------------------------------------------------------------------

test('every shipped judging step pays for the target it runs', async () => {
  const { workflowText } = await load();
  assert.deepEqual(stepTargetErrors({host: HOST, workflowText}), [], 'every shipped step target must be paid for');
  // 前提的另一半：空数组不能只是「什么都没解析出来」。
  const steps = workflowStepsOf(workflowText);
  assert.equal(steps.length, WORKFLOW_STEPS.length, 'every registered step must have a parsed step');
  const declared = WORKFLOW_STEPS.filter(item => item.target !== undefined);
  assert.ok(declared.length > 0, 'the registry must declare targets, or the judge is vacuous');
  assert.ok(
    WORKFLOW_STEPS.length - declared.length > 0,
    'the registry must also carry steps that legitimately declare no target',
  );
  const produced = new Map(steps.map(step => [step.identity, targetsOf(step.run).map(targetKey).sort()]));
  // 前提：在册与解析出的**身份**集合一致。那是 **W95 的判据**，本判据**不重复报**——
  // 所以这条前提若被违反（例如探针改掉一个步名），先在这里具名，而不是让下面的逐条比较
  // 以 `undefined` 的形式**假装成「目标对不上」**（W101 实测：C1 改步名时 W101 判据返回 `[]`，
  // 这条断言是唯一会红的——把它写清，红的就是它）。
  const missing = WORKFLOW_STEPS.filter(item => !produced.has(item.identity)).map(item => item.identity);
  assert.deepEqual(
    missing,
    [],
    'the shipped registry must match the workflow by identity before targets can be keyed on it — that consistency is W95 business, not this judge\'s',
  );
  for (const item of declared) {
    assert.deepEqual(produced.get(item.identity), item.target.map(targetKey).sort(), `${item.identity} must run exactly what it declares`);
  }
  // ⚠️ 每个形状的**实例数**不在这里钉：那会让一次合法的目标收敛反而变红（W92 的教训）。
  // 实测（W101）为 `xtask` 10 / `node-test` 1 / `pnpm` 2，记在报告里，不写成断言。
});

test('the judge is orthogonal to W95, which reads the step set rather than its target', async () => {
  const { workflowText, gateSources, reasonInput } = await load();
  // 只改**目标**：步的名字、身份、kind、凭据一个字都没动。
  const retargeted = patch(workflowText, 'run: cargo xtask test --suite provider', 'run: cargo xtask test --suite storage');
  assert.deepEqual(
    workflowStepErrors(HOST, retargeted, WORKFLOW_STEPS, gateSources, reasonInput),
    [],
    'the W95 step-set judge must not read which target a step runs',
  );
  assert.ok(
    named(stepTargetErrors({host: HOST, workflowText: retargeted}), '"API Provider', 'test --suite storage'),
    'the target judge must be the one that notices a swapped target',
  );
  // 反向：只改**身份**。目标一个字没动 ⇒ 目标判据沉默，步集判据具名。
  const renamed = patch(workflowText, '      - name: Kernel registration and binding\n', '      - name: Kernel registration and naming\n');
  assert.ok(
    workflowStepErrors(HOST, renamed, WORKFLOW_STEPS, gateSources, reasonInput).length > 0,
    'the step-set judge must notice a rename',
  );
  assert.deepEqual(
    stepTargetErrors({host: HOST, workflowText: renamed}),
    [],
    'the target judge must not restate the step-set verdict',
  );
});

test('the target judge is the single home for "which instance of the gate runs"', async () => {
  const { workflowText } = await load();
  const retargeted = patch(workflowText, 'cargo xtask check --profile full', 'cargo xtask check --profile portable');
  assert.ok(
    named(stepTargetErrors({host: HOST, workflowText: retargeted}), 'xtask:check --profile portable'),
    'the target judge must name the swapped gate profile',
  );
  // W97 只负责**形状**：规范化之后仍然认得出「有一条 `check --profile …`」。
  assert.ok(
    ciXtaskCommands(retargeted).some(command => /^check --profile \S+$/.test(command)),
    'the normaliser must keep recognising the gate command shape',
  );
});

test('the judge is orthogonal to W84, which reads the tracked set and not the workflow body', async () => {
  const { tracked, workflowText } = await load();
  // 铁律一：本用例的断言依赖「本轮新测试文件在册」，而提交前它还没进 `.git/index`
  // ⇒ 由**构造**使前提成立（并集），不依赖提交顺序。**构造 MUST 遍历站点表自己的键**
  // （W101 订正），MUST NOT 只补本轮那一个文件——那正是 W99/W100/W101 各踩一次的坑。
  const withNew = new Set([...tracked, ...declaredMarkerPaths()]);
  const swapped = patch(workflowText, 'run: cargo xtask test --suite knowledge', 'run: cargo xtask test --suite storage');
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
    named(stepTargetErrors({host: HOST, workflowText: swapped}), '"Knowledge algorithms', 'test --suite storage'),
    'the target judge must be the one that notices a swapped target',
  );
});
