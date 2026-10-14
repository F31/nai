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
import {
  WORKFLOW_STEPS,
  workflowJobsOf,
  workflowStepsOf,
  workflowStepErrors,
} from '../../scripts/contracts/workflow-steps.mjs';
import {
  RUNNER_OS_VALUES,
  RUNNER_LABEL_PREFIXES,
  CONDITION_SHAPES,
  PLATFORM_COMMAND_SHAPES,
  osFamilyOf,
  platformsOf,
  conditionShapeOf,
  platformLegsOf,
  continueOnErrorOf,
  stepConfigErrors,
} from '../../scripts/contracts/step-config.mjs';

const HOST = '.github/workflows/foundation.yml';
const GATE = 'xtask/src/lib.rs';

/// 判据的输入**全部显式注入**：本模块自己不读磁盘（遍历工作区是 W75 的 F2 根因）。
/// 真实仓库那一组在文件末尾，它读真 workflow 与真 `lib.rs`。
async function load() {
  const index = await readTrackedPaths('.');
  assert.ok(index.ok, describeFailure(index));
  return {
    tracked: index.paths,
    workflowText: await readFile(HOST, 'utf8'),
    gateText: await readFile(GATE, 'utf8'),
    gateSources: new Map([[GATE, readFileSync(GATE, 'utf8')]]),
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

// ---------------------------------------------------------------------------
// 合成世界：一个两腿矩阵的单 job，七步覆盖五种 kind 与两种条件形状
// ---------------------------------------------------------------------------

const SYNTHETIC_WORKFLOW = [
  'name: Synthetic',
  'on:',
  '  push:',
  'jobs:',
  '  j:',
  '    strategy:',
  '      fail-fast: false',
  '      matrix:',
  '        os: [ubuntu-24.04, windows-latest]',
  '    runs-on: ${{ matrix.os }}',
  '    steps:',
  '      - name: Setup',
  '        run: pnpm install --frozen-lockfile',
  '      - name: Linux runtime',
  "        if: runner.os == 'Linux'",
  '        run: sudo apt-get install -y libprotobuf-dev',
  '      - name: Gate',
  '        run: cargo xtask check --profile full',
  '      - name: Portable suite',
  '        run: node --test tests/a.test.mjs',
  '      - name: Contracts',
  "        if: runner.os == 'Linux'",
  '        run: cargo xtask test --suite contracts',
  '      - name: Spec',
  '        run: pnpm spec:validate',
  '      - name: Diagnose',
  '        if: failure()',
  '        run: node scripts/ci-diagnose.mjs log',
  '        continue-on-error: true',
  '',
].join('\n');

const SYNTHETIC_REGISTRY = [
  {host: HOST, identity: 'Setup', kind: 'infra'},
  {host: HOST, identity: 'Linux runtime', kind: 'infra'},
  {host: HOST, identity: 'Gate', kind: 'local', localEvidence: {file: GATE, marker: 'gate'}},
  {host: HOST, identity: 'Portable suite', kind: 'local', localEvidence: {file: GATE, marker: 'suite'}},
  {host: HOST, identity: 'Contracts', kind: 'ci-only', reason: 'platform-toolchain'},
  {host: HOST, identity: 'Spec', kind: 'ci-only', reason: 'not-in-profile'},
  {host: HOST, identity: 'Diagnose', kind: 'diagnostic'},
];

const synthetic = (workflowText = SYNTHETIC_WORKFLOW) =>
  stepConfigErrors({host: HOST, workflowText, registered: SYNTHETIC_REGISTRY});

const named = (errors, ...needles) =>
  errors.some(error => needles.every(needle => error.includes(needle)));

// ---------------------------------------------------------------------------
// F 级：合成世界
// ---------------------------------------------------------------------------

test('F0 the synthetic world is green and every registered step is actually judged', () => {
  assert.deepEqual(synthetic(), [], 'the synthetic baseline must be green');
  // 前提的另一半：空数组不能只是「什么都没判到」。七个在册步全部被解析出来。
  const parsed = workflowStepsOf(SYNTHETIC_WORKFLOW);
  assert.equal(parsed.length, SYNTHETIC_REGISTRY.length, 'every registered step must have a parsed step');
});

test('F1 a step whose execution platform set is empty is named', () => {
  // 矩阵只剩 Windows，而 Contracts 步仍要求 Linux ⇒ 它永远不会跑。
  const errors = synthetic(patch(SYNTHETIC_WORKFLOW, 'os: [ubuntu-24.04, windows-latest]', 'os: [windows-latest]'));
  assert.ok(
    named(errors, 'Contracts', 'empty execution platform set', 'silently disappeared'),
    `an unreachable judgement must be named, saw ${JSON.stringify(errors)}`,
  );
  // 反向（**集合级，不是字段级**）：条件仍在、语法正确，只是它筛空了 ⇒ 这正是「字段级」看不见的那一格。
  assert.ok(
    !errors.some(error => error.includes('Spec')),
    'a step with no condition must not be reported as unreachable',
  );
});

test('F2 a judgement that can be swallowed is named, and a diagnostic that is not tolerated is named', () => {
  const swallowed = synthetic(
    patch(SYNTHETIC_WORKFLOW, '      - name: Gate\n        run:', '      - name: Gate\n        continue-on-error: true\n        run:'),
  );
  assert.ok(
    named(swallowed, 'Gate', 'is local yet can fail without failing the job'),
    `a swallowable judgement must be named, saw ${JSON.stringify(swallowed)}`,
  );

  const ciSwallowed = synthetic(
    patch(SYNTHETIC_WORKFLOW, '      - name: Spec\n        run:', '      - name: Spec\n        continue-on-error: true\n        run:'),
  );
  assert.ok(
    named(ciSwallowed, 'Spec', 'is ci-only yet can fail without failing the job'),
    `a swallowable ci-only judgement must be named, saw ${JSON.stringify(ciSwallowed)}`,
  );

  const strictDiagnostic = synthetic(
    patch(SYNTHETIC_WORKFLOW, '        run: node scripts/ci-diagnose.mjs log\n        continue-on-error: true\n', '        run: node scripts/ci-diagnose.mjs log\n'),
  );
  assert.ok(
    named(strictDiagnostic, 'Diagnose', 'diagnostic step without continue-on-error'),
    `a diagnostic that can replace the verdict must be named, saw ${JSON.stringify(strictDiagnostic)}`,
  );
});

test('F3 the ci-only cause must agree with the platform condition, in both directions', () => {
  // 正向：`platform-toolchain` 却没有任何平台限制 ⇒ 成因没付代价。
  const unrestricted = synthetic(
    patch(SYNTHETIC_WORKFLOW, "      - name: Contracts\n        if: runner.os == 'Linux'\n", '      - name: Contracts\n'),
  );
  assert.ok(
    named(unrestricted, 'Contracts', 'needs a platform toolchain', 'not paid for by a platform condition'),
    `an unpaid platform cause must be named, saw ${JSON.stringify(unrestricted)}`,
  );

  // 反向：`not-in-profile` 与平台无关，限制它就等于让别的平台**静默不跑**。
  const restricted = synthetic(
    patch(SYNTHETIC_WORKFLOW, '      - name: Spec\n        run:', "      - name: Spec\n        if: runner.os == 'Linux'\n        run:"),
  );
  assert.ok(
    named(restricted, 'Spec', '"not-in-profile"', 'silently skip the other platforms'),
    `a platform-unrelated cause must not be platform-restricted, saw ${JSON.stringify(restricted)}`,
  );
});

test('F4 every configuration value outside its closed set is named', () => {
  const cases = [
    {
      name: 'runner.os value',
      text: patch(SYNTHETIC_WORKFLOW, "        if: runner.os == 'Linux'\n        run: cargo xtask test --suite contracts", "        if: runner.os == 'FreeBSD'\n        run: cargo xtask test --suite contracts"),
      needle: ['FreeBSD', 'not a runner OS value'],
    },
    {
      name: 'runner label',
      text: patch(SYNTHETIC_WORKFLOW, 'os: [ubuntu-24.04, windows-latest]', 'os: [ubuntu-24.04, freebsd-14]'),
      needle: ['freebsd-14', 'not in the recognized label table'],
    },
    {
      name: 'condition shape',
      text: patch(SYNTHETIC_WORKFLOW, '        if: failure()\n', '        if: always()\n'),
      needle: ['always()', 'not one of the recognized condition shapes'],
    },
    {
      name: 'continue-on-error literal',
      text: patch(SYNTHETIC_WORKFLOW, '        continue-on-error: true\n', '        continue-on-error: ${{ matrix.os }}\n'),
      needle: ['not a boolean literal'],
    },
    {
      name: 'extra matrix axis',
      text: patch(SYNTHETIC_WORKFLOW, '        os: [ubuntu-24.04, windows-latest]', '        os: [ubuntu-24.04, windows-latest]\n        flavor: [a, b]'),
      needle: ['matrix axes', 'only the single "os" axis is recognized'],
    },
    {
      name: 'empty matrix',
      text: patch(SYNTHETIC_WORKFLOW, '        os: [ubuntu-24.04, windows-latest]', '        os: []'),
      needle: ['not a non-empty list of runner labels'],
    },
    {
      name: 'runs-on does not consume the matrix',
      text: patch(SYNTHETIC_WORKFLOW, '    runs-on: ${{ matrix.os }}', '    runs-on: ubuntu-24.04'),
      needle: ['a matrix that runs-on does not consume'],
    },
    {
      name: 'runs-on references a matrix that is not declared',
      text: patch(SYNTHETIC_WORKFLOW, '    strategy:\n      fail-fast: false\n      matrix:\n        os: [ubuntu-24.04, windows-latest]\n', '    strategy:\n      fail-fast: false\n'),
      needle: ['declares no matrix', 'the platform set cannot be read'],
    },
  ];
  for (const { name, text, needle } of cases) {
    const errors = synthetic(text);
    assert.ok(
      named(errors, ...needle),
      `an out-of-domain ${name} must be named, saw ${JSON.stringify(errors)}`,
    );
  }
});

test('F5 a platform-specific command on a platform that lacks it is named', () => {
  // 删掉 Linux 条件 ⇒ `sudo`/`apt-get` 会在 Windows 腿上也跑。
  // 注意 `platform-os` 形状仍由 Contracts 步实例化，所以红必须来自 R5 而不是形状非空洞。
  const errors = synthetic(
    patch(SYNTHETIC_WORKFLOW, "      - name: Linux runtime\n        if: runner.os == 'Linux'\n", '      - name: Linux runtime\n'),
  );
  assert.ok(
    named(errors, 'Linux runtime', 'the command is not available there'),
    `a platform-specific command outside its platforms must be named, saw ${JSON.stringify(errors)}`,
  );
  assert.ok(
    named(errors, '"Windows"'),
    'the unavailable platform must be named, not just counted',
  );
  assert.ok(
    !errors.some(error => error.includes('no step with a "platform-os" condition')),
    'the platform-os shape is still instantiated by another step',
  );
});

test('F6 the shape tables are not vacuous, and the judged set must not be empty', () => {
  // 条件形状表：每个形状 MUST 有实例。
  const noFailure = synthetic(patch(SYNTHETIC_WORKFLOW, '        if: failure()\n', ''));
  assert.ok(
    named(noFailure, 'no step with a "failure" condition'),
    `a condition shape with no instance must be named, saw ${JSON.stringify(noFailure)}`,
  );
  // 命令形状表：每个形状 MUST 有实例。
  const noSudo = synthetic(
    patch(SYNTHETIC_WORKFLOW, '        run: sudo apt-get install -y libprotobuf-dev', '        run: echo ready'),
  );
  assert.ok(
    named(noSudo, 'no step whose command matches'),
    `a platform-command shape with no instance must be named, saw ${JSON.stringify(noSudo)}`,
  );
  // 空注册表 / 注册表与 host 不匹配 ⇒ 判定集为空。
  const emptyRegistry = stepConfigErrors({host: HOST, workflowText: SYNTHETIC_WORKFLOW, registered: []});
  assert.ok(
    named(emptyRegistry, 'non-empty registered step set'),
    `an empty registry must be named, saw ${JSON.stringify(emptyRegistry)}`,
  );
  const noMatch = stepConfigErrors({
    host: '.github/workflows/other.yml',
    workflowText: SYNTHETIC_WORKFLOW,
    registered: SYNTHETIC_REGISTRY,
  });
  assert.ok(
    named(noMatch, 'carries no registered step', 'vacuously true'),
    `an empty judged set must be named, saw ${JSON.stringify(noMatch)}`,
  );
});

test('F7 missing or unparseable input is named rather than silently skipped', () => {
  const noHost = stepConfigErrors({workflowText: SYNTHETIC_WORKFLOW, registered: SYNTHETIC_REGISTRY});
  assert.ok(named(noHost, 'requires the workflow host path'), `a missing host must be named, saw ${JSON.stringify(noHost)}`);
  const noText = stepConfigErrors({host: HOST, registered: SYNTHETIC_REGISTRY});
  assert.ok(named(noText, 'requires the workflow text'), `missing text must be named, saw ${JSON.stringify(noText)}`);
  const unparseable = synthetic('jobs: [ this is not a mapping of jobs\n');
  assert.ok(
    named(unparseable, 'could not be parsed'),
    `an unparseable workflow must be named, saw ${JSON.stringify(unparseable)}`,
  );
  assert.notDeepEqual(unparseable, [], 'being unable to read must not look like having no problem');
});

test('F8 the shape and platform helpers are pure and closed', () => {
  assert.deepEqual(osFamilyOf('ubuntu-24.04'), 'Linux');
  assert.deepEqual(osFamilyOf('windows-latest'), 'Windows');
  assert.deepEqual(osFamilyOf('macos-14'), 'macOS');
  assert.equal(conditionShapeOf(''), '');
  assert.equal(conditionShapeOf("runner.os == 'Linux'"), 'platform-os');
  assert.equal(conditionShapeOf('failure()'), 'failure');
  assert.equal(conditionShapeOf('always()'), 'unknown');

  const errors = [];
  const platforms = platformsOf(
    {matrix: {os: ['ubuntu-24.04', 'macos-14', 'windows-latest']}, runsOn: '${{ matrix.os }}'},
    errors,
    'job',
  );
  assert.deepEqual(errors, []);
  // macOS 上的 `sudo` 是**可以的**：支持集写成 ['Linux'] 会误报一个正确的配置。
  const legs = platformLegsOf({identity: 'x', condition: ''}, platforms, errors, 'job');
  assert.deepEqual(legs, ['ubuntu-24.04', 'macos-14', 'windows-latest']);
  for (const command of PLATFORM_COMMAND_SHAPES) {
    assert.ok(
      command.supported.every(os => RUNNER_OS_VALUES.includes(os)),
      `every supported platform of ${command.id} must be a runner OS value`,
    );
  }
  assert.deepEqual(RUNNER_OS_VALUES, ['Linux', 'Windows', 'macOS']);
  assert.deepEqual(RUNNER_LABEL_PREFIXES.map(entry => entry.os), RUNNER_OS_VALUES);
  assert.deepEqual(CONDITION_SHAPES, ['platform-os', 'failure']);
  // `continue-on-error` 只认布尔字面量或「没写」。
  assert.equal(continueOnErrorOf({continueOnError: undefined}, [], 'x'), false);
  assert.equal(continueOnErrorOf({continueOnError: false}, [], 'x'), false);
  assert.equal(continueOnErrorOf({continueOnError: true}, [], 'x'), true);
  assert.equal(continueOnErrorOf({continueOnError: 'yes'}, [], 'x'), null);
});

test('F9 the two views come from one parse and the W95 shape is unchanged', () => {
  const jobs = workflowJobsOf(SYNTHETIC_WORKFLOW);
  const flat = workflowStepsOf(SYNTHETIC_WORKFLOW);
  assert.equal(jobs.length, 1);
  assert.equal(jobs[0].id, 'j');
  assert.deepEqual(jobs[0].matrix, {os: ['ubuntu-24.04', 'windows-latest']});
  assert.equal(flat.length, jobs[0].steps.length, 'both views must describe the same step set');
  // W95 的 `{identity, condition, run}` 语义一字不动，新字段只是增补。
  assert.deepEqual(
    flat.map(step => step.identity),
    jobs[0].steps.map(step => step.identity),
  );
  assert.equal(flat[0].identity, 'Setup');
  assert.equal(flat[0].condition, '');
  assert.equal(flat[1].condition, "runner.os == 'Linux'");
  assert.equal(flat[6].condition, 'failure()');
  assert.equal(flat[6].continueOnError, true);
  assert.equal(flat[0].continueOnError, undefined);
  // `matrix` 与同 job 的步共享**同一个对象**：拷贝会让同一个矩阵变成若干份可分别篡改的副本。
  assert.equal(flat[0].matrix, flat[6].matrix, 'the matrix must be shared, not copied');
  // 扁平视图 MUST 就是 job 视图的派生：逐字对账，MUST NOT 是第二份解析结果。
  const derived = [];
  for (const job of jobs) {
    for (const step of job.steps) derived.push({...step, job: job.id, matrix: job.matrix});
  }
  assert.deepEqual(flat, derived, 'the flat view must be exactly the derived job view');
});

// ---------------------------------------------------------------------------
// 真实仓库
// ---------------------------------------------------------------------------

test('the real repository pays for every step execution configuration', async () => {
  const { workflowText } = await load();
  assert.deepEqual(
    stepConfigErrors({host: HOST, workflowText}),
    [],
    'every shipped step execution configuration must be paid for',
  );
  // 前提的另一半：空数组不能只是「什么都没解析出来」。
  const jobs = workflowJobsOf(workflowText);
  const steps = workflowStepsOf(workflowText);
  assert.equal(steps.length, WORKFLOW_STEPS.length, 'every registered step must have a parsed step');
  assert.ok(jobs.length >= 1, 'the workflow must declare at least one job');
  const platforms = jobs.flatMap(job => platformsOf(job, [], 'job'));
  assert.ok(platforms.length >= 2, 'the real matrix must have at least two legs, or reachability is vacuous');
  const restricted = steps.filter(step => step.condition.includes('runner.os'));
  assert.ok(restricted.length >= 1, 'the real workflow must exercise a platform condition');
  assert.ok(
    steps.some(step => step.continueOnError === true),
    'the real workflow must exercise a tolerated failure, or the diagnostic rule is vacuous',
  );
});

test('the real workflow keeps the failure switch off the judged steps only', async () => {
  const { workflowText } = await load();
  const steps = workflowStepsOf(workflowText);
  const tolerated = steps.filter(step => step.continueOnError === true).map(step => step.identity);
  // 唯一的容错步就是诊断通道——把断言写成**清单**，而不是「数量为 1」：
  // 数量会随合法新增漂移，名字不会。
  assert.deepEqual(tolerated.sort(), ['Publish the gate failure summary as annotations']);
});

test('the judge is orthogonal to W95, which reads the step set rather than its configuration', async () => {
  const { workflowText, gateSources } = await load();
  // 只改**配置**：把一个步的平台条件指到一个不存在的平台。步集、身份、kind 全都没动。
  const reconfigured = patch(
    workflowText,
    "        if: runner.os == 'Linux'\n        run: cargo xtask test --suite contracts",
    "        if: runner.os == 'FreeBSD'\n        run: cargo xtask test --suite contracts",
  );
  assert.deepEqual(
    workflowStepErrors(HOST, reconfigured, WORKFLOW_STEPS, gateSources),
    [],
    'the W95 step-set judge must not read the platform condition of a non-diagnostic step',
  );
  assert.ok(
    stepConfigErrors({host: HOST, workflowText: reconfigured}).length > 0,
    'the configuration judge must be the one that notices a platform condition change',
  );

  // 反向：只改**身份**（步改名）。配置一个字没动 ⇒ 配置判据沉默，步集判据具名。
  const renamed = patch(workflowText, '      - name: Kernel registration and binding\n', '      - name: Kernel registration and naming\n');
  assert.ok(
    workflowStepErrors(HOST, renamed, WORKFLOW_STEPS, gateSources).length > 0,
    'the step-set judge must notice a rename',
  );
  assert.deepEqual(
    stepConfigErrors({host: HOST, workflowText: renamed}),
    [],
    'the configuration judge must not read step identities beyond registry membership',
  );
});

test('the judge is orthogonal to W84 and W99, which read the tracked set and the run text', async () => {
  const { tracked, workflowText } = await load();
  // 铁律一：本用例的断言依赖「本轮新测试文件在册」，而提交前它还没进 `.git/index`
  // ⇒ 由**构造**使前提成立（并集），不依赖提交顺序。**构造 MUST 遍历站点表自己的键**
  // （W101 订正）：原先只补自己那一轮新增的那一个文件，于是每加一个站点就让上一轮的用例失效。
  const withNew = new Set([...tracked, ...declaredMarkerPaths()]);
  const reconfigured = patch(workflowText, 'os: [ubuntu-24.04, windows-latest]', 'os: [windows-latest]');
  // W84 只看 `.git/index` 上的文件覆盖，不读 workflow 正文的一个字符。
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
    stepConfigErrors({host: HOST, workflowText: reconfigured}).length > 0,
    'the configuration judge must be the one that notices a matrix change',
  );
});

test('the judge shares the condition field with W95 without duplicating its verdict', async () => {
  const { workflowText, gateSources } = await load();
  // 删掉诊断步的失败条件：两条判据都会红，但**断言不同**——
  // W95 判「该步的角色与它的条件是否一致」，W100 判「条件形状表是否还有实例」。
  const dropped = patch(workflowText, '      - name: Publish the gate failure summary as annotations\n        if: failure()\n', '      - name: Publish the gate failure summary as annotations\n');
  const w95 = workflowStepErrors(HOST, dropped, WORKFLOW_STEPS, gateSources);
  const w100 = stepConfigErrors({host: HOST, workflowText: dropped});
  assert.ok(
    w95.some(error => error.includes('diagnostic step but') || error.includes('failure condition')),
    `W95 must name the role mismatch, saw ${JSON.stringify(w95)}`,
  );
  assert.ok(
    w100.some(error => error.includes('no step with a "failure" condition')),
    `W100 must name the vacuous shape, saw ${JSON.stringify(w100)}`,
  );
  assert.ok(
    !w100.some(error => error.includes('diagnostic step without') || error.includes('failure condition')),
    'W100 must not restate the W95 verdict — different judgement domains, different messages',
  );
});
