import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { readFileSync } from 'node:fs';

import { readTrackedPaths, describeFailure } from '../../scripts/contracts/git-index.mjs';
import {
  TEST_TIERS,
  TEST_REGISTRATION_SITES,
  LOCALLY_UNREACHABLE,
  reachabilityErrors,
} from '../../scripts/contracts/test-reachability.mjs';
import { gateRegions } from '../../scripts/contracts/xtask-commands.mjs';
import { driverSteps } from '../../scripts/contracts/npm-scripts.mjs';
import { WORKFLOW_STEPS, workflowStepErrors, workflowStepsOf } from '../../scripts/contracts/workflow-steps.mjs';
import {
  HARNESS_LIB,
  DRIVER,
  CI_WORKFLOW,
  PORTABLE_REGION,
  TIER_PAYMENTS,
  stripShellComments,
  portableSurface,
  tierErrors,
} from '../../scripts/contracts/tier-payment.mjs';

/// 判据的输入**全部显式注入**：本模块自己不读磁盘（遍历工作区是 W75 的 F2 根因）。
/// 真实仓库那一组在文件末尾，它读 `.git/index` 与四份真文件。
async function load() {
  const index = await readTrackedPaths('.');
  assert.ok(index.ok, describeFailure(index));

  const packages = new Map();
  for (const dir of ['.', 'packages/contracts-ts', 'apps/desktop']) {
    const file = dir === '.' ? 'package.json' : `${dir}/package.json`;
    packages.set(dir, await readFile(file, 'utf8'));
  }

  return {
    sites: TEST_REGISTRATION_SITES,
    libText: await readFile(HARNESS_LIB, 'utf8'),
    driverText: await readFile(DRIVER, 'utf8'),
    workflowText: await readFile(CI_WORKFLOW, 'utf8'),
    packages,
    tracked: index.paths,
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
  assert.notEqual(next, text, `the injection must really change something: ${from}`);
  return next;
};

/* ----------------------------------------------------------------------------------------------
 * 合成世界：三个站点、一份门控宿主、一份驱动表、一份 workflow、一份 package.json。
 *
 * 三层的兑付面**两两不同**，所以一条证据不可能同时给两种签名：
 *   port   portable → engineering_checks 区域（剥注释后）里的 `--test <marker>`
 *   front  full     → 驱动表 STEP `['.', 'test:ui']` 的脚本体
 *   remote ci       → 某个 step 的 `run` 文本
 * -------------------------------------------------------------------------------------------- */

const LIB_STUB = [
  'fn engineering_checks(root: &Path) -> Result<(), String> {',
  '    harness::run(root, "tests/one.test.mjs")?;',
  '    Ok(())',
  '}',
  '',
  'pub fn execute(args: &[String], root: &Path) -> Result<(), String> {',
  '    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {',
  '        ["check", "--profile", profile @ ("portable" | "windows-local")] => {',
  '            engineering_checks(root)?;',
  '            Ok(())',
  '        }',
  '        ["check", "--profile", "full"] => {',
  '            engineering_checks(root)?;',
  '            frontend::check(root)?;',
  '            Ok(())',
  '        }',
  '        _ => Err("unsupported".into()),',
  '    }',
  '}',
].join('\n');

const DRIVER_STUB = [
  'const STEPS = [',
  "  ['.', 'test:ui'],",
  "  ['apps/desktop', 'test'],",
  '];',
  "const KNOWN_EXECUTABLES = new Set(['node']);",
].join('\n');

const WORKFLOW_STUB = [
  'jobs:',
  '  foundation:',
  '    steps:',
  '      - name: Foundation gate',
  '        run: cargo xtask check --profile full',
  '      - name: Contract suite',
  '        run: node --test tests/ci.test.mjs',
].join('\n');

const PACKAGE_STUB = JSON.stringify({
  name: 'root',
  scripts: { 'test:ui': 'node --test tests/group/*.test.mjs' },
});

const fakeSites = () => [
  { id: 'port', host: HARNESS_LIB, marker: 'tests/one.test.mjs', base: '.', tier: 'portable' },
  { id: 'front', host: 'package.json', marker: 'tests/group/*.test.mjs', base: '.', tier: 'full' },
  { id: 'remote', host: CI_WORKFLOW, marker: 'tests/ci.test.mjs', base: '.', tier: 'ci' },
];

const fakeInputs = () => ({
  sites: fakeSites(),
  libText: LIB_STUB,
  driverText: DRIVER_STUB,
  workflowText: WORKFLOW_STUB,
  packages: new Map([['.', PACKAGE_STUB]]),
});

// ---------------------------------------------------------------------------
// T1 规则闭世界
// ---------------------------------------------------------------------------

test('the payment rule table is exactly the closed set of tiers, and each tier has a site', async () => {
  assert.deepEqual(Object.keys(TIER_PAYMENTS).sort(), [...TEST_TIERS].sort());
  assert.deepEqual(TEST_TIERS, ['portable', 'full', 'ci']);
  // **非空洞**：一个没有站点的取值让「这条规则」恒真。
  for (const tier of TEST_TIERS) {
    assert.ok(
      TEST_REGISTRATION_SITES.some(site => site.tier === tier),
      `tier ${tier} must have at least one site, or its rule is vacuously true`,
    );
  }
  assert.ok(TEST_REGISTRATION_SITES.length >= 12, 'the shipped site table must not silently shrink');
  // 三层的兑付面两两不同（一条证据不可能同时给两种签名）。
  assert.equal(new Set(Object.values(TIER_PAYMENTS).map(rule => rule.surface)).size, 3);
});

test('a tier outside the rule table is named', () => {
  const sites = [...fakeSites(), { id: 'weird', host: HARNESS_LIB, marker: 'tests/weird.test.mjs', base: '.', tier: 'nightly' }];
  const errors = tierErrors({ ...fakeInputs(), sites });
  assert.ok(
    errors.some(error => error.includes('weird') && error.includes('no payment rule')),
    `a tier with no rule must be named; got ${JSON.stringify(errors)}`,
  );
});

test('a tier value with no site is named, so its rule cannot be vacuously true', () => {
  const sites = fakeSites().filter(site => site.tier !== 'ci');
  const errors = tierErrors({ ...fakeInputs(), sites });
  assert.ok(
    errors.some(error => error.includes('no registration site declares tier "ci"')),
    `a tier with no site must be named; got ${JSON.stringify(errors)}`,
  );
});

// ---------------------------------------------------------------------------
// T2 portable：由 engineering_checks 区域（**剥注释后**）兑付
// ---------------------------------------------------------------------------

test('a portable marker that left the portable region is named', () => {
  // 这就是探针 P-A 的合成版：注册行被搬离 `engineering_checks`，站点仍说 `portable`。
  const libText = patch(LIB_STUB, '    harness::run(root, "tests/one.test.mjs")?;\n', '');
  const errors = tierErrors({ ...fakeInputs(), libText });
  assert.ok(
    errors.some(error => error.includes('port') && error.includes('is not in the engineering_checks region')),
    `a portable site with no region paying for it must be named; got ${JSON.stringify(errors)}`,
  );
});

test('a marker that survives only in a comment does not pay for a tier', () => {
  // W99 的注释陷阱：区域**原文**含它，剥注释后不含。实测真实仓库里
  // `tests/contracts/descriptor.test.mjs` 正是这种形态（见下面的前提用例）。
  const libText = patch(LIB_STUB, '    harness::run(root, "tests/one.test.mjs")?;', '    // harness::run(root, "tests/one.test.mjs")?;');
  assert.ok(libText.includes('tests/one.test.mjs'), 'the raw harness must still contain the marker');
  const errors = tierErrors({ ...fakeInputs(), libText });
  assert.ok(
    errors.some(error => error.includes('port') && error.includes('is not in the engineering_checks region')),
    `a comment must not pay for a tier; got ${JSON.stringify(errors)}`,
  );
});

test('a portable site hosted outside the harness is paid by a call to its module', () => {
  // `xtask-registry` 的形态：marker 的字面量在 `registry.rs` 里，portable 区域里出现的是
  // `registry::check(root)?;`。兑付串因此由 host **派生**，不是一个可省的 `via` 字段。
  const sites = fakeSites().map(site => (site.id === 'port' ? { ...site, host: 'xtask/src/registry.rs' } : site));

  const paid = patch(LIB_STUB, 'harness::run(root, "tests/one.test.mjs")?;', 'registry::check(root)?;');
  assert.deepEqual(tierErrors({ ...fakeInputs(), sites, libText: paid }), [], 'a module call in the region must pay');

  const unpaid = fakeInputs().libText;
  const errors = tierErrors({ ...fakeInputs(), sites, libText: unpaid });
  assert.ok(
    errors.some(error => error.includes('port') && error.includes('registry::<fn>(root')),
    `a module that is not called in the region must be named; got ${JSON.stringify(errors)}`,
  );
});

// ---------------------------------------------------------------------------
// T3 full：由驱动表 STEP 的脚本体兑付
// ---------------------------------------------------------------------------

test('a full site whose package dir has no driver step is named', () => {
  const driverText = patch(DRIVER_STUB, "  ['.', 'test:ui'],\n", '');
  const errors = tierErrors({ ...fakeInputs(), driverText });
  assert.ok(
    errors.some(error => error.includes('front') && error.includes('names no STEP for package dir "."')),
    `a full site with no driver step must be named; got ${JSON.stringify(errors)}`,
  );
});

test('a driver step whose script body does not name the marker is an indirection, not a payment', () => {
  // `apps/desktop` 有 typecheck / build / test 三个 STEP，只有 test 的体含 `tests/*.test.mjs`
  // ——「该目录出现过 STEP」不算兑付。
  const wrong = JSON.stringify({ name: 'root', scripts: { 'test:ui': 'node --test tests/other/*.test.mjs' } });
  const errors = tierErrors({ ...fakeInputs(), packages: new Map([['.', wrong]]) });
  assert.ok(
    errors.some(error => error.includes('front') && error.includes('not in the script body of any STEP')),
    `a step body that does not name the marker must be named; got ${JSON.stringify(errors)}`,
  );
  // 前提的另一半：那条 STEP **确实**落在该目录上（否则上一条可能只是碰巧命中）。
  assert.ok(driverSteps(DRIVER_STUB).some(step => step.dir === '.'));
});

test('a full site whose host is not a package manifest is named', () => {
  const sites = fakeSites().map(site => (site.id === 'front' ? { ...site, host: 'scripts/frontend-gate.mjs' } : site));
  const errors = tierErrors({ ...fakeInputs(), sites });
  assert.ok(
    errors.some(error => error.includes('front') && error.includes('is not a package manifest')),
    `a full site must be wired through a manifest; got ${JSON.stringify(errors)}`,
  );
});

test('a full site whose manifest was not provided is named, not silently skipped', () => {
  const sites = fakeSites().map(site => (site.id === 'front' ? { ...site, host: 'apps/desktop/package.json' } : site));
  const errors = tierErrors({ ...fakeInputs(), sites }); // packages 里只有 `.`
  assert.ok(
    errors.some(error => error.includes('front') && error.includes('manifest of package dir "apps/desktop" was not provided')),
    `a missing manifest must be named; got ${JSON.stringify(errors)}`,
  );
});

// ---------------------------------------------------------------------------
// T4 ci：由某个 step 的 `run` 文本（剥整行注释）兑付
// ---------------------------------------------------------------------------

test('a ci marker no step runs is named', () => {
  const workflowText = patch(WORKFLOW_STUB, 'node --test tests/ci.test.mjs', 'node --test tests/absent.test.mjs');
  const errors = tierErrors({ ...fakeInputs(), workflowText });
  assert.ok(
    errors.some(error => error.includes('remote') && error.includes('is not named by any step')),
    `a ci-only test that no step runs must be named; got ${JSON.stringify(errors)}`,
  );
});

test('a ci marker that survives only in a run comment does not pay for a tier', () => {
  const workflowText = patch(WORKFLOW_STUB, '        run: node --test tests/ci.test.mjs', [
    '        run: |',
    '          # node --test tests/ci.test.mjs',
    '          cargo xtask check --profile portable',
  ].join('\n'));
  const errors = tierErrors({ ...fakeInputs(), workflowText });
  assert.ok(
    errors.some(error => error.includes('remote') && error.includes('is not named by any step')),
    `a run comment must not pay for a tier; got ${JSON.stringify(errors)}`,
  );
});

test('the shell comment stripper drops comments and keeps commands', () => {
  assert.equal(stripShellComments('node --test a.mjs\n# tests/x.test.mjs\n'), 'node --test a.mjs\n\n');
  assert.equal(stripShellComments('node --test a.mjs # tests/x.test.mjs'), 'node --test a.mjs');
  assert.equal(stripShellComments('  # tests/x.test.mjs'), '');
  assert.ok(stripShellComments('node --test tests/x.test.mjs').includes('tests/x.test.mjs'));
});

// ---------------------------------------------------------------------------
// T5 反向：声明不得少报本机门控真正跑到的东西
// ---------------------------------------------------------------------------

test('a full or ci site whose marker sits in the portable region is named', () => {
  const sites = fakeSites().map(site => (site.id === 'front' ? { ...site, marker: 'tests/one.test.mjs' } : site));
  const packages = new Map([['.', JSON.stringify({ name: 'root', scripts: { 'test:ui': 'node --test tests/one.test.mjs' } })]]);
  const errors = tierErrors({ ...fakeInputs(), sites, packages });
  assert.ok(
    errors.some(error => error.includes('front') && error.includes('understates what the local gate already runs')),
    `a declaration that understates the local gate must be named; got ${JSON.stringify(errors)}`,
  );
});

// ---------------------------------------------------------------------------
// 输入完整性
// ---------------------------------------------------------------------------

test('missing inputs are named, not silently skipped', () => {
  const errors = tierErrors({ sites: fakeSites() });
  assert.ok(errors.some(error => error.includes('harness source was not provided')), JSON.stringify(errors));
  assert.ok(errors.some(error => error.includes(`${DRIVER} was not provided`)), JSON.stringify(errors));
  assert.ok(errors.some(error => error.includes(`${CI_WORKFLOW} was not provided`)), JSON.stringify(errors));
});

test('an empty site table names every tier instead of passing vacuously', () => {
  const errors = tierErrors({ ...fakeInputs(), sites: [] });
  assert.equal(errors.length, 3, `expected one error per tier; got ${JSON.stringify(errors)}`);
  for (const tier of TEST_TIERS) {
    assert.ok(errors.some(error => error.includes(`declares tier "${tier}"`)), JSON.stringify(errors));
  }
});

test('an unparsable harness is a named failure, not an empty verdict', () => {
  const errors = tierErrors({ ...fakeInputs(), libText: '// nothing here\n' });
  assert.ok(
    errors.some(error => error.includes('the portable surface could not be read')),
    `an unreadable region must be named; got ${JSON.stringify(errors)}`,
  );
  assert.ok(errors.length > 0);
});

// ---------------------------------------------------------------------------
// 真实仓库
// ---------------------------------------------------------------------------

test('the comment trap is real: the raw region names the ci marker, the stripped one does not', async () => {
  const { libText } = await load();
  const raw = gateRegions(libText).find(region => region.name === PORTABLE_REGION).text;
  assert.ok(
    raw.includes('tests/contracts/descriptor.test.mjs'),
    'the raw region must really contain the ci marker, or this premise is not demonstrated',
  );
  assert.ok(
    !portableSurface(libText).text.includes('tests/contracts/descriptor.test.mjs'),
    'comments must be stripped, or two ci sites would be satisfied by a comment',
  );
});

test('the real repository pays for every tier declaration', async () => {
  const inputs = await load();
  const errors = tierErrors({
    sites: inputs.sites,
    libText: inputs.libText,
    driverText: inputs.driverText,
    workflowText: inputs.workflowText,
    packages: inputs.packages,
  });
  assert.deepEqual(errors, [], 'every shipped tier declaration must be paid for');

  // 前提的另一半：三层面都**确实**解析出了东西，否则上面的空数组可能只是「什么都没解析出来」。
  const surface = portableSurface(inputs.libText).text;
  for (const site of inputs.sites.filter(site => site.tier === 'portable' && site.host === HARNESS_LIB)) {
    assert.ok(surface.includes(site.marker), `the portable surface must really name ${site.marker}`);
  }
  assert.ok(driverSteps(inputs.driverText).length >= 6, 'the driver step table must be non-trivial');
  assert.ok(workflowStepsOf(inputs.workflowText).length >= 10, 'the workflow step set must be non-trivial');
});

// ---------------------------------------------------------------------------
// 边界（与相邻判据互不顶替）
// ---------------------------------------------------------------------------

test('the tier judge is orthogonal to the reachability judge', async () => {
  const inputs = await load();
  // 铁律一：本用例的断言依赖「本轮新测试文件在册」，而提交前它还没进 `.git/index`
  // ⇒ 由**构造**使前提成立（并集），不依赖提交顺序。
  const tracked = new Set([...inputs.tracked, 'tests/contracts/tier-payment.test.mjs']);
  // 只改 `tier`，可达性一个字都不该动——`reachabilityErrors` 根本不读这个字段。
  const flipped = inputs.sites.map(site => ({ ...site, tier: site.tier === 'portable' ? 'full' : 'portable' }));
  assert.deepEqual(
    reachabilityErrors({
      tracked,
      sites: flipped,
      unreachable: [...LOCALLY_UNREACHABLE],
      readHost,
    }),
    [],
    'the reachability judge must not read tier at all',
  );
  assert.ok(
    tierErrors({ ...inputs, sites: flipped }).length > 0,
    'the tier judge must be the one that notices a tier change',
  );
});

test('dropping a site reds the reachability judge but not the tier judge', async () => {
  const inputs = await load();
  // 铁律一：断言依赖「新测试文件在册」，而提交前它还没进 `.git/index`
  // ⇒ 由**构造**使这个前提成立（并集），不依赖墙钟与提交顺序。
  const newTest = 'tests/contracts/tier-payment.test.mjs';
  const tracked = new Set([...inputs.tracked, newTest]);
  const withoutNew = inputs.sites.filter(site => site.id !== 'xtask-engineering-tier-payment');

  assert.ok(
    reachabilityErrors({ tracked, sites: withoutNew, unreachable: [], readHost }).some(error => error.includes(newTest)),
    'W84 must notice an uncovered test file',
  );
  assert.deepEqual(
    tierErrors({ ...inputs, sites: withoutNew }),
    [],
    'three tiers still have sites, so the tier judge has nothing to say',
  );
});

test('renaming a CI step reds the workflow judge but not the tier judge', async () => {
  const inputs = await load();
  const renamed = patch(inputs.workflowText, 'name: Validate schemas and API contract', 'name: Validate the schema contract');
  // W95：注册的 step 身份消失 ⇒ 红。
  assert.ok(
    workflowStepErrors(CI_WORKFLOW, renamed, WORKFLOW_STEPS, new Map()).some(error => error.includes('silently disappeared')),
    'W95 must notice a registered step that vanished',
  );
  // W99：tier 只看 `run` 文本，不看 step 名 ⇒ 仍绿。
  assert.deepEqual(tierErrors({ ...inputs, workflowText: renamed }), [], 'the tier judge must not read step names');
});
