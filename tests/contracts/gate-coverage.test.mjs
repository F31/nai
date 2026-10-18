import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import {
  SUITE_DRIVERS,
  COVERAGE_KINDS,
  TEST_SUITES,
  membersFromToml,
  packageNames,
  topLevelItems,
  testCoverage,
  resolveCoverage,
  coveragesOf,
  testCommandsOf,
  functionBody,
  ciXtaskCommands,
  suiteRegistryShapeErrors,
  prebuildRecipeErrors,
  documentedPrebuildSteps,
  commandSignature,
  PREBUILD_BLOCK,
  coverageErrors,
} from '../../scripts/contracts/gate-coverage.mjs';

const ROOT_TOML = 'Cargo.toml';
const HARNESS = 'xtask/src/lib.rs';
const GATE_SCHEMA = 'xtask/src/schema.rs';
const WORKFLOW = '.github/workflows/foundation.yml';
const ENGINEERING_DOC = 'docs/engineering.md';

/// 判据的输入**全部显式注入**：本模块自己不读磁盘（遍历工作区是 W75 的 F2 根因）。
async function load() {
  const rootToml = await readFile(ROOT_TOML, 'utf8');
  const members = membersFromToml(rootToml);
  const memberTomls = new Map();
  for (const member of members) {
    memberTomls.set(member, await readFile(`${member}/Cargo.toml`, 'utf8'));
  }
  const harnessText = await readFile(HARNESS, 'utf8');
  return {
    rootToml,
    memberTomls,
    harnessText,
    gateSources: new Map([
      [HARNESS, harnessText],
      [GATE_SCHEMA, await readFile(GATE_SCHEMA, 'utf8')],
    ]),
    workflowText: await readFile(WORKFLOW, 'utf8'),
    documentText: await readFile(ENGINEERING_DOC, 'utf8'),
    // 标识符的值**从包清单派生**，不写死：`HARNESS` 的真源是 `xtask/Cargo.toml` 的 `name`。
    identifiers: new Map([['HARNESS', packageNames(memberTomls).get('xtask')]]),
  };
}

function withInputs(inputs, patch) {
  return {...inputs, ...patch};
}

function registryOf(...mutations) {
  const copy = structuredClone(TEST_SUITES);
  for (const mutate of mutations) mutate(copy);
  return copy;
}

const WORKSPACE_RUN = '&["test", "--workspace", "--locked", "--exclude", HARNESS],';

test('the shipped baseline is clean, and every fact it needs was really provided', async () => {
  const inputs = await load();
  assert.deepEqual(coverageErrors(inputs), [], 'the baseline must carry no error');
  // 非空洞：注入进来的事实不许是空——否则每条判据都对空集恒真。
  assert.ok(inputs.memberTomls.size > 0, 'the member manifests must not be empty');
  assert.ok(packageNames(inputs.memberTomls).size > 0, 'at least one package must be known');
  assert.ok(TEST_SUITES.length > 0, 'the suite registry must not be empty');
  assert.ok(SUITE_DRIVERS.length > 0, 'the driver closed set must not be empty');
  assert.ok(COVERAGE_KINDS.length > 0, 'the coverage closed set must not be empty');
  assert.ok(ciXtaskCommands(inputs.workflowText).length > 0, 'the CI workflow must call something');
});

test('a suite that is dispatched but not declared is named', async () => {
  const inputs = await load();
  const dropped = registryOf(copy => {
    const at = copy.findIndex(entry => entry.suite === 'storage');
    copy.splice(at, 1);
  });
  assert.ok(
    coverageErrors(withInputs(inputs, {suiteRegistry: dropped})).some(error =>
      error.includes('the suite "storage"') && error.includes('nobody declares'),
    ),
    'a dispatched suite with no declaration must be named',
  );
});

test('a declared suite that no longer exists is named', async () => {
  const inputs = await load();
  const added = registryOf(copy => copy.push({suite: 'nonexistent', driver: 'inline'}));
  assert.ok(
    coverageErrors(withInputs(inputs, {suiteRegistry: added})).some(error =>
      error.includes('the suite "nonexistent"') && error.includes('silently disappeared'),
    ),
    'a registered suite that vanished must be named',
  );
});

test('a driver outside the closed set is a label, not a classification', async () => {
  const inputs = await load();
  const bad = registryOf(copy => {
    copy[0].driver = 'whatever';
  });
  assert.ok(
    coverageErrors(withInputs(inputs, {suiteRegistry: bad})).some(error => error.includes('not a suite driver')),
    'an unknown driver must be named',
  );
});

test('an inline suite must not also name a delegation target', async () => {
  const inputs = await load();
  const both = registryOf(copy => {
    const entry = copy.find(item => item.suite === 'kernel');
    entry.file = GATE_SCHEMA;
  });
  assert.ok(
    coverageErrors(withInputs(inputs, {suiteRegistry: both})).some(error =>
      error.includes('is inline yet also names a delegation target'),
    ),
    'the packages cannot be computed from two places',
  );
});

test('a delegated suite must name both its file and its function', async () => {
  const inputs = await load();
  const noFile = registryOf(copy => {
    delete copy.find(item => item.suite === 'contracts').file;
  });
  assert.ok(
    coverageErrors(withInputs(inputs, {suiteRegistry: noFile})).some(error =>
      error.includes('names no file'),
    ),
    'a delegation without a file must be named',
  );
  const noFn = registryOf(copy => {
    delete copy.find(item => item.suite === 'contracts').fn;
  });
  assert.ok(
    coverageErrors(withInputs(inputs, {suiteRegistry: noFn})).some(error =>
      error.includes('names no function'),
    ),
    'a delegation without a function must be named',
  );
});

test('a delegated suite whose source was not provided is named, never skipped', async () => {
  const inputs = await load();
  const withoutSchema = new Map([...inputs.gateSources].filter(([path]) => path !== GATE_SCHEMA));
  assert.ok(
    coverageErrors(withInputs(inputs, {gateSources: withoutSchema})).some(error =>
      error.includes('the suite "contracts"') && error.includes('not provided for judgement'),
    ),
    'a missing delegated source must be named rather than silently lower the coverage',
  );
});

test('a phantom package reference is named', async () => {
  const inputs = await load();
  const renamed = inputs.harnessText.replace('"nai-security"', '"nai-phantom"');
  assert.ok(
    coverageErrors(withInputs(inputs, {harnessText: renamed})).some(error =>
      error.includes('"nai-phantom"') && error.includes('not a package of this workspace'),
    ),
    'a suite referencing a package that is not in the workspace must be named',
  );
});

test('shrinking the gate to one package breaks every suite by name', async () => {
  const inputs = await load();
  const shrunk = inputs.harnessText.replace(
    WORKSPACE_RUN,
    '&["test", "--locked", "--package", "nai-storage"],',
  );
  assert.notEqual(shrunk, inputs.harnessText, 'the injection must really change the harness');
  const errors = coverageErrors(withInputs(inputs, {harnessText: shrunk}));
  // 每个失去覆盖的 suite 都被点名，而不是笼统地报一次。
  for (const suite of ['contracts', 'knowledge', 'provider', 'kernel', 'hosts', 'upgrade']) {
    assert.ok(
      errors.some(error => error.includes(`the suite "${suite}"`)),
      `losing coverage must name the suite "${suite}"`,
    );
  }
  assert.ok(
    errors.some(error => error.includes('tested by no command')),
    'members that are no longer run must be named too',
  );
});

test('coverage by a union of commands is not coverage', async () => {
  const inputs = await load();
  // 把全 workspace 那条拆成两条各测一半：`kernel` 的包集只能被**并集**覆盖 ⇒ 必须红。
  const split = inputs.harnessText.replace(
    WORKSPACE_RUN,
    '&["test", "--locked", "--package", "nai-kernel"],\n        &["test", "--locked", "--package", "nai-plugin-sdk"],',
  );
  const errors = coverageErrors(withInputs(inputs, {harnessText: split}));
  assert.ok(
    errors.some(error => error.includes('the suite "kernel"') && error.includes('union')),
    `a suite covered only by a union must be named, saw ${JSON.stringify(errors.filter(e => e.includes('union')))}`,
  );
});

test('a workspace run adapts to new members; an explicit list that forgets one is named', async () => {
  const inputs = await load();
  const memberTomls = new Map([...inputs.memberTomls, ['crates/nai-newcomer', 'name = "nai-newcomer"\n']]);
  const extra = inputs.rootToml.replace(', "xtask"]', ', "xtask", "crates/nai-newcomer"]');
  assert.notEqual(extra, inputs.rootToml, 'the injection must really add a member');

  // 情形 A：`--workspace` 形态**自适应**于成员集合——`cargo test --workspace` 真的会跑到新成员。
  // ⇒ 这不是缝隙，判据 MUST NOT 报警（把「自适应」误报成「没覆盖」会把工具教坏）。
  assert.deepEqual(
    coverageErrors(withInputs(inputs, {rootToml: extra, memberTomls})),
    [],
    'a workspace run must cover a newly added member without any change to the registry',
  );

  // 情形 B：门控改用**显式**包列表并漏掉某个成员 ⇒ 那个成员 MUST 被点名。
  // 这是 V5 真正有懈可乘的地方：`workspace` 形态消失后，成员集合与列表不再自动对齐。
  const explicit = inputs.harnessText.replace(
    WORKSPACE_RUN,
    '&["test", "--locked", "--package", "nai-kernel"],',
  );
  const errors = coverageErrors(withInputs(inputs, {
    rootToml: extra,
    memberTomls,
    harnessText: explicit,
  }));
  assert.ok(
    errors.some(error => error.includes('crates/nai-newcomer') && error.includes('tested by no command')),
    `a member an explicit list forgot must be named, saw ${JSON.stringify(errors.filter(e => e.includes('no command')))}`,
  );
});

test('a CI command that the harness does not dispatch is named', async () => {
  const inputs = await load();
  const anchor = '      - name: Validate schemas and API contract\n';
  const ghost = inputs.workflowText.replace(
    anchor,
    '      - name: W97 ghost probe\n        run: cargo xtask check --audit\n      - name: Validate schemas and API contract\n',
  );
  // 锚点 MUST 真的命中：锚在一处**已改名/已删除**的步上时，`replace` 是静默 no-op，
  // 于是注入根本没发生、断言却照样跑——W134 删掉 7 条分套件步时正是这样红了两条。
  assert.notEqual(ghost, inputs.workflowText, `the anchor step must exist in ${'.github/workflows/foundation.yml'}`);
  assert.ok(
    coverageErrors(withInputs(inputs, {workflowText: ghost})).some(error =>
      error.includes('check --audit') && error.includes('does not dispatch'),
    ),
    'a CI step calling a command that does not exist must be named',
  );
});

test('shell syntax around a CI command must not change its identity', async () => {
  const inputs = await load();
  // 同一条命令带上 `2>&1 | tee …` 与一个 defanging 后缀，签名必须还是 `check --profile full`。
  const anchor = '      - name: Validate schemas and API contract\n';
  const piped = inputs.workflowText.replace(
    anchor,
    '      - name: W97 piped probe\n        run: cargo xtask check --profile full 2>&1 | tee "$RUNNER_TEMP/x.log"\n      - name: Validate schemas and API contract\n',
  );
  assert.notEqual(piped, inputs.workflowText, 'the anchor step must exist in the workflow');
  const commands = ciXtaskCommands(piped);
  assert.ok(commands.includes('check --profile full'), 'a piped command must normalise to its signature');
  const errors = coverageErrors(withInputs(inputs, {workflowText: piped}));
  assert.ok(
    !errors.some(error => error.includes('check --profile full')),
    `the normalised command must be recognised, saw ${JSON.stringify(errors)}`,
  );
});

test('a member manifest that was not provided is named', async () => {
  const inputs = await load();
  const key = [...inputs.memberTomls.keys()][0];
  const memberTomls = new Map([...inputs.memberTomls].filter(([path]) => path !== key));
  assert.ok(
    coverageErrors(withInputs(inputs, {memberTomls})).some(error =>
      error.includes(key) && error.includes('has no manifest among the provided ones'),
    ),
    'a missing member manifest must be named rather than shrink the judged set',
  );
});

test('an operand that cannot be resolved is a failure, not an empty set', async () => {
  const inputs = await load();
  // 把 `HARNESS` 换成另一个没人解析得出的标识符： bromine ⇒ 覆盖多算一个包 ⇒ 掩盖缝隙。
  const unresolved = inputs.harnessText.replace('--exclude", HARNESS]', '--exclude", SOMETHING_ELSE]');
  assert.ok(
    coverageErrors(withInputs(inputs, {harnessText: unresolved})).some(error =>
      error.includes('SOMETHING_ELSE') && error.includes('resolved'),
    ),
    'an unresolvable operand must be named',
  );
  // 反向：显式给出映射 ⇒ 解析成功，且这一条随之消失。
  const supplied = withInputs(inputs, {
    harnessText: unresolved,
    identifiers: new Map([['SOMETHING_ELSE', 'nai-xtask']]),
  });
  assert.ok(
    !coverageErrors(supplied).some(error => error.includes('SOMETHING_ELSE')),
    'a supplied identifier must resolve',
  );
});

test('a missing input is named, never silently downgraded', async () => {
  const inputs = await load();
  assert.ok(
    coverageErrors({...inputs, harnessText: ''}).some(error => error.includes('no suite can be judged')),
    'an absent harness must be named',
  );
  assert.ok(
    coverageErrors({...inputs, workflowText: ''}).some(error => error.includes('cannot be judged')),
    'an absent workflow must be named',
  );
  assert.ok(
    coverageErrors({...inputs, gateSources: null}).some(error => error.includes('as a map')),
    'a non-map gate source must be named',
  );
  assert.ok(
    coverageErrors({...inputs, memberTomls: null}).some(error => error.includes('as a map')),
    'a non-map member manifest set must be named',
  );
  assert.ok(
    coverageErrors({...inputs, rootToml: 'nonsense'}).some(error => error.includes('declares no `members`')),
    'a manifest without members must be named',
  );
  assert.ok(
    coverageErrors({}).some(error => error.includes('was not provided')),
    'calling the judge with nothing must be named',
  );
});

test('an empty registry or empty suites make the closure vacuous, and that is named', async () => {
  const inputs = await load();
  const empty = coverageErrors(withInputs(inputs, {suiteRegistry: []}));
  assert.ok(empty.some(error => error.includes('registry is empty')), 'an empty registry must be named');
  assert.ok(
    empty.some(error => error.includes('nobody declares')),
    'every dispatched suite must be reported as undeclared too',
  );
});

test('a duplicate suite declaration is named', async () => {
  const inputs = await load();
  const twice = registryOf(copy => copy.push({...copy.find(item => item.suite === 'kernel')}));
  assert.ok(
    coverageErrors(withInputs(inputs, {suiteRegistry: twice})).some(error =>
      error.includes('is declared twice'),
    ),
    'one suite must have one declaration',
  );
});

test('the coverage of a test command is computed from its operands', () => {
  // `--workspace` ⇒ 全集（可经 `--exclude` 挖洞）。
  assert.deepEqual(testCoverage(['test', '--workspace', '--locked', '--exclude', 'nai-xtask']), {
    kind: 'workspace',
    packages: [],
    excludes: ['nai-xtask'],
    raw: 'test --workspace --locked --exclude nai-xtask',
  });
  // `-p` / `--package` ⇒ 显式列表。`--lib` 约束目标类型，不改变包集合。
  assert.deepEqual(testCoverage(['test', '--locked', '-p', 'a', '--package', 'b', '--lib']), {
    kind: 'packages',
    packages: ['a', 'b'],
    excludes: [],
    raw: 'test --locked -p a --package b --lib',
  });
  assert.equal(testCoverage(['fmt', '--all', '--check']), null, 'a non-test command is not a coverage');
  const all = new Set(['a', 'b', 'c']);
  assert.deepEqual(
    resolveCoverage(testCoverage(['test', '--workspace', '--exclude', 'c']), all),
    new Set(['a', 'b']),
    'an excluded package must be removed from the workspace set',
  );
  assert.deepEqual(
    resolveCoverage(testCoverage(['test', '-p', 'a']), all),
    new Set(['a']),
    'an explicit list is used verbatim',
  );
});

test('array items are classified, and an unknown shape is not silently dropped', () => {
  assert.deepEqual(
    topLevelItems('"test", HARNESS, ok(1)').map(item => item.kind),
    ['string', 'identifier', 'unknown'],
  );
  const {unresolvable} = coveragesOf('&["test", "--locked", ok(1)]', new Map());
  assert.equal(unresolvable.length, 1, 'an unknown operand shape must be reported');
  assert.deepEqual(unresolvable[0].token, 'ok(1)');
});

test('the gate really runs both coverage kinds', async () => {
  const inputs = await load();
  // 非空洞：把 `--workspace` 那条拿掉 ⇒ `workspace` 形态消失 ⇒ 它的规则对空集恒真 ⇒ 必须红。
  const gone = inputs.harnessText.replace(WORKSPACE_RUN, '');
  const errors = coverageErrors(withInputs(inputs, {harnessText: gone}));
  assert.ok(
    errors.some(error => error.includes('is of the "workspace" kind')),
    `losing a coverage kind must be named, saw ${JSON.stringify(errors)}`,
  );
});

test('the registry really exercises both suite drivers', async () => {
  const inputs = await load();
  const inlineOnly = registryOf(copy => {
    const at = copy.findIndex(entry => entry.suite === 'contracts');
    copy[at] = {suite: 'contracts', driver: 'inline'};
  });
  const errors = coverageErrors(withInputs(inputs, {suiteRegistry: inlineOnly}));
  assert.ok(
    errors.some(error => error.includes('no suite is of the "delegated" driver')),
    'emptying a driver must be named rather than quietly drop its rule',
  );
  // 反向：`contracts` 标成 inline 后包集不在 arm 体里 ⇒ 它自己的包集算不出来。
  assert.ok(
    errors.some(error => error.includes('the suite "contracts"')),
    'mislabelling the driver must surface on the suite itself',
  );
});

test('prose is not operands: a bracket shape inside a comment must not be parsed', async () => {
  const inputs = await load();
  // 注释里写一个 `["test", "…"]` 的形状是很自然的写法；判据 MUST 把它当散文，不当实参。
  const prose = inputs.harnessText.replace(
    'fn engineering_checks(root: &Path) -> Result<(), String> {',
    'fn engineering_checks(root: &Path) -> Result<(), String> {\n    // 举例：这里本该有一条 `["test", "--workspace"]` 却故意不写。\n    /* 块注释同理：["test", "-p", "none"] */',
  );
  assert.deepEqual(
    coverageErrors(withInputs(inputs, {harnessText: prose})),
    [],
    'a commented-out example must not change the computed coverage',
  );
  // 反向：字符串字面量里的 `//` MUST NOT 被当成注释起点（Windows 下常见 PATH 写法）。
  const url = inputs.harnessText.replace(
    'fn engineering_checks(root: &Path) -> Result<(), String> {',
    'fn engineering_checks(root: &Path) -> Result<(), String> {\n    let _note = "https://example.test/x";',
  );
  assert.deepEqual(
    coverageErrors(withInputs(inputs, {harnessText: url})),
    [],
    'a string literal containing // must be preserved verbatim',
  );
});

test('shapes alone: a normalised CI command must be comparable with the dispatch', async () => {
  const inputs = await load();
  const commands = ciXtaskCommands(inputs.workflowText);
  for (const command of commands) {
    assert.ok(!/2>&1|\|\s*tee|&&|;/.test(command), `${command} must carry no shell syntax`);
  }
  // ⚠️ **W101 就地订正**：这里原先钉的是两个**具体串**
  // （`commands.includes('check --profile full') && commands.includes('schema --check')`）。
  // 那条断言既**假阳性**（把 `full` 合法改成别的 profile 就红），又**假阴性**
  // （换一条同形状的目标它看不见 —— W101 探针 W4 实测零信号）；而且它**违背 W97 自己的边界声明**
  // （「不验签名的取值」）。按对象性质分：**W97 管形状，W101 管取值绑定**
  // （`scripts/contracts/step-target.mjs` 把「这一步跑的是哪个目标」钉死在在册条目上）。
  assert.ok(
    commands.some(command => /^check --profile \S+$/.test(command)),
    'the gate-command shape must be recognised after normalisation',
  );
  assert.ok(
    commands.some(command => /^schema(?: --check)?$/.test(command)),
    'the schema-command shape must be recognised after normalisation',
  );
  assert.ok(
    commands.some(command => command.startsWith('test --suite ')),
    'the suite commands must be recognised too',
  );
});

// ---------- W147：预构建处方 ↔ 门控测试步 ----------
//
// 这一层回答的问题与上面所有层都不同：「改完代码先手工预构建，编的是不是门控要跑的那批单元」。
// W146 把它答成了「clippy 作废产物」，实测否证（clippy 不判脏；作废预构建的是 `--all-targets`
// 与 `cargo test` 的单元集不同）。判据不重述那个故事，只钉对应关系。

/// 只替换文档里**标记块内部**的命令行，块外的散文一字不动。
/// （直接在全文上做字符串替换会命中前面的说明文字，那是另一种假阳性。）
function withRecipe(inputs, mutate) {
  const text = inputs.documentText;
  const begin = text.indexOf(PREBUILD_BLOCK.begin) + PREBUILD_BLOCK.begin.length;
  const end = text.indexOf(PREBUILD_BLOCK.end);
  const lines = documentedPrebuildSteps(text);
  const patched = mutate([...lines]);
  const block = `\n\`\`\`bash\n${patched.join('\n')}\n\`\`\`\n`;
  return {...inputs, documentText: text.slice(0, begin) + block + text.slice(end)};
}

function gateStepsOf(harnessText) {
  // 与判据走同一个解析器：`functionBody` 由被测模块导出，测试不另写一份括号计数。
  return testCommandsOf(
    functionBody(harnessText, 'engineering_checks'),
    new Map([['HARNESS', 'nai-xtask']]),
  );
}

test('the shipped prebuild recipe is one-to-one with the gate test steps', async () => {
  const inputs = await load();
  assert.deepEqual(prebuildRecipeErrors(inputs), [], 'the shipped recipe must carry no error');
  // 非空洞：两侧都真的取到了东西，否则「双向一一对应」对空集恒真。
  const gate = gateStepsOf(inputs.harnessText);
  const recipe = documentedPrebuildSteps(inputs.documentText);
  assert.ok(gate.commands.length > 0, 'the gate must run test commands');
  assert.ok(recipe.length > 0, 'the document must carry a prebuild recipe');
  assert.equal(
    recipe.length,
    gate.commands.length,
    'a recipe line per gate test step — otherwise one of the two sides is judged vacuously',
  );
  for (const line of recipe) {
    assert.ok(line.includes('--no-run'), `${line} must be a prebuild, not the gate command itself`);
    assert.ok(!line.startsWith('```'), 'the fenced block must not be read as a command');
  }
  // 门控自己 MUST NOT 预构建：`--no-run` 只出现在处方里，出现两次就是两处定义。
  assert.ok(
    !inputs.harnessText.includes('"--no-run"'),
    'the gate must not prebuild — a recipe is a manual step',
  );
});

test('a recipe line without --no-run is named', async () => {
  const inputs = await load();
  const without = withRecipe(inputs, lines => {
    lines[0] = lines[0].replace(' --no-run', '');
    return lines;
  });
  assert.ok(
    prebuildRecipeErrors(without).some(error => error.includes('carries `--no-run` 0 time(s)')),
    'a prebuild that also runs the tests is not a prebuild',
  );
});

test('a recipe line that changes one flag matches no gate step', async () => {
  const inputs = await load();
  const drifted = withRecipe(inputs, lines => {
    lines[0] = lines[0].replace('--exclude', '--skip');
    return lines;
  });
  const errors = prebuildRecipeErrors(drifted);
  assert.ok(
    errors.some(error => error.includes('matches no test step of the gate')),
    `a drifted recipe must be named, saw ${JSON.stringify(errors)}`,
  );
  assert.ok(
    errors.some(error => error.includes('has no command for it')),
    'the gate step it drifted away from must be named too',
  );
});

test('a gate step that changes its coverage shape is named', async () => {
  const inputs = await load();
  // 门控侧把 `--workspace` 换成 `-p <一个包>`：包集合守卫（`coverageErrors`）会另有一整层
  // 报错，但那一层**看不见**「预构建处方已经对不上」——它们管的是不同的问题。
  const narrowed = inputs.harnessText.replace(
    WORKSPACE_RUN,
    '&["test", "-p", "nai-storage", "--locked"],',
  );
  assert.notEqual(narrowed, inputs.harnessText, 'the gate step must have been rewritten');
  const errors = prebuildRecipeErrors({...inputs, harnessText: narrowed});
  assert.ok(
    errors.some(error => error.includes('has no command for it')),
    `the recipe must be reported as stale, saw ${JSON.stringify(errors)}`,
  );
});

test('a gate step that prebuilds is named, not silently matched', async () => {
  const inputs = await load();
  // 门控自己带 `--no-run` ⇒ 「编译了什么」有两处定义，而处方存在的意义是把那个决定留在门控之外。
  const prebuilding = inputs.harnessText.replace(
    '&["test", "--locked", "--package", HARNESS, "--lib"]',
    '&["test", "--locked", "--package", HARNESS, "--lib", "--no-run"]',
  );
  assert.notEqual(prebuilding, inputs.harnessText, 'the gate step must have been rewritten');
  assert.ok(
    prebuildRecipeErrors({...inputs, harnessText: prebuilding}).some(error =>
      error.includes('belongs to the documented recipe, not to the gate'),
    ),
    'a prebuilding gate step must be named rather than matched',
  );
});

test('a missing marker, a duplicated marker and an empty block are distinguished', async () => {
  const inputs = await load();
  const noBegin = inputs.documentText.replace(PREBUILD_BLOCK.begin, '');
  assert.ok(
    prebuildRecipeErrors({...inputs, documentText: noBegin}).some(error =>
      error.includes('carries no'),
    ),
    'an unlocatable block must be named rather than read as an empty recipe',
  );
  const twice = inputs.documentText.replace(
    PREBUILD_BLOCK.end,
    `${PREBUILD_BLOCK.begin}\n${PREBUILD_BLOCK.end}`,
  );
  assert.ok(
    prebuildRecipeErrors({...inputs, documentText: twice}).some(error =>
      error.includes('declared twice'),
    ),
    'two blocks are two sources of truth',
  );
  const empty = withRecipe(inputs, () => ['# 暂时没写']);
  assert.ok(
    prebuildRecipeErrors(empty).some(error => error.includes('carries no command')),
    'an empty recipe must not pass by matching nothing',
  );
});

test('flag order is not the judged property, but a duplicate line is', async () => {
  const inputs = await load();
  // 门控写 `--workspace --locked --exclude`，文档按阅读顺序写 `--workspace --exclude … --locked`：
  // 这是同一批单元，判据 MUST NOT 因为它红。
  const reordered = withRecipe(inputs, lines =>
    lines.map(line => {
      const parts = line.split(' ');
      return [parts[0], parts[1], ...parts.slice(2).reverse()].join(' ');
    }),
  );
  assert.notDeepEqual(
    documentedPrebuildSteps(reordered.documentText),
    documentedPrebuildSteps(inputs.documentText),
    'the token order must really have changed',
  );
  assert.deepEqual(
    prebuildRecipeErrors(reordered),
    [],
    'the same flag set in another order must still match',
  );
  // 反向：多一条同样的命令 ⇒ 它覆盖不了第二条门控步，而门控也只需要它一次。
  const duplicated = withRecipe(inputs, lines => [lines[0], ...lines]);
  assert.ok(
    prebuildRecipeErrors(duplicated).some(error => error.includes('a duplicated step is not the same step')),
    'a duplicated recipe line must be named',
  );
  // 签名本身：排序后拼接，`--no-run` 之外的一切旗标都参与比较。
  assert.equal(
    commandSignature(['test', '--lib', '--locked']),
    commandSignature(['--locked', 'test', '--lib']),
  );
  assert.notEqual(
    commandSignature(['test', '--locked']),
    commandSignature(['test', '--locked', '--lib']),
    'a dropped target flag must change the signature',
  );
});
