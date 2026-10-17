import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { membersFromToml, packageNames } from '../../scripts/contracts/gate-coverage.mjs';
import { WORKFLOW_STEPS } from '../../scripts/contracts/workflow-steps.mjs';
import {
  COVERAGE_CLASSES,
  coverageClassOf,
  gateCoverageOf,
  suiteCoverageOf,
  stepCoverageErrors,
} from '../../scripts/contracts/step-overlap.mjs';

const HOST = '.github/workflows/foundation.yml';
const HARNESS = 'xtask/src/lib.rs';

/// 判据的输入**全部显式注入**：本模块自己不读磁盘（遍历工作区是 W75 的 F2 根因）。
async function load() {
  const members = membersFromToml(await readFile('Cargo.toml', 'utf8'));
  const memberTomls = new Map();
  for (const member of members) {
    memberTomls.set(member, await readFile(`${member}/Cargo.toml`, 'utf8'));
  }
  const names = packageNames(memberTomls);
  const packages = new Set(members.map(member => names.get(member)).filter(Boolean));
  const harnessText = await readFile(HARNESS, 'utf8');
  return {
    host: HOST,
    workflowText: await readFile(HOST, 'utf8'),
    harnessText,
    packages,
    // `HARNESS` 是源码里的常量 ⇒ 从 xtask 自己的包名派生，**不写死**（W93）。
    identifiers: new Map([['HARNESS', names.get('xtask')]]),
  };
}

/// 一份**最小的** workflow 文本：只有判据真正读的字段（`name` / `run`）。
/// 缺字段会让 YAML 解析失败 ⇒ 那成了「输入构造坏了」，会把判据该报的缺陷伪装成测试自己坏了。
function workflowOf(steps) {
  const lines = ['on: {push: {}}', 'jobs:', '  foundation:', '    runs-on: ubuntu-24.04', '    steps:'];
  for (const step of steps) {
    lines.push(`      - name: ${JSON.stringify(step.name)}`);
    lines.push(`        run: ${JSON.stringify(step.run)}`);
  }
  return `${lines.join('\n')}\n`;
}

function registryOf(...entries) {
  return entries.map(entry => ({ host: HOST, kind: 'local', ...entry }));
}

const GATE_STEP = {
  identity: 'Complete engineering foundation (portable steps + frontend workspace)',
  target: [{ shape: 'xtask', value: 'check --profile full' }],
};

test('the shipped baseline is clean, and every fact it needs was really provided', async () => {
  const inputs = await load();
  assert.deepEqual(stepCoverageErrors(inputs), [], 'the baseline must run no test twice');
  // 非空洞：注入进来的事实不许是空——否则每条判据都对空集恒真。
  assert.ok(inputs.packages.size > 0, 'the workspace package set must not be empty');
  assert.ok(inputs.identifiers.get('HARNESS') !== undefined, 'the harness package name must be known');
  assert.ok(WORKFLOW_STEPS.length > 0, 'the step registry must not be empty');
  assert.ok(COVERAGE_CLASSES.length > 0, 'the coverage class set must not be empty');
});

test('the gate coverage is a union, not its first test command', async () => {
  const inputs = await load();
  const coverage = gateCoverageOf(inputs.harnessText, inputs.packages, inputs.identifiers);
  // `engineering_checks` 有两条 `["test", …]`：workspace 全量，以及 harness 自己的 lib 测试。
  // 取第一条会少报 harness ⇒ 一条「只跑 harness」的步会被判成不重复（假阴性，方向不安全）。
  assert.ok(inputs.packages.has('nai-kernel'), 'the workspace run must reach the kernel package');
  assert.ok(
    coverage.has(inputs.identifiers.get('HARNESS')),
    'the gate coverage must include the harness package, which its own lib test covers',
  );
  for (const name of inputs.packages) {
    assert.ok(coverage.has(name), `the gate coverage must reach the workspace member ${name}`);
  }
});

test('a suite whose packages the gate already runs is named as a second execution', async () => {
  const inputs = await load();
  const registry = registryOf(GATE_STEP, {
    identity: 'probe duplicated suite',
    target: [{ shape: 'xtask', value: 'test --suite storage' }],
  });
  const text = workflowOf([
    { name: GATE_STEP.identity, run: 'cargo xtask check --profile full' },
    { name: 'probe duplicated suite', run: 'cargo xtask test --suite storage' },
  ]);
  const errors = stepCoverageErrors({ ...inputs, workflowText: text, registered: registry });
  assert.equal(errors.length, 1, `exactly one duplicate must be named, got ${JSON.stringify(errors)}`);
  assert.match(errors[0], /runs the same tests twice/);
  // 判据 MUST 说出**是哪两条步**，以及**哪个集合**：只有「有重复」这四个字无法定位。
  assert.match(errors[0], /probe duplicated suite/);
  assert.match(errors[0], /Complete engineering foundation/);
  assert.match(errors[0], /nai-storage/);
});

test('a suite the gate does not cover is not a duplicate', async () => {
  const inputs = await load();
  // `contracts` 走 `schema::contracts`（C++ 夹具），不是 cargo 的包测试集合
  // ⇒ 它归 `non-cargo-suite`，不参与包含关系。这条用例是上面那条的**限定词**：
  // 若判据把所有 suite 都当成「被门控覆盖」，它会在这里假红。
  const registry = registryOf(GATE_STEP, {
    identity: 'probe disjoint suite',
    target: [{ shape: 'xtask', value: 'test --suite contracts' }],
  });
  const text = workflowOf([
    { name: GATE_STEP.identity, run: 'cargo xtask check --profile full' },
    { name: 'probe disjoint suite', run: 'cargo xtask test --suite contracts' },
  ]);
  assert.deepEqual(stepCoverageErrors({ ...inputs, workflowText: text, registered: registry }), []);
});

test('two suite steps that overlap each other are named, even without the gate', async () => {
  const inputs = await load();
  // 判据 MUST NOT 只在「与门控比」时有效：两条 suite 步互相包含同样是重复执行。
  const registry = registryOf(
    { identity: 'probe storage suite', target: [{ shape: 'xtask', value: 'test --suite storage' }] },
    { identity: 'probe kernel suite', target: [{ shape: 'xtask', value: 'test --suite kernel' }] },
  );
  const text = workflowOf([
    { name: 'probe storage suite', run: 'cargo xtask test --suite storage' },
    { name: 'probe kernel suite', run: 'cargo xtask test --suite kernel' },
  ]);
  const errors = stepCoverageErrors({ ...inputs, workflowText: text, registered: registry });
  // `storage` 与 `kernel` 的包集不相交 ⇒ **不是**重复。这一条是**反例方向**：
  // 判据若把「同为 cargo-suite」当成重复，这里会假红。
  assert.deepEqual(errors, []);
});

test('a target outside the coverage classes is named, never skipped', async () => {
  const inputs = await load();
  // 门控步也在场：否则「一条都没判到」会额外报一条空集错误，把这条用例的意图搅浑。
  const registry = registryOf(GATE_STEP, {
    identity: 'probe unknown target',
    target: [{ shape: 'python', value: 'pytest tests' }],
  });
  const text = workflowOf([
    { name: GATE_STEP.identity, run: 'cargo xtask check --profile full' },
    { name: 'probe unknown target', run: 'pytest tests' },
  ]);
  const errors = stepCoverageErrors({ ...inputs, workflowText: text, registered: registry });
  assert.equal(errors.length, 1, `an unclassified target must be named, got ${JSON.stringify(errors)}`);
  assert.match(errors[0], /falls into no coverage class/);
  assert.equal(coverageClassOf({ shape: 'python', value: 'pytest tests' }), null);
});

test('every coverage class is reachable from the shipped registry', async () => {
  const inputs = await load();
  const kinds = new Set(
    WORKFLOW_STEPS.filter(entry => entry.host === HOST && Array.isArray(entry.target))
      .flatMap(entry => entry.target.map(target => coverageClassOf(target))),
  );
  // 非空洞：注册表里必须真的用到了这三格，否则对应分支对空集恒真。
  // ⚠️ `suite` 这一格在 W134 之后**只剩 `contracts` 一条**——那正是本轮删掉的
  // 「被门控覆盖的 cargo 套件」之外的唯一幸存者（它跑 C++ 夹具，门控不覆盖）。
  for (const kind of COVERAGE_CLASSES) {
    assert.ok(kinds.has(kind), `the shipped registry must still exercise the ${kind} class`);
  }
  for (const kind of kinds) {
    assert.ok(COVERAGE_CLASSES.includes(kind), `the class ${kind} must be in the closed set`);
  }
});

test('missing inputs are named, never silently downgraded', async () => {
  const inputs = await load();
  assert.match(
    stepCoverageErrors({ ...inputs, harnessText: '' })[0],
    /requires the harness source/,
  );
  assert.match(
    stepCoverageErrors({ ...inputs, packages: new Set() })[0],
    /requires the workspace package set/,
  );
  assert.match(
    stepCoverageErrors({ ...inputs, workflowText: 'jobs: [\n' })[0],
    /could not be parsed/,
  );
  assert.match(
    stepCoverageErrors({ ...inputs, registered: [] })[0],
    /requires a non-empty registered step set/,
  );
});

test('a step whose suite cannot be resolved is named, not silently empty', async () => {
  const inputs = await load();
  // 一个不存在的 suite 名 ⇒ `suiteArmBody` 抛 ⇒ 判据 MUST 把它变成一条**具名错误**。
  // 若这里返回空数组，就等于把「读不懂」当成「没有覆盖」——空集对包含关系恒真。
  const registry = registryOf(GATE_STEP, {
    identity: 'probe phantom suite',
    target: [{ shape: 'xtask', value: 'test --suite phantom' }],
  });
  const text = workflowOf([
    { name: GATE_STEP.identity, run: 'cargo xtask check --profile full' },
    { name: 'probe phantom suite', run: 'cargo xtask test --suite phantom' },
  ]);
  const errors = stepCoverageErrors({ ...inputs, workflowText: text, registered: registry });
  assert.equal(errors.length, 1, `an unreadable coverage must be named, got ${JSON.stringify(errors)}`);
  assert.match(errors[0], /unreadable coverage/);
  assert.match(errors[0], /no dispatch arm for the suite "phantom"/);
});

test('the suite coverage of a shipped suite is its own package set', async () => {
  const inputs = await load();
  const storage = suiteCoverageOf(inputs.harnessText, 'storage', inputs.packages, inputs.identifiers);
  assert.deepEqual([...storage].sort(), ['nai-security', 'nai-storage']);
  // 限定词的作用域：`storage` 的集合 MUST 被门控覆盖（本轮删掉的分套件步正是这个关系）。
  const gate = gateCoverageOf(inputs.harnessText, inputs.packages, inputs.identifiers);
  for (const name of storage) {
    assert.ok(gate.has(name), `the gate must cover ${name}, otherwise the removed suite step was not redundant`);
  }
});

test('an empty judged set is named, never vacuously clean', async () => {
  const inputs = await load();
  // 只有 `non-test-target` 步 ⇒ 一次包含关系都没判过。若返回 `[]`，
  // 「没有重复」这句话对空集**恒真**——这正是「短路把未检查伪装成已通过」的形状。
  const registry = registryOf({
    identity: 'probe schemas',
    kind: 'ci-only',
    target: [{ shape: 'xtask', value: 'schema --check' }],
  });
  const text = workflowOf([{ name: 'probe schemas', run: 'cargo xtask schema --check' }]);
  const errors = stepCoverageErrors({ ...inputs, workflowText: text, registered: registry });
  assert.equal(errors.length, 1, `an empty judged set must be named, got ${JSON.stringify(errors)}`);
  assert.match(errors[0], /no judged step with a coverage/);
});

test('a gate region that runs no cargo test is named', async () => {
  const inputs = await load();
  // 门控区域的形状变了（没有人再跑 `["test", …]`）⇒ 门控覆盖是**空**的。
  // 空覆盖会让**任何**步都被判成「被门控覆盖」（假红）或「都没被覆盖」（假绿），
  // 两种都错 ⇒ MUST 上抛成一条具名错误。
  //
  // ⚠️ 这份假宿主 MUST 保留 `["check", "--profile", …]` 臂：`gateRegions` 用它定位门控面，
  // 没有它抛的是**另一条**错误（「would be half-read」）——那样注入走不到我要的那一层。
  const bare = [
    'fn engineering_checks(root: &Path) -> Result<(), String> {',
    '    Ok(())',
    '}',
    'pub fn execute(args: &[String], root: &Path) -> Result<(), String> {',
    '    match args {',
    '        ["check", "--profile", "full"] => {',
    '            engineering_checks(root)?;',
    '            Ok(())',
    '        }',
    '        _ => Ok(()),',
    '    }',
    '}',
  ].join('\n');
  assert.throws(
    () => gateCoverageOf(bare, inputs.packages, inputs.identifiers),
    /runs no cargo test command/,
  );
  assert.throws(
    () => gateCoverageOf('fn something_else() {}', inputs.packages, inputs.identifiers),
    /no `fn engineering_checks\(`/,
  );
});
