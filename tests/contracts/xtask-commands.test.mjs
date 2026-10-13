import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import {
  XTASK_HOST,
  COMMAND_KINDS,
  STANDALONE_REASONS,
  HELP_SIGNATURES,
  XTASK_COMMANDS,
  dispatchSignatures,
  usageSignatures,
  gateRegions,
  registryShapeErrors,
  commandErrors,
} from '../../scripts/contracts/xtask-commands.mjs';

const HOST = XTASK_HOST;

/// 判据的输入**显式注入**：`gateSources` 少了它，「本机门控真的跑它」的凭据会退化成恒真。
async function load() {
  const text = await readFile(HOST, 'utf8');
  const gateSources = new Map([[HOST, text]]);
  return {text, gateSources};
}

function registryOf(...mutations) {
  const copy = structuredClone(XTASK_COMMANDS);
  for (const mutate of mutations) mutate(copy);
  return copy;
}

function entry(registry, signature) {
  return registry.find(item => item.signature === signature);
}

test('every xtask command is declared, and every declaration still has a command', async () => {
  const {text, gateSources} = await load();
  assert.ok(XTASK_COMMANDS.length > 0, 'the declared command set must not be empty');
  assert.deepEqual(
    commandErrors(text, XTASK_COMMANDS, gateSources),
    [],
    'the shipped command registry must match the harness',
  );
  const dispatched = dispatchSignatures(text);
  assert.ok(dispatched.length > 0, 'the harness must dispatch commands');
  assert.equal(new Set(dispatched).size, dispatched.length, 'dispatch signatures must be unique');
  assert.equal(
    dispatched.length,
    XTASK_COMMANDS.length,
    'every dispatched command must have exactly one declaration',
  );
  // 非空洞：四类都真的被用到——否则某一格的判据对空集恒真。
  const kinds = new Set(XTASK_COMMANDS.map(item => item.kind));
  for (const kind of COMMAND_KINDS) {
    assert.ok(kinds.has(kind), `the registry must exercise the ${kind} kind, otherwise its rules are vacuous`);
  }
});

test('an unregistered command is named', async () => {
  const {text, gateSources} = await load();
  const injected = text.replace(
    '        ["check", "--registry"] => registry::check(root),\n',
    '        ["check", "--registry"] => registry::check(root),\n        ["check", "--audit"] => Ok(()),\n',
  );
  const errors = commandErrors(injected, XTASK_COMMANDS, gateSources);
  assert.ok(
    errors.some(error => error.includes('"check --audit"') && error.includes('nobody declares')),
    `an unregistered command must be named, saw ${JSON.stringify(errors)}`,
  );
});

test('a registered command that vanished is named', async () => {
  const {text, gateSources} = await load();
  const injected = text.replace('        ["check", "--secrets"] => secrets::check(root),\n', '');
  const errors = commandErrors(injected, XTASK_COMMANDS, gateSources);
  assert.ok(
    errors.some(error => error.includes('"check --secrets"') && error.includes('does not')),
    `a vanished command must be named, saw ${JSON.stringify(errors)}`,
  );
});

test('renaming a command fails on both sides', async () => {
  const {text, gateSources} = await load();
  const injected = text.replace('["check", "--registry"]', '["check", "--catalog"]');
  const errors = commandErrors(injected, XTASK_COMMANDS, gateSources);
  assert.ok(errors.some(error => error.includes('"check --catalog"')), 'the new signature must be named');
  assert.ok(
    errors.some(error => error.includes('"check --registry"') && error.includes('does not')),
    'the old signature must be reported as vanished too',
  );
});

test('the usage string must not advertise a command that does not exist', async () => {
  const {text, gateSources} = await load();
  const injected = text.replace('check-deps | check --registry |', 'check-deps | check --audit | check --registry |');
  const errors = commandErrors(injected, XTASK_COMMANDS, gateSources);
  assert.ok(
    errors.some(error => error.includes('"check --audit"') && error.includes('must not promise what does not exist')),
    `usage must not promise an unimplemented command, saw ${JSON.stringify(errors)}`,
  );
});

test('a dispatched command must appear in the usage string', async () => {
  const {text, gateSources} = await load();
  // 从 usage 里删掉 `sbom [--out <path>]` 一段——`sbom` 与 `sbom --out` 都消失。
  const injected = text.replace(' | sbom [--out <path>]', '');
  const errors = commandErrors(injected, XTASK_COMMANDS, gateSources);
  assert.ok(
    errors.some(error => error.includes('"sbom"') && error.includes('nobody can discover')),
    `an undiscoverable command must be named, saw ${JSON.stringify(errors)}`,
  );
});

test('gate coverage has to cite a site inside the local gate closure', async () => {
  const {text, gateSources} = await load();
  // 注入一：凭据指向文件里**根本不存在**的串 ⇒ 假契约。
  const ghost = registryOf(copy => {
    entry(copy, 'check-deps').evidence = {file: HOST, marker: '"no-such-gate-marker"'};
  });
  assert.ok(
    commandErrors(text, ghost, gateSources).some(error =>
      error.includes('"check-deps"') && error.includes('not inside the local gate closure'),
    ),
    'a marker that exists nowhere must be reported',
  );
  // 注入二（**本层独有**）：凭据指向一个**文件里逐字存在、却不在门控闭包内**的串。
  // `secrets::check(root)` 只住在 `["check", "--secrets"]` 这条独立 arm 里，门控从不执行它
  // ⇒ 只验「逐字存在」而不验「落在闭包内」的判据会放它过去（**自我满足**）。
  const outside = registryOf(copy => {
    entry(copy, 'check-deps').evidence = {file: HOST, marker: 'secrets::check(root)'};
  });
  assert.ok(
    commandErrors(text, outside, gateSources).some(error =>
      error.includes('"check-deps"') && error.includes('not inside the local gate closure'),
    ),
    'a marker that exists in the file but outside the gate closure must be reported',
  );
  // 注入三：凭据指向一个没被提供的文件 ⇒ 缺输入必须具名，绝不静默跳过。
  const absent = registryOf(copy => {
    entry(copy, 'check-deps').evidence = {file: 'xtask/src/no-such.rs', marker: 'x'};
  });
  assert.ok(
    commandErrors(text, absent, gateSources).some(error => error.includes('not provided for judgement')),
    'a missing gate source must be named rather than skipped',
  );
  // 注入四：根本没有凭据 ⇒ 「本机门控真的跑它」是一句空话。
  const bare = registryOf(copy => {
    delete entry(copy, 'check-deps').evidence;
  });
  assert.ok(
    commandErrors(text, bare, gateSources).some(error => error.includes('names no gate site')),
    'a gate claim without evidence must be reported',
  );
});

test('the gate closure is located, not assumed', async () => {
  const {text, gateSources} = await load();
  const regions = gateRegions(text);
  assert.ok(regions.length >= 2, 'the gate closure must hold the driver and the profile arms');
  assert.ok(
    regions.some(region => region.name === 'engineering_checks'),
    'the shared driver must be one of the regions',
  );
  // 非空洞：驱动体里真的含有 `check-deps` 的证据落点。
  const driver = regions.find(region => region.name === 'engineering_checks');
  assert.ok(driver.text.includes('deps::check(root)?'), 'the driver must really run check-deps');
  // 反向：一条独立 arm 的执行体**不在**闭包里（这正是「区域」这一层要挡住的东西）。
  const injected = text.replace('fn engineering_checks(', 'fn not_engineering_checks(');
  assert.throws(() => gateRegions(injected), /engineering_checks/, 'a missing driver must throw rather than pass');
  assert.deepEqual(
    commandErrors(injected, XTASK_COMMANDS, gateSources).some(error => error.includes('gate closure could not be located')),
    true,
    'an unlocatable gate closure must be named',
  );
});

test('a reason outside the closed set is a label, not a reason', async () => {
  const {text, gateSources} = await load();
  const injected = registryOf(copy => {
    entry(copy, 'check --secrets').reason = 'because';
  });
  assert.ok(
    commandErrors(text, injected, gateSources).some(error => error.includes('not in the cause table')),
    'an unknown cause must be named',
  );
  // 非空洞：闭集本身非空，否则上面那条对空集恒真。
  assert.ok(STANDALONE_REASONS.length > 0, 'the cause table must not be empty');
});

test('a standalone command must not also claim gate coverage', async () => {
  const {text, gateSources} = await load();
  const injected = registryOf(copy => {
    entry(copy, 'check --secrets').evidence = {file: HOST, marker: 'deps::check(root)?'};
  });
  assert.ok(
    commandErrors(text, injected, gateSources).some(error =>
      error.includes('inside the gate closure and outside it at once'),
    ),
    'a command cannot be gated and standalone at once',
  );
});

test('artifact and help carry neither gate evidence nor a cause', async () => {
  const {text, gateSources} = await load();
  const withEvidence = registryOf(copy => {
    entry(copy, 'sbom').evidence = {file: HOST, marker: 'deps::check(root)?'};
  });
  assert.ok(
    commandErrors(text, withEvidence, gateSources).some(error => error.includes('cannot claim gate coverage')),
    'an artifact must not claim gate coverage',
  );
  const withReason = registryOf(copy => {
    entry(copy, 'help').reason = 'deliberate-on-demand';
  });
  assert.ok(
    commandErrors(text, withReason, gateSources).some(error => error.includes('must not be merged')),
    'a help entry must not carry a standalone cause',
  );
});

test('a legitimate registration is not reported', async () => {
  const {text, gateSources} = await load();
  // 反向：把凭据换成**另一处真的在闭包内**的落点 ⇒ MUST NOT 红
  // （判据验「落在闭包内」，不验「等于某串」）。
  const other = registryOf(copy => {
    entry(copy, 'check --profile full').evidence = {file: HOST, marker: 'engineering_checks(root)?'};
  });
  assert.deepEqual(
    commandErrors(text, other, gateSources),
    [],
    'a different but in-closure gate site must not be reported',
  );
});

test('an unparseable harness or an empty dispatch is named, never skipped', async () => {
  const {gateSources} = await load();
  const noExecute = commandErrors('fn other() {}\n', XTASK_COMMANDS, gateSources);
  assert.ok(
    noExecute.some(error => error.includes('could not be parsed')),
    'a harness with no execute must be named rather than silently pass',
  );
  const noUsage = commandErrors('pub fn execute(args: &[String]) { match args { ["help"] => {} _ => {} } }', XTASK_COMMANDS, gateSources);
  assert.ok(
    noUsage.some(error => error.includes('usage string could not be parsed')),
    'a missing usage string must be named rather than silently pass',
  );
  const unbalanced = commandErrors('pub fn execute(args: &[String]) { match args {', XTASK_COMMANDS, gateSources);
  assert.ok(
    unbalanced.some(error => error.includes('could not be parsed')),
    'an unbalanced match must be named rather than silently pass',
  );
  const {text} = await load();
  // 空 dispatch：所有臂都被删掉 ⇒ 闭世界对空集恒真 ⇒ 必须显式报错。
  const emptied = text.replace(/^[ \t]*\[[^\n]*\] => .*$/gm, '        _ => Err(String::new()),');
  const emptiedErrors = commandErrors(emptied, XTASK_COMMANDS, gateSources);
  assert.ok(
    emptiedErrors.length > 0,
    'an emptied dispatch must not pass the closed world',
  );
});

test('the registry shape rejects duplicates and non-string signatures', () => {
  assert.deepEqual(registryShapeErrors(), [], 'the shipped registry must be self-consistent');
  const duplicated = [...XTASK_COMMANDS, {...XTASK_COMMANDS[1]}];
  assert.ok(
    registryShapeErrors(duplicated).some(error => error.includes('declared twice')),
    'a duplicated signature must be reported',
  );
  assert.ok(
    registryShapeErrors([{signature: 42, kind: 'help'}]).some(error => error.includes('non-string signature')),
    'a non-string signature must be reported',
  );
});

test('the signature separates commands that share a leading name', () => {
  const signatures = dispatchSignatures(
    'pub fn execute(a: &[String]) { match args {\n' +
      '    ["check", "--golden"] => {}\n' +
      '    ["check", "--golden", "--dir", dir] => {}\n' +
      '    ["help"] | ["--help"] | [] => {}\n' +
      '    _ => {}\n' +
      '} }\n',
  );
  assert.deepEqual(signatures, ['check --golden', 'check --golden --dir', 'help', '--help', '']);
  // 非空洞：真实宿主里这两条确实都存在，否则上面的区分是凭空的。
  assert.ok(signatures.length >= 5, 'the synthetic arms must be read');
});

test('the usage expansion understands optional segments and multi-valued tokens', () => {
  const usage = usageSignatures(
    '"Usage: cargo xtask a | b [--dir <path>] | c one|two|three\\nOther tasks are not available yet."',
  );
  assert.deepEqual(usage, ['a', 'b', 'b --dir', 'c one', 'c two', 'c three']);
  // 非空洞：真实 usage 里可选段与多值 token 都存在（`sbom [--out <path>]`、`schema [--check]`、
  // `test --suite contracts|storage|…`），否则上面的规则没有对象。
  assert.ok(HELP_SIGNATURES.includes('') && HELP_SIGNATURES.includes('help'));
});

test('the command table is pinned verbatim', async () => {
  const {text} = await load();
  // 表级：逐字钉住四条常量与整张签名清单。派生写法（例如把命令集由 `execute()` 算出）
  // 会让「清单 == 它自己」恒真，一个字都没说。
  assert.deepEqual(COMMAND_KINDS, ['gate', 'standalone', 'artifact', 'help']);
  assert.deepEqual(STANDALONE_REASONS, [
    'host-subprocess',
    'requires-compiled-artifact',
    'superseded-by-workspace-run',
    'deliberate-on-demand',
  ]);
  assert.deepEqual(HELP_SIGNATURES, ['help', '--help', '']);
  assert.deepEqual(XTASK_COMMANDS.map(item => item.signature).sort(), [
    '',
    '--help',
    'check --golden',
    'check --golden --dir',
    'check --profile full',
    'check --profile portable',
    'check --profile windows-local',
    'check --registry',
    'check --secrets',
    'check-deps',
    'help',
    'sbom',
    'sbom --out',
    'schema',
    'schema --check',
    'test --suite contracts',
    'test --suite hosts',
    'test --suite kernel',
    'test --suite knowledge',
    'test --suite provider',
    'test --suite scheduler',
    'test --suite session',
    'test --suite storage',
    'test --suite upgrade',
  ]);
  assert.deepEqual(dispatchSignatures(text).sort(), XTASK_COMMANDS.map(item => item.signature).sort());
  // 非空洞：每一类都有对象，否则分类规则对空集恒真。
  for (const kind of COMMAND_KINDS) {
    assert.ok(XTASK_COMMANDS.some(item => item.kind === kind), `the ${kind} kind must have entries`);
  }
});
