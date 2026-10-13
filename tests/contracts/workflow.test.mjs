import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { readTrackedPaths, describeFailure } from '../../scripts/contracts/git-index.mjs';
import {
  WORKFLOW_FILES,
  STEP_KINDS,
  CI_ONLY_REASONS,
  WORKFLOW_STEPS,
  stepIdentity,
  workflowStepsOf,
  workflowFileErrors,
  workflowStepErrors,
} from '../../scripts/contracts/workflow-steps.mjs';

const HOST = '.github/workflows/foundation.yml';
const GATE = 'xtask/src/lib.rs';

/// 判据的输入**显式注入**：`gateSources` 少了它，「本机也跑」的凭据会退化成恒真。
async function load() {
  const repository = await readTrackedPaths();
  assert.ok(repository.ok, describeFailure(repository));
  const workflowText = await readFile(HOST, 'utf8');
  const gateSources = new Map([[GATE, await readFile(GATE, 'utf8')]]);
  return {repository, workflowText, gateSources};
}

function registryOf(...mutations) {
  const copy = structuredClone(WORKFLOW_STEPS);
  for (const mutate of mutations) mutate(copy);
  return copy;
}

function entry(registry, identity) {
  return registry.find(item => item.identity === identity);
}

test('every workflow step is declared, and every declaration still has a step', async () => {
  const {repository, workflowText, gateSources} = await load();
  // 文件层：清单非空，且与仓库已跟踪的 workflow 文件双向一致。
  assert.ok(WORKFLOW_FILES.length > 0, 'the declared workflow set must not be empty');
  assert.deepEqual(workflowFileErrors(repository.paths), [], 'the declared workflow set must match the repository');
  // 步层：解析出的步与在册条目**双向**一致。
  assert.deepEqual(
    workflowStepErrors(HOST, workflowText, WORKFLOW_STEPS, gateSources),
    [],
    'the shipped step registry must match the workflow',
  );
  const parsed = workflowStepsOf(workflowText);
  const registered = WORKFLOW_STEPS.filter(item => item.host === HOST);
  assert.ok(parsed.length > 0, 'the workflow must declare steps');
  assert.equal(parsed.length, registered.length, 'every step must have exactly one declaration');
  assert.equal(new Set(parsed.map(step => step.identity)).size, parsed.length, 'step identities must be unique');
  // 非空洞：四类都真的被用到——否则某一格的判据对空集恒真。
  const kinds = new Set(registered.map(item => item.kind));
  for (const kind of STEP_KINDS) {
    assert.ok(kinds.has(kind), `the registry must exercise the ${kind} kind, otherwise its rules are vacuous`);
  }
});

test('an unregistered step is named', async () => {
  const {workflowText, gateSources} = await load();
  const injected = workflowText.replace(
    '      - name: Kernel registration and binding\n',
    '      - name: W95 unregistered probe\n        run: node -e "probe"\n      - name: Kernel registration and binding\n',
  );
  const errors = workflowStepErrors(HOST, injected, WORKFLOW_STEPS, gateSources);
  assert.ok(
    errors.some(error => error.includes('W95 unregistered probe') && error.includes('nobody declares')),
    `an unregistered step must be named, saw ${JSON.stringify(errors)}`,
  );
});

test('a registered step that vanished is named', async () => {
  const {workflowText, gateSources} = await load();
  const injected = workflowText.replace(
    '      - name: Kernel registration and binding\n        run: cargo xtask test --suite kernel\n',
    '',
  );
  const errors = workflowStepErrors(HOST, injected, WORKFLOW_STEPS, gateSources);
  assert.ok(
    errors.some(error => error.includes('Kernel registration and binding') && error.includes('does not')),
    `a vanished step must be named, saw ${JSON.stringify(errors)}`,
  );
});

test('renaming a step fails on both sides', async () => {
  const {workflowText, gateSources} = await load();
  const injected = workflowText.replace(
    '      - name: Plugin hosts and IPC\n',
    '      - name: Plugin hosts and IPC (renamed)\n',
  );
  const errors = workflowStepErrors(HOST, injected, WORKFLOW_STEPS, gateSources);
  assert.ok(errors.some(error => error.includes('Plugin hosts and IPC (renamed)')), 'the new name must be named');
  assert.ok(
    errors.some(error => error.includes('"Plugin hosts and IPC"') && error.includes('does not')),
    'the old name must be reported as vanished too',
  );
});

test('local coverage has to cite a gate site that exists', async () => {
  const {workflowText, gateSources} = await load();
  // 注入一：凭据指向一个真实文件里不存在的串 ⇒ 假契约。
  const ghost = registryOf(copy => {
    entry(copy, 'Kernel registration and binding').localEvidence = {file: GATE, marker: '"no-such-gate-marker"'};
  });
  assert.ok(
    workflowStepErrors(HOST, workflowText, ghost, gateSources).some(error =>
      error.includes('Kernel registration and binding') && error.includes('not in that file'),
    ),
    'a gate marker that does not exist must be reported',
  );
  // 注入二：凭据指向一个没被提供的文件 ⇒ 缺输入必须具名，绝不静默跳过。
  const absent = registryOf(copy => {
    entry(copy, 'Kernel registration and binding').localEvidence = {file: 'xtask/src/no-such.rs', marker: 'x'};
  });
  assert.ok(
    workflowStepErrors(HOST, workflowText, absent, gateSources).some(error =>
      error.includes('not provided for judgement'),
    ),
    'a missing gate source must be named rather than skipped',
  );
  // 注入三：根本没有凭据 ⇒ 「本机也跑」是一句空话。
  const bare = registryOf(copy => {
    delete entry(copy, 'Kernel registration and binding').localEvidence;
  });
  assert.ok(
    workflowStepErrors(HOST, workflowText, bare, gateSources).some(error =>
      error.includes('names no gate site'),
    ),
    'a local claim without evidence must be reported',
  );
});

test('a reason outside the closed set is a label, not a reason', async () => {
  const {workflowText, gateSources} = await load();
  const injected = registryOf(copy => {
    entry(copy, 'Full three-language contract suite (Linux C++ toolchain)').reason = 'because';
  });
  assert.ok(
    workflowStepErrors(HOST, workflowText, injected, gateSources).some(error =>
      error.includes('not in the cause table'),
    ),
    'an unknown cause must be named',
  );
  // 非空洞：闭集本身非空，否则上面那条对空集恒真。
  assert.ok(CI_ONLY_REASONS.length > 0, 'the cause table must not be empty');
});

test('a ci-only step must not also claim local coverage', async () => {
  const {workflowText, gateSources} = await load();
  const injected = registryOf(copy => {
    entry(copy, 'pnpm spec:validate').localEvidence = {file: GATE, marker: '["check", "--profile", "full"]'};
  });
  assert.ok(
    workflowStepErrors(HOST, workflowText, injected, gateSources).some(error =>
      error.includes('covered locally and only-in-CI at once'),
    ),
    'a step cannot be local and ci-only at once',
  );
});

test('infra must not be a back door around the checks', async () => {
  const {workflowText, gateSources} = await load();
  const injected = registryOf(copy => {
    entry(copy, 'Kernel registration and binding').kind = 'infra';
    delete entry(copy, 'Kernel registration and binding').localEvidence;
  });
  assert.ok(
    workflowStepErrors(HOST, workflowText, injected, gateSources).some(error =>
      error.includes('looks like a judgement'),
    ),
    'a judgement disguised as infra must be reported',
  );
});

test('a diagnostic step is recognised by its condition, not by its name', async () => {
  const {workflowText, gateSources} = await load();
  const dropped = workflowText.replace('        if: failure()\n', '');
  assert.ok(
    workflowStepErrors(HOST, dropped, WORKFLOW_STEPS, gateSources).some(error =>
      error.includes('no failure condition'),
    ),
    'a diagnostic without a failure condition must be reported',
  );
  // 反向：`if: runner.os == 'Linux'` 是平台条件，不是失败条件。
  const platform = registryOf(copy => {
    entry(copy, 'Install Linux C++ protobuf runtime').kind = 'diagnostic';
  });
  assert.ok(
    workflowStepErrors(HOST, workflowText, platform, gateSources).some(error =>
      error.includes('no failure condition'),
    ),
    'a platform condition must not pass as a failure condition',
  );
});

test('a legitimate registration is not reported', async () => {
  const {workflowText, gateSources} = await load();
  // 反向：把凭据换成**另一处真的存在**的门控落点 ⇒ MUST NOT 红（判据验「存在」，不验「等于某串」）。
  const other = registryOf(copy => {
    entry(copy, 'Kernel registration and binding').localEvidence = {
      file: GATE,
      marker: '["check", "--profile", "full"]',
    };
  });
  assert.deepEqual(
    workflowStepErrors(HOST, workflowText, other, gateSources),
    [],
    'a different but existing gate site must not be reported',
  );
});

test('an unparseable workflow or an empty step set is named, never skipped', async () => {
  const {gateSources} = await load();
  const broken = workflowStepErrors(HOST, 'jobs: [1, 2', WORKFLOW_STEPS, gateSources);
  assert.ok(
    broken.some(error => error.includes('could not be parsed')),
    'an unparseable workflow must be named rather than silently pass',
  );
  const empty = workflowStepErrors(HOST, 'jobs:\n  x:\n    steps: []\n', WORKFLOW_STEPS, gateSources);
  assert.ok(
    empty.some(error => error.includes('declares no steps')),
    'an empty step set must be reported',
  );
});

test('a tracked workflow nobody declares is named', async () => {
  const {repository} = await load();
  assert.deepEqual(workflowFileErrors(repository.paths), [], 'the baseline must be clean');
  const extra = new Set([...repository.paths, '.github/workflows/extra.yml']);
  assert.ok(
    workflowFileErrors(extra).some(error => error.includes('.github/workflows/extra.yml')),
    'a tracked workflow with no declaration must be named',
  );
  assert.ok(
    workflowFileErrors(new Set()).some(error => error.includes('not tracked')),
    'a declared workflow that is not tracked must be named',
  );
});

test('a step identity prefers name, then uses, then the first run line', () => {
  assert.equal(stepIdentity({name: 'n', uses: 'u', run: 'r'}), 'n');
  assert.equal(stepIdentity({uses: 'u', run: 'r'}), 'u');
  assert.equal(stepIdentity({run: 'first\nsecond'}), 'first');
  assert.equal(stepIdentity({}), '');
  // 非空洞：这些形状必须在真实 workflow 里都出现过，否则上面的优先级是凭空的。
  const parsed = workflowStepsOf('jobs:\n  j:\n    steps:\n      - uses: a/b\n      - run: c\n');
  assert.deepEqual(parsed.map(step => step.identity), ['a/b', 'c']);
});
