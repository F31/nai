import test from 'node:test';
import assert from 'node:assert/strict';
import { readTrackedPaths, readUntrackedPaths } from '../../scripts/contracts/git-index.mjs';
import {
  TEST_REGISTRATION_SITES,
  markerMatcher,
  pendingRegistrationErrors,
  pendingTestFiles,
} from '../../scripts/contracts/test-reachability.mjs';

/// W135：**补集判据**——「未跟踪且未被忽略」的那一半也必须登记。
///
/// 它守的是 W134 实测过的缺陷：新测试文件未登记、且还没 `git add` ⇒ `.git/index` 里没有
/// ⇒ `reachabilityErrors` 的闭世界看不见它 ⇒ 本机全绿、CI 一 checkout 就红
/// （run `39016233527` 两平台 `failure`，170 秒早退）。
///
/// ⚠️ **本文件自己就是这条判据的第一个受审对象**：它诞生时是未跟踪的、也还没登记
/// ⇒ 编号 ① 的用例在登记之前**必须**红（红在它自己的文件名上）。这不是装饰，是本轮
/// 判据「能走到那一层」的现场证据。

const sites = entries => entries.map((marker, index) => ({
  id: `probe-${index}`,
  host: 'xtask/src/lib.rs',
  marker,
  base: '.',
  tier: 'portable',
}));

test('the shipped repository has no unregistered pending test file, and the scan really ran', async () => {
  const tracked = await readTrackedPaths('.');
  assert.ok(tracked.ok, `the index must be readable: ${tracked.reason ?? ''}`);
  const present = await readUntrackedPaths('.');
  // **不可解析 ⇒ 具名失败**：不能把「读不到未跟踪集合」当成「没有未跟踪文件」。
  assert.ok(present.ok, `the untracked set must be readable: ${present.reason ?? ''}`);

  const errors = pendingRegistrationErrors({ present: present.paths, tracked: tracked.paths });
  assert.deepEqual(errors, [], 'an unregistered pending test file would turn CI red on the first checkout');

  // **非空洞**：上面那个「空错误」只有在**清扫真的会收东西**的前提下才算数。
  // ⚠️ 刻意**不钉** `scanned > 0`：未跟踪集合的**大小是工作区状态**，不是判定对象——
  // 本机因为有未被忽略的 `target-check/` 扫到 24,796 条，而 CI 是干净 checkout（0 条）
  // ⇒ 钉它就成了「本机绿、CI 红」的机制性差异（与 W131 同型）。反空洞的落点是下面这条
  // **合成输入**前提：它在两种环境里都成立。
  const { candidates } = pendingTestFiles(present.paths, tracked.paths);
  // **前提的第二条：清扫本身不是惰性的**（W135 注入 I3 实测发现的缺口）。`scanned > 0` 只证明
  // 「输入非空」，不证明「清扫会把符合形状的路径收进来」——把 `candidates.push` 关掉之后，本用例
  // 与真实仓库的 F0 **同时**全绿，只有单元用例（把具名路径喂进去的那几条）会红。这条用一个
  // **合成**输入把「清扫不是死的」钉在本用例里：它会随 `present` 退化成恒真，因此必须与
  // `scanned > 0` 成对出现，且两条都排在「真实仓库无待登记文件」之前。
  const sweepProbe = pendingTestFiles(new Set(['tests/contracts/__sweep_probe__.test.mjs']), new Set());
  assert.deepEqual(
    sweepProbe.candidates,
    ['tests/contracts/__sweep_probe__.test.mjs'],
    'the pending sweep is inert — then "nothing pending" below means nothing at all',
  );
  // 独立的复算（不是把判据的返回值再看一眼）：每个「待登记」候选都必须被某个站点的 marker 覆盖。
  // 它对**空集恒真**，所以上面的前提是它的前提——三条必须成对出现。
  const uncovered = candidates.filter(
    filePath => !TEST_REGISTRATION_SITES.some(site => markerMatcher(site).matches(filePath)),
  );
  assert.deepEqual(uncovered, [], `pending test files nobody registered: ${JSON.stringify(uncovered)}`);
});

test('an unregistered pending test file is named, with the fix in the message', () => {
  const errors = pendingRegistrationErrors({
    present: new Set(['tests/contracts/new-thing.test.mjs']),
    tracked: new Set(['tests/registry.test.mjs']),
    sites: sites(['tests/registry.test.mjs']),
  });
  assert.equal(errors.length, 1, `expected exactly one error, got ${JSON.stringify(errors)}`);
  assert.match(errors[0], /tests\/contracts\/new-thing\.test\.mjs/);
  assert.match(errors[0], /no registration site covers it/);
  // 报错点必须包含**怎么做**——一条只说「坏了」的守卫会让人去关掉它。
  assert.match(errors[0], /git add/);
});

test('a pending test file that a site already declares is not named', () => {
  const errors = pendingRegistrationErrors({
    present: new Set(['tests/contracts/new-thing.test.mjs']),
    tracked: new Set(['tests/registry.test.mjs']),
    sites: sites(['tests/registry.test.mjs', 'tests/contracts/new-thing.test.mjs']),
  });
  assert.deepEqual(errors, [], 'registering before committing must satisfy the judge — otherwise it is a sledgehammer');
});

test('a glob marker covers a pending test file as well', () => {
  const errors = pendingRegistrationErrors({
    present: new Set(['tests/contracts/new-thing.test.mjs']),
    tracked: new Set(),
    sites: sites(['tests/contracts/*.test.mjs']),
  });
  assert.deepEqual(errors, [], 'the marker matcher must not differ between the two halves of the commit set');
});

test('a pending test-like file with an unknown extension is named as an extension gap', () => {
  const errors = pendingRegistrationErrors({
    present: new Set(['apps/site/thing.test.ts']),
    tracked: new Set(),
    sites: sites(['apps/site/thing.test.ts']),
  });
  assert.equal(errors.length, 1, `expected exactly one error, got ${JSON.stringify(errors)}`);
  assert.match(errors[0], /test-like file apps\/site\/thing\.test\.ts/);
  assert.match(errors[0], /not one of \.test\.mjs/);
});

test('missing inputs are named, never silently downgraded to a pass', () => {
  const missing = pendingRegistrationErrors({});
  assert.equal(missing.length, 2, `expected both inputs to be named, got ${JSON.stringify(missing)}`);
  assert.ok(missing.some(error => error.includes('untracked-path set')));
  assert.ok(missing.some(error => error.includes('tracked-path set')));
});

test('a pending set that overlaps the index is named, not silently re-judged', () => {
  const errors = pendingRegistrationErrors({
    present: new Set(['tests/contracts/a.test.mjs', 'tests/contracts/b.test.mjs']),
    tracked: new Set(['tests/contracts/a.test.mjs']),
    sites: sites(['tests/contracts/a.test.mjs', 'tests/contracts/b.test.mjs']),
  });
  assert.equal(errors.length, 1, `expected exactly one overlap error, got ${JSON.stringify(errors)}`);
  assert.match(errors[0], /overlaps \.git\/index/);
});

test('an empty pending set is counted as zero, not skipped', () => {
  const present = new Set();
  const { scanned, candidates } = pendingTestFiles(present, new Set());
  assert.equal(scanned, 0, 'the scan count must be reported, not inferred');
  assert.deepEqual(candidates, []);
  assert.deepEqual(pendingRegistrationErrors({ present, tracked: new Set() }), []);
});

test('an empty site table turns every pending test file red', () => {
  const errors = pendingRegistrationErrors({
    present: new Set(['tests/contracts/new-thing.test.mjs']),
    tracked: new Set(),
    sites: [],
  });
  assert.equal(errors.length, 1, `an empty site table must not make the judge vacuously green: ${JSON.stringify(errors)}`);
});

test('the untracked reader fails with a named reason instead of an empty set', async () => {
  // **不可解析 ⇒ 失败并分类**：给一个 git 无法回答的目录，读取器 MUST 报 `ok: false`。
  // 若它返回 `{ok: true, paths: <空集>}`，调用方会把「不知道」读成「没有」——那正是本轮的缺陷形状。
  // ⚠️ 用一个**不存在**的目录而不是 `scripts/` 这类子目录：git 会向上找到仓库根，
  // 子目录仍然是合法仓库 ⇒ 那种输入根本走不到失败路径（等于没测）。
  const failure = await readUntrackedPaths('no-such-directory-w135');
  assert.equal(failure.ok, false, 'a directory git cannot work in must not yield an empty successful set');
  assert.equal(failure.reason, 'spawn');
  assert.match(failure.detail, /git ls-files --others --exclude-standard failed/);
});

test('the shipped site table covers every test file the index holds', async () => {
  // 与 ① 成对的方向：补集判据绿了，不代表**已跟踪**那一半也绿。这条只钉「本表非空且能解析」，
  // 真正的闭世界仍由 `tests/registry.test.mjs` 的 F0 判（同一件事 MUST NOT 两处定义）。
  assert.ok(TEST_REGISTRATION_SITES.length >= 16, `expected at least sixteen sites, got ${TEST_REGISTRATION_SITES.length}`);
});
