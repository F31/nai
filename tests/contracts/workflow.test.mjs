import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { readTrackedPaths, describeFailure } from '../../scripts/contracts/git-index.mjs';
import {
  WORKFLOW_FILES,
  STEP_KINDS,
  CI_ONLY_REASONS,
  CI_ONLY_REASON_PAYMENTS,
  WORKFLOW_STEPS,
  stepIdentity,
  workflowStepsOf,
  workflowFileErrors,
  workflowStepErrors,
} from '../../scripts/contracts/workflow-steps.mjs';

const HOST = '.github/workflows/foundation.yml';
const GATE = 'xtask/src/lib.rs';

/// 夹具锚点：**单行 `- name:` 的步**。
///
/// ⚠️ W134 教训：这些锚点原先指向 `Kernel registration and binding` 等 7 条分套件步，
/// 而那 7 条被删掉（它们的包集全被门控步覆盖，见 `scripts/contracts/step-overlap.mjs`）
/// ⇒ `replace` 变成**静默 no-op**、注入根本没发生，于是「注入后应当红」的用例
/// 反过来全红。锚点 MUST 是那种**删掉它就整条消失**的单行步，并且 MUST 在改动后
/// 立刻被现测（`assert.notEqual(injected, workflowText)`）确认命中。
const SCHEMA_STEP = 'Validate schemas and API contract';

/// 唯一幸存的全平台 `local` 步：`localEvidence` 类断言（W75）只能锚在它身上。
const GATE_STEP = 'Complete engineering foundation (portable steps + frontend workspace)';

/// 凭据文件的**内容**从兑付表自己派生——**不写第二份文件清单**（两份清单会各自漏掉对方的新增）。
/// 表改了，输入自动跟上。读不到的文件**不抛**：让**判据**去具名报告「未提供」，
/// 而不是让输入构造的 `ENOENT` 把「判据该报的缺陷」伪装成「测试自己坏了」。
async function paymentInput(payments, tracked) {
  const sources = new Map();
  for (const file of new Set(Object.values(payments).flatMap(p => p.evidence.map(i => i.file)))) {
    try {
      sources.set(file, await readFile(file, 'utf8'));
    } catch {
      // 判据会具名报告它，MUST NOT 在这里静默吞掉。
    }
  }
  return {sources, tracked};
}

/// 判据的输入**显式注入**：`gateSources` 少了它，「本机也跑」的凭据会退化成恒真；
/// `reasonInput` 少了它，「这条成因凭什么成立」同样会退化成恒真。
async function load() {
  const repository = await readTrackedPaths();
  assert.ok(repository.ok, describeFailure(repository));
  const workflowText = await readFile(HOST, 'utf8');
  const gateSources = new Map([[GATE, await readFile(GATE, 'utf8')]]);
  return {
    repository,
    workflowText,
    gateSources,
    reasonInput: await paymentInput(CI_ONLY_REASON_PAYMENTS, repository.paths),
  };
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
  const {repository, workflowText, gateSources, reasonInput} = await load();
  // 文件层：清单非空，且与仓库已跟踪的 workflow 文件双向一致。
  assert.ok(WORKFLOW_FILES.length > 0, 'the declared workflow set must not be empty');
  assert.deepEqual(workflowFileErrors(repository.paths), [], 'the declared workflow set must match the repository');
  // 步层：解析出的步与在册条目**双向**一致。
  assert.deepEqual(
    workflowStepErrors(HOST, workflowText, WORKFLOW_STEPS, gateSources, reasonInput),
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
  const {workflowText, gateSources, reasonInput} = await load();
  const anchor = `      - name: ${SCHEMA_STEP}\n`;
  const injected = workflowText.replace(
    anchor,
    `      - name: W95 unregistered probe\n        run: node -e "probe"\n${anchor}`,
  );
  assert.notEqual(injected, workflowText, 'the fixture anchor must hit the workflow');
  const errors = workflowStepErrors(HOST, injected, WORKFLOW_STEPS, gateSources, reasonInput);
  assert.ok(
    errors.some(error => error.includes('W95 unregistered probe') && error.includes('nobody declares')),
    `an unregistered step must be named, saw ${JSON.stringify(errors)}`,
  );
});

test('a registered step that vanished is named', async () => {
  const {workflowText, gateSources, reasonInput} = await load();
  // 删掉一条**单行**步：它整条消失，不多留半截 `run:` 块。
  const injected = workflowText.replace('      - run: pnpm spec:validate\n', '');
  assert.notEqual(injected, workflowText, 'the fixture anchor must hit the workflow');
  const errors = workflowStepErrors(HOST, injected, WORKFLOW_STEPS, gateSources, reasonInput);
  assert.ok(
    errors.some(error => error.includes('"pnpm spec:validate"') && error.includes('does not')),
    `a vanished step must be named, saw ${JSON.stringify(errors)}`,
  );
});

test('renaming a step fails on both sides', async () => {
  const {workflowText, gateSources, reasonInput} = await load();
  const injected = workflowText.replace(
    `      - name: ${SCHEMA_STEP}\n`,
    `      - name: ${SCHEMA_STEP} (renamed)\n`,
  );
  assert.notEqual(injected, workflowText, 'the fixture anchor must hit the workflow');
  const errors = workflowStepErrors(HOST, injected, WORKFLOW_STEPS, gateSources, reasonInput);
  assert.ok(errors.some(error => error.includes(`${SCHEMA_STEP} (renamed)`)), 'the new name must be named');
  assert.ok(
    errors.some(error => error.includes(`"${SCHEMA_STEP}"`) && error.includes('does not')),
    'the old name must be reported as vanished too',
  );
});

test('local coverage has to cite a gate site that exists', async () => {
  const {workflowText, gateSources, reasonInput} = await load();
  // 注入一：凭据指向一个真实文件里不存在的串 ⇒ 假契约。
  const ghost = registryOf(copy => {
    entry(copy, GATE_STEP).localEvidence = {file: GATE, marker: '"no-such-gate-marker"'};
  });
  assert.ok(
    workflowStepErrors(HOST, workflowText, ghost, gateSources, reasonInput).some(error =>
      error.includes(GATE_STEP) && error.includes('not in that file'),
    ),
    'a gate marker that does not exist must be reported',
  );
  // 注入二：凭据指向一个没被提供的文件 ⇒ 缺输入必须具名，绝不静默跳过。
  const absent = registryOf(copy => {
    entry(copy, GATE_STEP).localEvidence = {file: 'xtask/src/no-such.rs', marker: 'x'};
  });
  assert.ok(
    workflowStepErrors(HOST, workflowText, absent, gateSources, reasonInput).some(error =>
      error.includes('not provided for judgement'),
    ),
    'a missing gate source must be named rather than skipped',
  );
  // 注入三：根本没有凭据 ⇒ 「本机也跑」是一句空话。
  const bare = registryOf(copy => {
    delete entry(copy, GATE_STEP).localEvidence;
  });
  assert.ok(
    workflowStepErrors(HOST, workflowText, bare, gateSources, reasonInput).some(error =>
      error.includes('names no gate site'),
    ),
    'a local claim without evidence must be reported',
  );
});

test('a reason outside the closed set is a label, not a reason', async () => {
  const {workflowText, gateSources, reasonInput} = await load();
  const injected = registryOf(copy => {
    entry(copy, 'Full three-language contract suite (Linux C++ toolchain)').reason = 'because';
  });
  assert.ok(
    workflowStepErrors(HOST, workflowText, injected, gateSources, reasonInput).some(error =>
      error.includes('not in the cause table'),
    ),
    'an unknown cause must be named',
  );
  // 非空洞：闭集**逐格**都有实例——某一格被合法清空 ⇒ 它的规则对空集恒真，
  // 而「闭集非空」这条弱断言**照样绿**（W108 之前正是这样）。
  for (const cause of CI_ONLY_REASONS) {
    assert.ok(
      WORKFLOW_STEPS.some(item => item.reason === cause),
      `the ${cause} cause must have at least one step, otherwise its rules are vacuous`,
    );
  }
});

test('a ci-only step must not also claim local coverage', async () => {
  const {workflowText, gateSources, reasonInput} = await load();
  const injected = registryOf(copy => {
    entry(copy, 'pnpm spec:validate').localEvidence = {file: GATE, marker: '["check", "--profile", "full"]'};
  });
  assert.ok(
    workflowStepErrors(HOST, workflowText, injected, gateSources, reasonInput).some(error =>
      error.includes('covered locally and only-in-CI at once'),
    ),
    'a step cannot be local and ci-only at once',
  );
});

test('infra must not be a back door around the checks', async () => {
  const {workflowText, gateSources, reasonInput} = await load();
  const injected = registryOf(copy => {
    // 这一步的 `run` 首行是 `node --test …`，形状上就是一条判定 ⇒ 标成 `infra` 必须被拦。
    entry(copy, SCHEMA_STEP).kind = 'infra';
  });
  assert.ok(
    workflowStepErrors(HOST, workflowText, injected, gateSources, reasonInput).some(error =>
      error.includes('looks like a judgement'),
    ),
    'a judgement disguised as infra must be reported',
  );
});

test('a diagnostic step is recognised by its condition, not by its name', async () => {
  const {workflowText, gateSources, reasonInput} = await load();
  const dropped = workflowText.replace('        if: failure()\n', '');
  assert.ok(
    workflowStepErrors(HOST, dropped, WORKFLOW_STEPS, gateSources, reasonInput).some(error =>
      error.includes('no failure condition'),
    ),
    'a diagnostic without a failure condition must be reported',
  );
  // 反向：`if: runner.os == 'Linux'` 是平台条件，不是失败条件。
  const platform = registryOf(copy => {
    entry(copy, 'Install Linux C++ protobuf runtime').kind = 'diagnostic';
  });
  assert.ok(
    workflowStepErrors(HOST, workflowText, platform, gateSources, reasonInput).some(error =>
      error.includes('no failure condition'),
    ),
    'a platform condition must not pass as a failure condition',
  );
});

test('a legitimate registration is not reported', async () => {
  const {workflowText, gateSources, reasonInput} = await load();
  // 反向：把凭据换成**另一处真的存在**的门控落点 ⇒ MUST NOT 红（判据验「存在」，不验「等于某串」）。
  const other = registryOf(copy => {
    entry(copy, GATE_STEP).localEvidence = {
      file: GATE,
      marker: '["check", "--profile", "full"]',
    };
  });
  assert.deepEqual(
    workflowStepErrors(HOST, workflowText, other, gateSources, reasonInput),
    [],
    'a different but existing gate site must not be reported',
  );
});

test('an unparseable workflow or an empty step set is named, never skipped', async () => {
  const {gateSources, reasonInput} = await load();
  const broken = workflowStepErrors(HOST, 'jobs: [1, 2', WORKFLOW_STEPS, gateSources, reasonInput);
  assert.ok(
    broken.some(error => error.includes('could not be parsed')),
    'an unparseable workflow must be named rather than silently pass',
  );
  const empty = workflowStepErrors(HOST, 'jobs:\n  x:\n    steps: []\n', WORKFLOW_STEPS, gateSources, reasonInput);
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

test('a ci-only cause must pay for itself with a receipt the repository can show', async () => {
  const {workflowText, gateSources, reasonInput} = await load();
  // 前提：出货的兑付表是干净的——否则下面的注入没有对照物。
  assert.deepEqual(
    workflowStepErrors(HOST, workflowText, WORKFLOW_STEPS, gateSources, reasonInput),
    [],
    'the shipped cause payment table must be clean',
  );
  // 兑付表的**注入位**：表本身是模块常量，没有这一层就构造不出「少一格／多一格／凭据指错」。
  const paid = mutate => {
    const copy = structuredClone(CI_ONLY_REASON_PAYMENTS);
    mutate(copy);
    return {...reasonInput, payments: copy};
  };
  const reports = (input, needle) =>
    workflowStepErrors(HOST, workflowText, WORKFLOW_STEPS, gateSources, input).some(error =>
      error.includes(needle),
    );

  // 注入一：凭据指向一串**文件里不存在**的文本 ⇒ 凭据是一句空话。
  // ⚠️ 探针值先确认过它**不在**那个文件里（W93 教训）。
  assert.ok(
    reports(
      paid(p => {
        p['compiled-artifact'].evidence[0].marker = 'no-such-step-receipt';
      }),
      'the receipt is a claim, not a fact',
    ),
    'a receipt that exists nowhere must be reported',
  );
  // 注入二：凭据住在一个**未被 git 跟踪**的路径上 ⇒ 能读到内容也不是仓库事实。
  assert.ok(
    reports(
      paid(p => {
        p['compiled-artifact'].evidence[0].file = 'target/w108-untracked.rs';
      }),
      'the repository does not track',
    ),
    'a receipt outside the repository must be reported',
  );
  // 注入三：凭据由**被审注册表自己**写 ⇒ 对表恒真，自我满足。
  assert.ok(
    reports(
      paid(p => {
        p['compiled-artifact'].evidence[0].file = 'scripts/contracts/workflow-steps.mjs';
      }),
      'true by construction',
    ),
    'a receipt written by the registry itself must be reported',
  );
  // 注入四：删掉一整格 ⇒ **欠付**（有人声明、没人付）。
  assert.ok(
    reports(
      paid(p => {
        delete p['not-in-profile'];
      }),
      'no payment says what it stands on',
    ),
    'a cause with no payment must be reported',
  );
  // 注入五：多出一格不在闭集里的成因 ⇒ **空规格**（有人付、没对象）。反方向，独立具名。
  assert.ok(
    reports(
      paid(p => {
        p['w108-surplus-step'] = {evidence: [...p['not-in-profile'].evidence]};
      }),
      'declares a payment but is not in the cause table',
    ),
    'a payment with no cause must be reported',
  );
  // 注入六：凭据不答任何问题（`why` 为空）⇒ 装饰。
  assert.ok(
    reports(
      paid(p => {
        p['compiled-artifact'].evidence[0].why = '';
      }),
      'does not say what it answers',
    ),
    'a receipt with no claim must be reported',
  );
  // 注入七：某格**零实例**（把该格的全部条目改归别因）⇒ 它的规则对空集恒真。
  const emptied = registryOf(copy => {
    for (const item of copy) {
      if (item.reason === 'not-in-profile') item.reason = 'platform-toolchain';
    }
  });
  assert.ok(
    workflowStepErrors(HOST, workflowText, emptied, gateSources, reasonInput).some(error =>
      error.includes('vacuously true'),
    ),
    'an empty cause cell must be reported',
  );
  // 注入八：缺输入 ⇒ **具名失败**，MUST NOT 静默跳过。
  assert.ok(
    workflowStepErrors(HOST, workflowText, WORKFLOW_STEPS, gateSources, {}).some(error =>
      error.includes('evidence sources were not provided'),
    ),
    'a missing evidence source set must be named rather than skipped',
  );
  assert.ok(
    workflowStepErrors(HOST, workflowText, WORKFLOW_STEPS, gateSources, {
      sources: reasonInput.sources,
    }).some(error => error.includes('tracked-path set was not provided')),
    'a missing tracked-path set must be named rather than skipped',
  );
  // 反向：把凭据换成**另一个真的存在**的串 ⇒ MUST NOT 红。
  assert.deepEqual(
    workflowStepErrors(
      HOST,
      workflowText,
      WORKFLOW_STEPS,
      gateSources,
      paid(p => {
        p['compiled-artifact'].evidence[0].marker = 'needs protoc';
      }),
    ),
    [],
    'a different but existing receipt must not be reported',
  );
});
