import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, readdir, stat } from 'node:fs/promises';
import { readFileSync } from 'node:fs';
import {
  readRegistry,
  readCatalogIds,
  validateRegistry,
  ID_RE,
  RELEASE_LINES,
  STATUSES,
  CAPABILITY_PACK_IDS,
} from '../scripts/contracts/registry.mjs';
import { validators } from '../scripts/contracts/validation.mjs';
import {
  readBaseline,
  parseBaseline,
  parseCapabilityPacks,
  parseReleaseLines,
  sectionTable,
  reconcile,
  pointerErrors,
  declaredBaselineName,
} from '../scripts/contracts/baseline.mjs';
import YAML from 'yaml';
import {
  validatePendingDecisions,
  baselineSections,
  baselineWorkItems,
  baselineWorkItemAcceptance,
  shapeErrors,
  CITATION_FIELDS,
  ID_LIST_FIELDS,
  REPO_PATH_FIELDS,
  ANCHOR_FIELDS,
  STRUCTURAL_FIELDS,
  COUPLED_REQUIRED_FIELDS,
  LEDGER_FIELDS,
  TOP_LEVEL_FIELDS,
  REFERENCE_FIELDS,
  DISPOSITIONS,
  DISPOSITION_COUNTS,
  REQUIRED_FIELD,
  ACCEPTANCE_STATUSES,
  MET_STATUSES,
  UNMET_STATUSES,
} from '../scripts/contracts/pending-decisions.mjs';
import { readTrackedPaths } from '../scripts/contracts/git-index.mjs';
import {
  TEST_EXTENSIONS,
  TEST_TIERS,
  TEST_REGISTRATION_SITES,
  LOCALLY_UNREACHABLE,
  isTestFile,
  markerMatcher,
  resolveSite,
  siteShapeErrors,
  extensionErrors,
  reachabilityErrors,
} from '../scripts/contracts/test-reachability.mjs';

/// 基线文档只读一次：后面的注入用例都在同一份文本上做「最小改动」。
const baseline = await readBaseline();
const configText = await readFile('openspec/config.yaml', 'utf8');

/// 锚点必须**恰好命中一次**。
///
/// 这不是洁癖：注入用例的价值全在「我改的确实是我以为的那一处」。用 `replace` 改到第一处
/// 就把 §1.4 的表改掉了而 §57.3 没动，用例会**假装通过**——守卫在这里变成了「什么都没验证」。
const replaceOnce = (text, from, to) => {
  const occurrences = text.split(from).length - 1;
  assert.equal(
    occurrences,
    1,
    `fixture anchor must occur exactly once, found ${occurrences}: ${from.slice(0, 72)}…`,
  );
  return text.replace(from, to);
};
const mutated = (from, to) => parseBaseline(replaceOnce(baseline.text, from, to));

/// 对账的第三个入参是**当前基线版本**（ADR 0051 决策 2 的台账跨版判据必需）。
/// 用例统一经这里传入，`reconcile` 本身在缺版本时**具名报错**——它 MUST NOT 静默跳过。
const against = (registry, parsed) => reconcile(registry, parsed, baseline.version);

test('registry.yaml is machine-readable and internally consistent', async () => {
  const registry = await readRegistry();
  const catalogIds = await readCatalogIds();
  const errors = validateRegistry(registry, catalogIds);
  assert.deepEqual(errors, []);
  // Cover V3.11 §16 + §30.4 + §48.5 + §51.3 + §57.3 acceptance families and
  // release lines.
  assert.ok(registry.acceptance.length >= 87, 'expected >=87 acceptance items');
  assert.ok(registry.mapping.d01_to_d14.D14, 'expected D14 mapping');
  const slice0 = registry.acceptance.filter(a => a.release_line === 'slice_0');
  assert.ok(['S01','S02','S03','S04','R01','R02','R03','R07','A01','A04'].every(id =>
    slice0.some(a => a.id === id)), 'Slice-0 exit acceptance missing');
  // Every family the baseline defines as a complete range must be registered in
  // full. The extension/marketplace family M (§37.5, §38.10) is not optional —
  // ADR 0042 decision 4 left it unregistered once and the omission was recorded
  // as a defect. The harness family H (V3.11 §40-§47, §48.5) starts from that
  // lesson rather than repeating it: "not started" reads as `pending`, never as
  // a missing row.
  const byId = new Map(registry.acceptance.map(a => [a.id, a]));
  for (const family of ['M', 'H']) {
    for (const n of Array.from({length: 18}, (_, i) => `${family}${String(i + 1).padStart(2, '0')}`)) {
      assert.ok(byId.has(n), `baseline acceptance ${n} is not registered`);
    }
  }
  // The V3.11 scope split has to be readable, not just asserted in prose: the
  // pack line holds exactly the §51.3 set, and none of those ids sits on a
  // first-release line (INV-33).
  const packLine = registry.acceptance.filter(a => a.release_line === 'capability_pack').map(a => a.id);
  assert.equal(packLine.length, 29, 'expected the V3.11 §51.3 capability-pack set');
  for (const id of ['M01', 'M18', 'H05', 'H14', 'H16']) {
    assert.ok(packLine.includes(id), `${id} must be on the capability_pack line`);
  }
  for (const id of ['H01', 'H02', 'H03', 'H04', 'H15', 'H17', 'H18', 'S05', 'B01']) {
    assert.ok(!packLine.includes(id), `${id} must stay on a first-release line`);
  }
});

test('a family present with a hole, or absent, fails the gate and names the id', async () => {
  const registry = await readRegistry();
  const catalogIds = await readCatalogIds();
  // The gate must not be satisfiable by "the family is non-empty": delete one
  // row (a hole) and the whole family (absent), and require both to be caught
  // by name.
  const holed = structuredClone(registry);
  holed.acceptance = holed.acceptance.filter(item => item.id !== 'H05');
  assert.ok(
    validateRegistry(holed, catalogIds).some(
      error => error.includes('H family incomplete') && error.includes('H05'),
    ),
    'a single missing id inside a registered family must be reported by name',
  );
  const absent = structuredClone(registry);
  absent.acceptance = absent.acceptance.filter(item => !/^H[0-9]{2}$/.test(item.id));
  const absentErrors = validateRegistry(absent, catalogIds);
  assert.ok(
    absentErrors.some(error => error.includes('H family incomplete') && error.includes('H01')),
    'an entirely absent family must be reported',
  );
  // Out-of-range ids inside a family are the other half of "complete range".
  const spurious = structuredClone(registry);
  spurious.acceptance.push({...spurious.acceptance[0], id: 'H19'});
  assert.ok(
    validateRegistry(spurious, catalogIds).some(error => error.includes('H19')),
    'an id outside the family range must be reported',
  );
  // M and H are counted separately: H's rows must not be able to cover an M hole.
  const mHoled = structuredClone(registry);
  mHoled.acceptance = mHoled.acceptance.filter(item => item.id !== 'M18');
  assert.ok(
    validateRegistry(mHoled, catalogIds).some(error => error.includes('M18')),
    'a hole in M must not be masked by a fully populated H family',
  );
});

test('the capability-pack split cannot be emptied, promoted, or filled with stray ids', async () => {
  const registry = await readRegistry();
  const catalogIds = await readCatalogIds();
  // Promoting a pack id back onto a first-release line is exactly the drift
  // V3.10 closed: before this round M01-M18 were counted as R1 acceptance, which
  // is where the "first release is too wide" reading came from.
  const promoted = structuredClone(registry);
  promoted.acceptance.find(item => item.id === 'M01').release_line = 'r1';
  assert.ok(
    validateRegistry(promoted, catalogIds).some(
      error => error.includes('INV-33') && error.includes('M01=r1'),
    ),
    'a capability-pack id back on r1 must be reported by name',
  );
  // The other direction: parking a first-release item on the pack line silently
  // shrinks the first release without any visible removal.
  const demoted = structuredClone(registry);
  demoted.acceptance.find(item => item.id === 'H17').release_line = 'capability_pack';
  assert.ok(
    validateRegistry(demoted, catalogIds).some(
      error => error.includes('outside the V3.11 §51.3 set') && error.includes('H17'),
    ),
    'a non-pack id on the capability_pack line must be reported',
  );
  // And the line must not be satisfiable by being empty: an emptied pack line
  // has to fail and name what went missing.
  const emptied = structuredClone(registry);
  emptied.acceptance = emptied.acceptance.filter(item => item.release_line !== 'capability_pack');
  assert.ok(
    validateRegistry(emptied, catalogIds).some(
      error => error.includes('capability pack set incomplete') && error.includes('M01'),
    ),
    'an emptied capability_pack line must fail and name a missing id',
  );
});

test('contract references must resolve to the catalog or a named non-capability contract', async () => {
  const registry = await readRegistry();
  const catalogIds = await readCatalogIds();
  // A dangling reference is what the gate exists for: before w52 the registry
  // pointed at six market.catalog@1 rows and a data.query@1 row while the
  // catalog contained none of them.
  const dangling = structuredClone(registry);
  const holder = dangling.acceptance.find(item => item.contract === 'data.query@1');
  assert.ok(holder, 'expected an acceptance referencing data.query@1');
  holder.contract = 'market.catalog@9';
  assert.ok(
    validateRegistry(dangling, catalogIds).some(error => error.includes('market.catalog@9')),
    'dangling contract reference must be reported',
  );
  // A draft satisfies the reference without changing any status.
  const draft = catalogIds.has('market.catalog@1');
  assert.ok(draft, 'market.catalog@1 must be declared in the catalog as a draft');
  const unchanged = structuredClone(registry);
  assert.deepEqual(validateRegistry(unchanged, catalogIds), []);
  assert.equal(
    unchanged.acceptance.find(item => item.contract === 'market.catalog@1').status,
    'pending',
    'a draft reference must not upgrade the referencing acceptance',
  );
  // The gate cannot be satisfied by simply not supplying the catalog.
  assert.ok(
    validateRegistry(registry, undefined).some(error => error.includes('required')),
    'omitting the catalog must itself be an error',
  );
});

test('the frozen baseline and the registry agree id by id', async () => {
  const registry = await readRegistry();
  const parsed = parseBaseline(baseline.text);

  // ① 基线的选择本身要自洽：docs/ 里版本号最大的那份 MUST 就是配置声明的唯一基线。
  assert.deepEqual(pointerErrors(baseline, configText), []);
  assert.equal(declaredBaselineName(configText), baseline.name);

  // ② 两侧声明必须逐 id 一致。这不是「配置正确」的同义反复：它读的是基线正文的两张表。
  assert.deepEqual(against(registry, parsed), []);

  // ③ 解析器的形状钉死。少了这一层，「对账」可能因为**解析出空集**而恒真：
  // 解析出空集时全部 id 会被算成 extra，所以对账其实很吵——但把两张表的内容逐项写下来
  // 仍然是必须的（W66 的教训：完整性守卫只钉条数，对「内容被改小」是瞎的）。
  assert.deepEqual(parsed.familyIndex.familiesInOrder, ['S', 'R', 'A', 'B', 'O', 'P', 'M', 'H', 'X']);
  const familySizes = new Map([...parsed.familyIndex.families].map(([f, ids]) => [f, ids.size]));
  assert.deepEqual(
    [...familySizes.entries()].sort(),
    [['A', 14], ['B', 7], ['H', 18], ['M', 18], ['O', 5], ['P', 1], ['R', 11], ['S', 12], ['X', 2]],
  );
  // §57.3 的「线」列用的拼写与 release_lines 的键并不相同（`Slice-0` vs `slice_0`），
  // 所以这张标签映射必须逐项钉住：改小它就是静默失效。
  assert.deepEqual(parsed.releaseLines.rowLabels, ['Slice-0', 'R1', 'R2', 'R3', '实验', '能力包']);
  assert.deepEqual(
    [...parsed.releaseLines.counts.entries()].sort(),
    [['capability_pack', 29], ['experimental', 2], ['r1', 37], ['r2', 9], ['r3', 1], ['slice_0', 10]],
  );
  assert.equal(parsed.releaseLines.total, 88);
  assert.equal(
    [...parsed.releaseLines.lines.values()].reduce((acc, ids) => acc + ids.size, 0),
    88,
  );

  // ④ 台账当前 MUST 为空：W71 是一次「订正版」，把两处偏差都收录了。这里把「已全部收录」
  // 这句话钉死——下次再有未收录的偏差，它要么进 §57.3，要么进台账，不能什么都不做。
  assert.deepEqual(registry.baseline_deviations, []);
});

test('a section that was renumbered fails loudly instead of passing vacuously', async () => {
  const registry = await readRegistry();
  // 解析按**章节号**定位，所以改标题措辞不该影响结果（比按标题文本匹配更耐改写）；
  // 但章节号变了 MUST 失败，否则「找不到这一节」会退化成「一切正常」。
  const renamedTitle = parseBaseline(
    replaceOnce(
      baseline.text,
      '### 57.3 发布线汇总（收缩后，正本）',
      '### 57.3 发布线汇总（措辞微调）',
    ),
  );
  assert.deepEqual(against(registry, renamedTitle), []);

  const renumbered = mutated('### 57.3 发布线汇总（收缩后，正本）', '### 59.3 发布线汇总（收缩后，正本）');
  const renumberedErrors = against(registry, renumbered);
  assert.ok(
    renumberedErrors.some(error => error.includes('§57.3 heading not found')),
    'a renumbered §57.3 must be reported as a parse failure',
  );
  assert.ok(
    renumberedErrors.some(error => error.includes('S01 is registered') && error.includes('absent from §57.3')),
    'and the consequence must still be spelled out, not swallowed',
  );

  const renumberedFamily = mutated(
    '### 48.5 验收总表索引（含 H 系列）',
    '### 48.9 验收总表索引（含 H 系列）',
  );
  assert.deepEqual(against(registry, renumberedFamily), ['§48.5 heading not found']);
});

test('§57.3 cannot disagree with itself between its table and its count line', async () => {
  const registry = await readRegistry();
  // 同一节里「表格」与「计数行」是两种写法，可以各自漂移——W71 收录 B07 时改的就是这两处。
  // 锚点带上 `计数：` 前缀：§58.5 会**逐字复述**同一条计数行（那本身就是一句散文形式的断言），
  // 不带前缀会命中两处、改错地方。
  const countOff = mutated('计数：`slice_0` 10 + `r1` **37**', '计数：`slice_0` 10 + `r1` **36**');
  assert.ok(
    against(registry, countOff).some(
      error => error.includes('§57.3 is self-inconsistent') && error.includes('`r1`') && error.includes('36'),
    ),
    'a count that disagrees with its own row must be reported',
  );
  const totalOff = mutated('+ `experimental` 2 = **88**', '+ `experimental` 2 = **87**');
  assert.ok(
    against(registry, totalOff).some(error => error.includes('total 87')),
    'a total that disagrees with the sum of its rows must be reported',
  );
  // 漏掉一条线、或凭空多出一条线，都不是「数字对得上」能盖住的。
  const omitted = mutated('+ `experimental` 2 = **88**', ' = **88**');
  assert.ok(
    against(registry, omitted).some(error => error.includes('count line omits `experimental`')),
    'a release line missing from the count line must be reported',
  );
  const invented = mutated('+ `experimental` 2 = **88**', '+ `experimental` 2 + `r4` 0 = **88**');
  assert.ok(
    against(registry, invented).some(error => error.includes('unknown release line: r4')),
    'a release line that does not exist must be reported',
  );
});

test('release-line placement drift is reported by id, from either side', async () => {
  const registry = await readRegistry();
  const parsed = parseBaseline(baseline.text);

  // 注册表侧：把 B07 从 r1 挪到 r2，基线（冻结）仍写 r1。
  const moved = structuredClone(registry);
  moved.acceptance.find(item => item.id === 'B07').release_line = 'r2';
  const movedErrors = against(moved, parsed);
  assert.ok(
    movedErrors.some(
      error => error.includes('B07 is registered on r2') && error.includes('declares it on r1'),
    ),
    'a moved id must be reported with both lines',
  );

  // 注册表侧：新增一条基线未声明的 id —— 这正是 W68 新增 B07 的形状。
  const added = structuredClone(registry);
  added.acceptance.push({...added.acceptance[0], id: 'S13', release_line: 'r1'});
  const addedErrors = against(added, parsed);
  assert.ok(
    addedErrors.some(error => error.includes('S13 is registered on r1') && error.includes('absent from §57.3')),
    'an unregistered-by-the-baseline id must be reported',
  );
  assert.ok(
    addedErrors.some(error => error.includes('§48.5 family S stops short')),
    'and so must the family index that stops short of it',
  );

  // 反方向：基线声明了、注册表没有 —— ADR 0042 记的正是这一类（家族整体缺席）。
  const removed = structuredClone(registry);
  removed.acceptance = removed.acceptance.filter(item => item.id !== 'S01');
  const removedErrors = against(removed, parsed);
  assert.ok(
    removedErrors.some(error => error.includes('S01 is declared on §57.3\'s slice_0') && error.includes('not registered')),
    'an id the baseline declares must be required to exist in the registry',
  );
  assert.ok(
    removedErrors.some(error => error.includes('§48.5 family S declares ids that are not registered: S01')),
    'and the family index must say so too',
  );

  // 基线侧：§57.3 的 R1 行落后于在册（W68→W71 的实际形状）。
  const r1Row = '| **R1** | S05–S12；R04、R05、R08–R10；A02、A03、A05–A07、A12、A13；B01、B03、B05、B06、B07；O01–O05；**H01–H04、H15、H17、H18** |';
  const lagging = parseBaseline(
    replaceOnce(baseline.text, r1Row, r1Row.replace('B01、B03、B05、B06、B07；', 'B01、B03、B05、B06；')),
  );
  assert.ok(
    against(registry, lagging).some(error => error.includes('B07 is registered on r1') && error.includes('absent from §57.3')),
    'the baseline lagging behind the registry must be reported for the id that drifted',
  );
});

test('§48.5 is the source of truth for family completeness, for every family it declares', async () => {
  const registry = await readRegistry();
  // 缩回一个家族 ⇒ 报「停在了在册项之前」；这正是 W68（B07 未收录）的形状。
  const shrunk = mutated('| B | B01–B07 | 第 16 章、53 |', '| B | B01–B06 | 第 16 章、53 |');
  const shrunkErrors = against(registry, shrunk);
  assert.ok(
    shrunkErrors.some(error => error.includes('§48.5 family B stops short of registered ids: B07')),
    'a family range that excludes a registered id must be reported by id',
  );
  // 扩到不存在的 id ⇒ 报「声明了未在册的 id」。
  const widened = mutated('| B | B01–B07 | 第 16 章、53 |', '| B | B01–B08 | 第 16 章、53 |');
  assert.ok(
    against(registry, widened).some(
      error => error.includes('§48.5 family B declares ids that are not registered: B08'),
    ),
    'a family range that invents an id must be reported',
  );
  // 判据取自 §48.5，所以**九族**都在守卫内，而不是只有 M 与 H 两张写死的表。
  const parsed = parseBaseline(baseline.text);
  assert.equal(parsed.familyIndex.families.size, 9);
  for (const family of ['S', 'R', 'A', 'B', 'O', 'P', 'M', 'H', 'X']) {
    assert.ok(parsed.familyIndex.families.has(family), `family ${family} must come from the baseline`);
  }
});

test('baseline_deviations names each pending deviation and cannot be a blanket exemption', async () => {
  const registry = await readRegistry();
  const parsed = parseBaseline(baseline.text);
  const entry = (over = {}) => ({
    id: 'S02',
    since_baseline: baseline.version,
    registered: 'W68',
    reason: '登记一个具名理由',
    ...over,
  });
  const withLedger = entries => {
    const next = structuredClone(registry);
    next.baseline_deviations = entries;
    return next;
  };
  // 没有差异却登记了一条 ⇒ 陈旧条目 MUST 失败，否则台账只会越积越长、再也不被清理。
  assert.ok(
    against(withLedger([entry({id: 'S01', registered: 'W99'})]), parsed).some(
      error => error.includes('baseline_deviations lists S01') && error.includes('clear the entry'),
    ),
    'a stale ledger entry must fail',
  );
  // 条目本身要具名：id 合法、登记轮次可读、理由非空。
  const malformed = against(withLedger([entry({id: 'ZZ9', registered: 'round-1', reason: ''})]), parsed);
  assert.ok(malformed.some(error => error.includes('invalid id')), 'a bad id must be reported');
  assert.ok(
    against(withLedger([entry({registered: 'round-1'})]), parsed).some(error =>
      error.includes('must name the registering round'),
    ),
    'the registering round must be named',
  );
  assert.ok(
    against(withLedger([entry({reason: '   '})]), parsed).some(error =>
      error.includes('must carry a non-empty reason'),
    ),
    'the reason must be non-empty',
  );
  assert.ok(
    against(withLedger({id: 'S01'}), parsed).some(error => error.includes('must be a list')),
    'the ledger must be a list',
  );
  assert.ok(
    against(withLedger([entry(), entry()]), parsed).some(error => error.includes('declares S02 twice')),
    'a duplicate entry must be reported',
  );
  // 逃生通道确实可用：真实差异 + 相称条目 ⇒ 一致。否则实现轮次会被门控锁死
  // （基线被 ADR 批准后不可变，实现轮次无法就地改 §57.3）。
  const realDeviation = structuredClone(registry);
  realDeviation.acceptance.find(item => item.id === 'B07').release_line = 'r2';
  realDeviation.baseline_deviations = [
    {
      id: 'B07',
      since_baseline: baseline.version,
      registered: 'W68',
      reason: '场景回放需要更完整的讲品证据，归属待下一轮升基线收录',
    },
  ];
  assert.deepEqual(against(realDeviation, parsed), []);
  // 但条目 MUST 与差异相称：差异是 B07、条目却写 S02 ⇒ 两条都要报。
  const mismatched = structuredClone(realDeviation);
  mismatched.baseline_deviations = [entry({reason: '张冠李戴'})];
  const mismatchedErrors = against(mismatched, parsed);
  assert.ok(
    mismatchedErrors.some(error => error.includes('B07 is registered on r2')),
    'the real difference must still be reported when the ledger names something else',
  );
  assert.ok(
    mismatchedErrors.some(error => error.includes('baseline_deviations lists S02')),
    'and the mismatched entry must be reported as stale',
  );
});

test('a registered deviation does not outlive a baseline version', async () => {
  const registry = await readRegistry();
  const parsed = parseBaseline(baseline.text);
  const deviation = over => {
    const next = structuredClone(registry);
    // 造一处**真实**差异，使条目相称——否则用例会因为别的原因失败，证不到寿命这一条。
    next.acceptance.find(item => item.id === 'B07').release_line = 'r2';
    next.baseline_deviations = [
      {id: 'B07', since_baseline: baseline.version, registered: 'W68', reason: '待下一轮升基线收录', ...over},
    ];
    return next;
  };
  // 基线换版而条目未处置 ⇒ 必须失败，且两侧版本都要给出。这正是「紧接着」的可判定形式。
  assert.deepEqual(against(deviation({}), parsed), [], 'the escape hatch must stay usable');
  const survivor = against(deviation({since_baseline: 'V3.9'}), parsed);
  assert.ok(
    survivor.some(
      error =>
        error.includes('B07 was registered against V3.9') && error.includes(`current baseline is ${baseline.version}`),
    ),
    'an entry that survived a baseline upgrade must be reported with both versions',
  );
  assert.ok(
    survivor.every(error => !error.includes('not a difference')),
    'the surviving entry is still a faithful description of a real difference — only its lifetime is wrong',
  );
  // 显式结转：改版本 + 记结转轮次 + 非空理由 ⇒ 一致。结转 MUST 留下轮次，否则「故意不收」是沉默的。
  assert.deepEqual(
    against(deviation({since_baseline: baseline.version, carried_at: 'W73'}), parsed),
    [],
    'an explicitly carried-forward entry must reconcile',
  );
  // 结转轮次必须晚于登记轮次，否则它不是结转。
  assert.ok(
    against(deviation({carried_at: 'W68'}), parsed).some(error =>
      error.includes('carried_at (W68) must be later than its registering round (W68)'),
    ),
    'a carry-forward round must be later than the registering round',
  );
  assert.ok(
    against(deviation({carried_at: 'later'}), parsed).some(error =>
      error.includes('carried_at must be a round'),
    ),
    'the carry-forward round must be readable',
  );
  // 条目 MUST 声明它相对于哪一份基线成立。
  assert.ok(
    against(deviation({since_baseline: undefined}), parsed).some(error =>
      error.includes('must name the baseline it deviates from'),
    ),
    'the entry must name the baseline it deviates from',
  );
  // 缺版本入参 ⇒ 具名报错。一条会因为入参缺失而恒真的断言比没有断言更坏。
  const noVersion = reconcile(registry, parsed, undefined);
  assert.ok(
    noVersion.some(error => error.includes('needs the current baseline version')),
    'a missing baseline version must be reported, never skipped',
  );
  assert.ok(
    reconcile(registry, parsed, 'v3.10').some(error => error.includes('needs the current baseline version')),
    'a malformed baseline version must be reported too',
  );
});

test('the baseline pointer in openspec/config.yaml resolves to the newest baseline document', () => {
  assert.equal(declaredBaselineName(configText), baseline.name);
  assert.deepEqual(pointerErrors(baseline, configText), []);
  // 一次升基线会把指针改到新文件；旧指针留下 MUST 报出来，否则门控会对着上一版跑。
  assert.ok(
    pointerErrors(
      baseline,
      replaceOnce(configText, baseline.name, '直播数字人 + 智能体系统技术方案 V3.9.md'),
    ).some(error => error.includes('declares') && error.includes('V3.9.md')),
    'a pointer left behind by a baseline upgrade must be reported',
  );
  assert.ok(
    pointerErrors(baseline, 'context: |\n  没有基线的配置文件').some(error =>
      error.includes('does not declare a baseline document path'),
    ),
    'a config without any baseline path must be reported',
  );
  // 「版本号最大者胜」只有在确实有多份候选时才算被行使过——只有一份时选择是平凡的。
  assert.ok(baseline.knownNames.length >= 2, 'expected more than one baseline document in docs/');
});

/// 注册表形状层（JSON Schema）与语义层（`scripts/contracts/*.mjs`）的一致性判据。
///
/// 两个函数都接受**注入后**的入参，这样「一致性断言真的会开火」可以在同一处被验证，
/// 而不必去改磁盘上的源码（改了就可能忘了改回来）。
function driftErrors(schema, {idSource, lines, statuses}) {
  const errors = [];
  const id = schema.properties.acceptance.items.properties.id.pattern;
  const targets = schema.properties.mapping.properties.d01_to_d14.patternProperties['^D[0-9]{2}$'].properties.targets.items.pattern;
  if (id !== idSource) errors.push(`id family pattern drifts between schema (${id}) and the scripts (${idSource})`);
  if (targets !== idSource) {
    errors.push(`id family pattern drifts between the mapping targets (${targets}) and the scripts (${idSource})`);
  }
  const lineKeys = Object.keys(schema.properties.release_lines.properties).sort();
  const enumLines = [...schema.properties.acceptance.items.properties.release_line.enum].sort();
  const scriptLines = [...lines].sort();
  if (JSON.stringify(lineKeys) !== JSON.stringify(scriptLines)) {
    errors.push(`release-line enumeration drifts: release_lines keys ${lineKeys.join(',')} vs scripts ${scriptLines.join(',')}`);
  }
  if (JSON.stringify(enumLines) !== JSON.stringify(scriptLines)) {
    errors.push(`release-line enumeration drifts: release_line.enum ${enumLines.join(',')} vs scripts ${scriptLines.join(',')}`);
  }
  const statusEnum = [...schema.properties.acceptance.items.properties.status.enum].sort();
  const mappedStatus = [
    ...schema.properties.mapping.properties.d01_to_d14.patternProperties['^D[0-9]{2}$'].properties.status.enum,
  ].sort();
  const scriptStatuses = [...statuses].sort();
  for (const [where, values] of [['status.enum', statusEnum], ['mapping status enum', mappedStatus]]) {
    if (JSON.stringify(values) !== JSON.stringify(scriptStatuses)) {
      errors.push(`status enumeration drifts: ${where} ${values.join(',')} vs scripts ${scriptStatuses.join(',')}`);
    }
  }
  return errors;
}

test('tests/registry.yaml conforms to its JSON Schema, and unknown keys are rejected', async () => {
  const {registry: validate} = await validators();
  const registry = await readRegistry();
  // 这条断言存在的原因写在 ADR 0051 决策 1：形状契约 MUST 真的被执行。
  // W72 的 `baseline_deviations` 就是在这个契约无人执行的情况下进去的。
  assert.equal(validate(registry), true, JSON.stringify(validate.errors));

  const rejection = (label, mutate) => {
    const bad = structuredClone(registry);
    mutate(bad);
    assert.equal(validate(bad), false, `the schema must reject ${label}`);
    return validate.errors ?? [];
  };
  const hits = (errors, path, keyword) => errors.some(error => error.instancePath === path && error.keyword === keyword);

  // 未知顶层键（W72 的形状）与未知条目字段：手工解析会**静默忽略**，只有 schema 拒绝它们。
  let errors = rejection('an unknown top-level key', next => {
    next.bogus_top = 1;
  });
  assert.ok(
    hits(errors, '', 'additionalProperties') &&
      errors.some(error => error.params?.additionalProperty === 'bogus_top'),
    'an unknown top-level key must be rejected by name',
  );
  errors = rejection('an unknown acceptance field', next => {
    next.acceptance[0].bogus_field = 1;
  });
  assert.ok(
    hits(errors, '/acceptance/0', 'additionalProperties') &&
      errors.some(error => error.params?.additionalProperty === 'bogus_field'),
    'an unknown acceptance field must be rejected by name',
  );
  // 值域：id / 发布线 / 状态，各报在自己那条路径上。
  assert.ok(
    hits(rejection('a malformed id', next => { next.acceptance[0].id = 'B1'; }), '/acceptance/0/id', 'pattern'),
    'a malformed id must be rejected at its own path',
  );
  assert.ok(
    hits(rejection('an unknown release line', next => { next.acceptance[0].release_line = 'r4'; }), '/acceptance/0/release_line', 'enum'),
    'an unknown release line must be rejected',
  );
  assert.ok(
    hits(rejection('an unknown status', next => { next.acceptance[0].status = 'done'; }), '/acceptance/0/status', 'enum'),
    'an unknown status must be rejected',
  );
  assert.ok(
    hits(rejection('an empty acceptance list', next => { next.acceptance = []; }), '/acceptance', 'minItems'),
    'an empty acceptance list must be rejected',
  );
  // 映射段：D01–D14 缺一不可。手写检查若只遍历实际存在的键，缺号是查不出来的。
  assert.ok(
    hits(rejection('a missing D key', next => { delete next.mapping.d01_to_d14.D09; }), '/mapping/d01_to_d14', 'required'),
    'a missing D key must be rejected',
  );
  // 台账是必需的：缺键与空台账读起来不是一回事，而「空台账」本身就是一条断言。
  assert.ok(
    hits(rejection('a missing ledger', next => { delete next.baseline_deviations; }), '', 'required'),
    'the ledger key itself must be required',
  );
});

test("the registry's shape layer and semantic layer agree, constant by constant", async () => {
  const schema = JSON.parse(await readFile('schemas/tests/registry.schema.json', 'utf8'));
  const healthy = {idSource: ID_RE.source, lines: RELEASE_LINES, statuses: STATUSES};
  assert.deepEqual(driftErrors(schema, healthy), [], 'the two layers must agree today');

  const shrunken = driftErrors(schema, {...healthy, idSource: '^(S|R)[0-9]{2}$'});
  assert.ok(
    shrunken.some(error => error.includes('id family pattern drifts')),
    'narrowing the script-side family whitelist must be reported',
  );
  const dropped = driftErrors(schema, {...healthy, lines: RELEASE_LINES.filter(line => line !== 'r3')});
  assert.ok(
    dropped.some(error => error.includes('release-line enumeration drifts')),
    'dropping a release line on one side must be reported',
  );
  const invented = driftErrors(schema, {...healthy, statuses: [...STATUSES, 'done']});
  assert.ok(
    invented.every(error => error.includes('status enumeration drifts')) && invented.length === 2,
    'adding a status on one side must be reported in both places it appears',
  );
});

/// 待裁定台账（`tests/pending-decisions.yaml`）与它的校验器。
///
/// 入参**显式注入**（与 `baseline.mjs` 的 `normalizeDeviations` 同一条纪律）：缺任一项即报错，
/// 不做「缺了就跳过对应检查」的降级——一条会被削弱的断言若保持沉默，比没有断言更坏。
const pendingLedger = YAML.parse(await readFile('tests/pending-decisions.yaml', 'utf8'));
const tracked = await readTrackedPaths('.');
assert.ok(tracked.ok, `the repository file set must be readable: ${tracked.reason ?? ''}`);
// W93：`landing_anchor` 的判据要读**落地点文件的内容**。
// 读不到时返回 `undefined`——判据会把「读不到」报成一条**具名失败**，而不是当成通过。
// 这里 MUST NOT 用 try/catch 把它吞成空串：那等于让一次宿主异常把这条判据整体关掉（铁律二）。
const readTrackedText = path => {
  try {
    return readFileSync(path, 'utf8');
  } catch {
    return undefined;
  }
};
// W94：前置的「是否仍未达成」由**两张事实表**注入，判定由判据自己做——
// 注入一个「这个前置达成了没有」的函数会让守卫自我满足（注入方算错时判据照样绿）。
const acceptanceStatus = new Map((await readRegistry()).acceptance.map(item => [item.id, item.status]));
const workItemAcceptance = baselineWorkItemAcceptance(baseline.text);
const ledgerDeps = {
  baselineVersion: baseline.version,
  baselineText: baseline.text,
  acceptanceIds: new Set(acceptanceStatus.keys()),
  workItemIds: baselineWorkItems(baseline.text),
  repositoryFiles: tracked.paths,
  readTrackedText,
  acceptanceStatus,
  workItemAcceptance,
};
const ledgerErrors = ledger => validatePendingDecisions(ledger, ledgerDeps);

test('the pending-decision ledger classifies every item and every reference resolves', async () => {
  assert.deepEqual(ledgerErrors(pendingLedger), [], 'the ledger must be clean as committed');

  // 非空洞自检：底座的抽取若恒空，后面的「引用可解析」与「逐章计数」都会恒真。
  assert.ok(
    baselineWorkItems(baseline.text).size > 20,
    'work-item extraction must actually find the baseline tables',
  );
  assert.ok(
    baselineSections(baseline.text).size > 50,
    'section extraction must actually find the baseline headings',
  );

  // 缺输入必须**报错**，而不是静默跳过对应的检查。
  for (const key of [
    'baselineVersion',
    'baselineText',
    'acceptanceIds',
    'workItemIds',
    'repositoryFiles',
    // W93：少了它，`landing_anchor` 的「内容真的在」会退化成恒真——
    // 与 `repositoryFiles` 是同一个陷阱的第二次出现。
    'readTrackedText',
    // W94：少了这两张表，前置的「是否仍未达成」会退化成恒真——同一个陷阱的第三次出现。
    'acceptanceStatus',
    'workItemAcceptance',
  ]) {
    const partial = {...ledgerDeps};
    delete partial[key];
    assert.ok(
      validatePendingDecisions(pendingLedger, partial).some(
        error => error.includes('inputs are required') && error.includes(key),
      ),
      `omitting ${key} must be reported, not silently skipped`,
    );
  }

  // 非空洞自检（W88）：跟踪集若恒空，`landing` 的「路径存在」会**恒假**（响得早、但方向相反），
  // 而「跟踪集真的取到了仓库」这件事本身必须被断言。
  assert.ok(
    tracked.paths.size > 100,
    'the tracked-path set must actually hold the repository, not a handful of files',
  );
});

test('a pending decision cannot cite a section the baseline does not have', async () => {
  const answered = structuredClone(pendingLedger);
  const entry = answered.entries.find(item => item.disposition === 'answered_by_baseline');
  entry.basis = '§99：一个基线里并不存在的章节';
  assert.ok(
    ledgerErrors(answered).some(error => error.includes(entry.id) && error.includes('§99')),
    'a basis pointing at a non-existent section must be reported',
  );

  // `basis` 之外的引用同样受检（`introduced_in` / `also_in`），否则「复述处」可以随便编。
  const strayed = structuredClone(pendingLedger);
  strayed.entries[0].also_in = ['§77'];
  assert.ok(
    ledgerErrors(strayed).some(error => error.includes('§77')),
    'a stray reference outside basis must be reported too',
  );

  // W80：`recommendation`／`closure`／`note` 同样承载 §NNN（实测 17 条），此前**整片漏扫**。
  // 三个字段各注入一次，且 MUST 指名是哪一个字段——同一个坏章节号若同时出现在多个字段里，
  // 只说条目无法定位（W79：错误的措辞会把定位引向错误的方向）。
  //
  // **样本按「字段存在」选，MUST NOT 按处置选**（W92）：本轮把 `decidable_in_repo` 的计数清到 0，
  // 而这三个字段此前是用**处置**定位的（`find(entry => entry.disposition === X)`）——一个态一旦
  // 合法地变空，用例就**失去作用对象**（原来会 `assert.ok(item)` 失败），或更坏：有人顺手补一句
  // 「找不到就跳过」，把它变成**静默失去覆盖**。判据要断言的是「**这个字段**里的 §NNN 被扫」，
  // 样本就该按「谁有这个字段」选——与 W80／W83／W88 三次确立的
  // 「触发条件 MUST 只由**字段是否存在**决定」是同一条纪律。
  for (const field of ['recommendation', 'closure', 'note']) {
    const stray = structuredClone(pendingLedger);
    const item = stray.entries.find(entry => Object.hasOwn(entry, field));
    assert.ok(item, `${field} must appear in the ledger — without a carrier the scan is untested`);
    item[field] = `${item[field]} §99.9`;
    assert.ok(
      ledgerErrors(stray).some(
        error =>
          error.includes(item.id) && error.includes(field) && error.includes('§99.9'),
      ),
      `a stray reference in ${field} must be reported and must name the field`,
    );
  }

  // 一条 answered 条目若连章节号都不给，「有依据」就是空的。
  const bare = structuredClone(pendingLedger);
  const other = bare.entries.find(item => item.disposition === 'answered_by_baseline');
  other.basis = '基线已经回答过了';
  assert.ok(
    ledgerErrors(bare).some(
      error => error.includes(other.id) && error.includes('at least one §NNN'),
    ),
    'an answered item that cites no section must be reported',
  );
});

test('the ledger checker cannot lose coverage through its own tables', async () => {
  // 校验器自身的常量表先自洽——表若分家，断言看起来还在、覆盖已经没了。
  assert.deepEqual(shapeErrors(), [], 'the checker tables must be consistent as committed');

  // 处置枚举与必填字段表 MUST 是同一集合：少了会「无必填字段」，多了会为一个不存在的处置定义字段。
  assert.deepEqual(
    [...REQUIRED_FIELD.keys()].sort(),
    [...DISPOSITIONS].sort(),
    'every disposition needs exactly one required field, and every required field needs a disposition',
  );

  // 每个处置的必填字段都必须被**某**检查覆盖：要么扫 `§NNN` 引用、要么按 id 校验、
  // 要么按**仓库成员**校验（W88 新增第三类）。
  for (const [disposition, field] of REQUIRED_FIELD) {
    assert.ok(
      CITATION_FIELDS.includes(field) ||
        ID_LIST_FIELDS.includes(field) ||
        REPO_PATH_FIELDS.includes(field),
      `${disposition}'s required field ${field} must be covered by some check`,
    );
  }

  // 直接钉内容：`CITATION_FIELDS` MUST 逐字含这三个此前漏扫的字段（W80 的核心）。
  // 只钉「关系」不够——三者一起被拿掉时，字段集同步变窄、上面的关系断言仍会全绿。
  for (const field of ['recommendation', 'closure', 'note']) {
    assert.ok(CITATION_FIELDS.includes(field), `${field} must be scanned for §NNN references`);
    assert.ok(!ID_LIST_FIELDS.includes(field), `${field} is a citation field, not an id list`);
    assert.ok(!REPO_PATH_FIELDS.includes(field), `${field} is a citation field, not a path list`);
  }
});

test('an entry field the checker does not know cannot hide from the scan', async () => {
  // W81：`CITATION_FIELDS` 是一份清单，而**清单的补集是盲区**——「按清单扫」挡得住清单上的字段
  // 被编造，挡不住**新增字段**。实测（W81 探针，改前）：给 PD-01 加 `rationale: "见 §99.9"`
  // ⇒ `validatePendingDecisions` 返回**空错误**。
  const unknown = structuredClone(pendingLedger);
  unknown.entries[0].rationale = '见 §99.9 的说明';
  assert.ok(
    ledgerErrors(unknown).some(
      error => error.includes(unknown.entries[0].id) && error.includes('rationale'),
    ),
    'a field outside the declared set must be reported, naming the entry and the field',
  );

  // **值干净也 MUST 失败**：判据断言的是「字段集封闭」，不是「这条引用坏不坏」。
  // 若只在未知字段的值含坏 `§` 时才报，一个含引用的字段会在将来某次被扫到时**无声通过**。
  const clean = structuredClone(pendingLedger);
  clean.entries[0].rationale = '见 §40.4 的说明';
  assert.ok(
    ledgerErrors(clean).some(error => error.includes('rationale')),
    'the field is rejected on its name alone — a clean value must not buy it a pass',
  );

  // `question` 是**复述字段**（它复述基线的提问），W80 的推理「复述处可以随便编」对它最直接适用，
  // 而它此前**只做非空检查、不扫引用**。加入扫描是安全侧：只有该引用不可解析时才失败。
  const asked = structuredClone(pendingLedger);
  const item = asked.entries.find(entry => entry.disposition === 'answered_by_baseline');
  item.question = `${item.question}（见 §99.9）`;
  assert.ok(
    ledgerErrors(asked).some(
      error => error.includes(item.id) && error.includes('question') && error.includes('§99.9'),
    ),
    'a restatement field must be scanned: a stray §NNN in question must be reported',
  );
});

test('the ledger field set is closed and every field has exactly one owner', async () => {
  // 逐字钉内容：`LEDGER_FIELDS` 是**独立写死**的一份声明（不是三类派生出来的），
  // 所以「三类之并 == 它」是一条**内容断言**，不是恒真式（W80：只钉关系不够，要逐字钉）。
  assert.deepEqual(
    [...LEDGER_FIELDS].sort(),
    [
      'also_in',
      'basis',
      'closure',
      'disposition',
      'id',
      'introduced_in',
      'landing',
      // W93 第五类（内容锚点）带来的新字段：它必须与 `landing` 同生同灭且等长。
      'landing_anchor',
      'note',
      'precondition',
      'question',
      'recommendation',
    ],
    'the allowed field set must be exactly the twelve declared fields',
  );

  // 各类**两两不相交**，且其并集**恰等于** `LEDGER_FIELDS`。
  // W93：四类变五类（新增内容锚点）。这一行的**类别清单**必须与 `shapeErrors()` 里的
  // `categories` 同形——漏掉 `ANCHOR_FIELDS` 会让「并集恰等于」直接失败（F8 的落点）。
  const categories = [
    CITATION_FIELDS,
    ID_LIST_FIELDS,
    REPO_PATH_FIELDS,
    ANCHOR_FIELDS,
    STRUCTURAL_FIELDS,
  ];
  assert.equal(
    new Set(categories.flat()).size,
    categories.flat().length,
    'no field may be claimed by two check categories',
  );
  assert.deepEqual(
    [...new Set(categories.flat())].sort(),
    [...LEDGER_FIELDS].sort(),
    'the five categories must partition the allowed field set',
  );
  assert.deepEqual(shapeErrors(), [], 'the partition must be internally consistent as committed');

  // **非空洞**：声明的每个字段都真的在台账里出现过。少了这条，`LEDGER_FIELDS` 可以掺进
  // 永远打不到数据的幽灵字段，而闭世界断言对它**恒真**。
  const present = new Set(pendingLedger.entries.flatMap(entry => Object.keys(entry)));
  for (const field of LEDGER_FIELDS) {
    assert.ok(present.has(field), `${field} is declared but appears in no entry — a phantom field`);
  }
  // 数据侧闭世界的**直接**形式（`ledgerErrors(pendingLedger) === []` 已隐含，这里明写）。
  for (const field of present) {
    assert.ok(LEDGER_FIELDS.includes(field), `${field} appears in the ledger but is undeclared`);
  }
});

test('a top-level field the checker does not know cannot hide from the scan', async () => {
  // W82：W81 的闭世界只覆盖**条目内部**，于是**信封**（台账顶层）是盲区。实测（探针，改前）：
  // 给顶层加 `revision: "见 §99.9"` ⇒ `validatePendingDecisions` 返回**空错误**。
  // 三个已知信封键**各有**具名检查，但「已知键各有人管」**不等于**「键集封闭」。
  const unknown = structuredClone(pendingLedger);
  unknown.revision = '见 §99.9 的说明';
  assert.ok(
    ledgerErrors(unknown).some(error => error.includes('revision')),
    'a top-level key outside the declared set must be reported by its key name',
  );

  // **值干净也 MUST 失败**：断言的是「键集封闭」，不是「这个键的值坏不坏」。
  const clean = structuredClone(pendingLedger);
  clean.revision = '见 §40.4 的说明';
  assert.ok(
    ledgerErrors(clean).some(error => error.includes('revision')),
    'the envelope key is rejected on its name alone — a clean value must not buy it a pass',
  );

  // **两个闭世界之间的缝**：把一个**条目字段名**写到顶层。条目闭世界看不见它（它在顶层），
  // 信封闭世界若不认识它，就没有任何人管它——实测改前这一条**完全静默**。
  const misplaced = structuredClone(pendingLedger);
  misplaced.question = '§99.9';
  assert.ok(
    ledgerErrors(misplaced).some(error => error.includes('question')),
    'a field name that belongs to an entry must not be silently accepted at the top level',
  );

  // 信封字段 MUST 必需：声明了它、数据里却缺它 ⇒ 具名。闭世界只挡「不认识的键」，
  // 它对「认识的键没人读」是瞎的——**声明一个字段就必须在数据里有它**。
  for (const field of TOP_LEVEL_FIELDS) {
    const missing = structuredClone(pendingLedger);
    delete missing[field];
    assert.ok(
      ledgerErrors(missing).some(
        error => error.includes(`envelope field ${field} is missing`),
      ),
      `declaring ${field} but omitting it must be reported by name`,
    );
  }
});

test('the envelope field set is closed and disjoint from the entry field set', async () => {
  // 逐字钉内容：`TOP_LEVEL_FIELDS` 是**独立写死**的声明，不是从「现有检查读了哪些键」派生的。
  assert.deepEqual(
    [...TOP_LEVEL_FIELDS].sort(),
    ['baseline', 'entries', 'schema_version'],
    'the allowed envelope field set must be exactly the three declared keys',
  );

  // **两个层级 MUST 不相交**：一个字段名只属于一个层级。若某个名字同时被两边声明，
  // 把它写到错的那一层就**两个闭世界都放行**（正是上面那条静默形状）。
  for (const field of TOP_LEVEL_FIELDS) {
    assert.ok(
      !LEDGER_FIELDS.includes(field),
      `${field} must belong to exactly one level, not both`,
    );
  }
  assert.deepEqual(shapeErrors(), [], 'the two field sets must stay disjoint as committed');

  // **非空洞**：每个声明的信封字段都真的在台账顶层出现。少了这条，`TOP_LEVEL_FIELDS`
  // 可以掺进永远打不到数据的幽灵字段——而「必需」断言会在**每个**真实台账上失败（响得早），
  // 但若有人「为了让它绿」而往台账里补上那个键，就再没有信号了。
  for (const field of TOP_LEVEL_FIELDS) {
    assert.ok(
      Object.hasOwn(pendingLedger, field),
      `${field} is declared as an envelope field but the ledger does not carry it`,
    );
  }
  for (const key of Object.keys(pendingLedger)) {
    assert.ok(TOP_LEVEL_FIELDS.includes(key), `${key} appears on the ledger but is undeclared`);
  }
});

test('each disposition has to pay for itself with its own required field', async () => {
  const cases = [
    ['answered_by_baseline', 'basis'],
    ['decidable_in_repo', 'recommendation'],
    ['decided_in_repo', 'landing'],
    ['needs_precondition', 'precondition'],
    ['needs_external', 'closure'],
    ['out_of_scope', 'note'],
  ];
  let injected = 0;
  for (const [disposition, field] of cases) {
    const ledger = structuredClone(pendingLedger);
    const entry = ledger.entries.find(item => item.disposition === disposition);
    // **空的处置**（W92 新增）：`decidable_in_repo` 的计数已在 W92 清到 0 ⇒ 数据侧没有实例可注入。
    // 这不是「找不到就跳过」——跳过是**静默**的，而这里是**被断言的事实**：计数说 0，数据里就
    // MUST 确实没有该态条目（两份声明**双向一致**）。该处置的「必填字段」关系与数据无关，
    // 由 `shapeErrors()` 与上面的常量表断言看守（它们不依赖任何一条数据）。
    const declared = DISPOSITION_COUNTS.get(disposition);
    assert.notEqual(declared, undefined, `${disposition} must carry a declared count`);
    if (declared === 0) {
      assert.equal(
        entry,
        undefined,
        `${disposition} declares zero entries but the ledger carries one — count and data disagree`,
      );
      continue;
    }
    assert.ok(entry, `${disposition} must be represented in the ledger`);
    if (Array.isArray(entry[field])) entry[field] = [];
    else delete entry[field];
    assert.ok(
      ledgerErrors(ledger).some(error => error.includes(entry.id) && error.includes(field)),
      `${disposition} without ${field} must be reported`,
    );
    injected += 1;
  }
  assert.ok(
    injected >= cases.length - 1,
    'the injection must actually run over the non-empty dispositions',
  );

  // **本轮成果断言**（W92）：没有任何条目还停在「可在仓库内决定」的门口。
  // 这条与 `DISPOSITION_COUNTS` 的 0 是**同一件事的两面**——计数是**声明**，下面是**数据**。
  assert.equal(
    DISPOSITION_COUNTS.get('decidable_in_repo'),
    0,
    'the decidable-in-repo queue must be emptied — every recommendation is settled or reclassified',
  );
  assert.equal(
    pendingLedger.entries.filter(item => item.disposition === 'decidable_in_repo').length,
    0,
    'no entry may remain decidable-in-repo once the count is declared zero',
  );

  const outside = structuredClone(pendingLedger);
  outside.entries[0].disposition = 'to_be_decided_later';
  assert.ok(
    ledgerErrors(outside).some(error => error.includes('invalid disposition')),
    'a disposition outside the closed enumeration must be reported',
  );
});

test('a precondition must name something that exists', async () => {
  const ghostAcceptance = structuredClone(pendingLedger);
  const pick = ledger => ledger.entries.find(item => item.disposition === 'needs_precondition');
  pick(ghostAcceptance).precondition = ['H99'];
  assert.ok(
    ledgerErrors(ghostAcceptance).some(
      error => error.includes('H99') && error.includes('not a registry acceptance id'),
    ),
    'a precondition naming a non-existent acceptance id must be reported',
  );

  const ghostWorkItem = structuredClone(pendingLedger);
  pick(ghostWorkItem).precondition = ['W99'];
  assert.ok(
    ledgerErrors(ghostWorkItem).some(error => error.includes('W99')),
    'a precondition naming a work item the baseline does not have must be reported',
  );

  const nonsense = structuredClone(pendingLedger);
  pick(nonsense).precondition = ['以后再说'];
  assert.ok(
    ledgerErrors(nonsense).some(error => error.includes('neither a registry acceptance id')),
    'a precondition that names nothing checkable must be reported',
  );
});

test('an id reference must resolve wherever its field appears, not only under its own disposition', async () => {
  // W83：`precondition` 是 `ID_LIST_FIELDS` 的唯一成员，而它的 id 校验此前**按处置触发**
  // ——只在 `disposition === 'needs_precondition'` 时运行。挂到别的处置上**完全静默**
  // （探针：`PD-01`（`needs_external`）加 `precondition: ["W99"]` ⇒ 空错误）。
  // 一个字段由谁解析它的引用，是**字段的属性**，与这条数据当前是什么处置无关。
  //
  // 本用例**泛化到清单**：对 `ID_LIST_FIELDS` 的**每个**成员，在其专属处置**之外**注入一个
  // 不可解析的引用 ⇒ MUST 失败并指名「条目 + 字段 + 值」。将来再加 id 清单字段，本用例自动扩展。
  let injected = 0;
  for (const field of ID_LIST_FIELDS) {
    const ownDisposition = [...REQUIRED_FIELD].find(([, owned]) => owned === field)?.[0];
    assert.ok(ownDisposition, `${field} must be the required field of exactly one disposition`);
    const host = structuredClone(pendingLedger);
    const target = host.entries.find(entry => entry.disposition !== ownDisposition);
    assert.ok(target, `an entry whose disposition is not ${ownDisposition} must exist to host the probe`);
    for (const bad of ['W99', 'H99', '不是 id']) {
      const probe = structuredClone(host);
      probe.entries.find(entry => entry.id === target.id)[field] = [bad];
      assert.ok(
        ledgerErrors(probe).some(
          error =>
            error.includes(target.id) && error.includes(field) && error.includes(bad),
        ),
        `${field} on a ${target.disposition} entry must name the entry, the field and the value`,
      );
      injected += 1;
    }
    // 三种坏值走**三条不同分支**（作业项 / 验收 id / 两者都不是）——都 MUST 被解析检查抓住，
    // 而不是被「必填字段」的那条检查顺带挡住：这里的宿主条目**不缺**它自己的必填字段。
    assert.ok(
      ledgerErrors(host).length === 0,
      `the host entry without the injected field must stay clean for ${field}`,
    );
  }
  assert.ok(injected >= 3, 'the injection must actually run over the declared id-list fields');

  // **反向注入**（不该红时不红）：一个**可解析**的 id 出现在**非专属**处置上 ⇒ MUST NOT 红。
  // 本轮拒绝的是「**不可解析**的引用」，不是「**放错位置**的引用」——把位置也一并拒绝会把
  // 合法数据判红（假阳性方向，design 决策 4）。
  const legal = structuredClone(pendingLedger);
  const carrier = legal.entries.find(entry => entry.disposition !== 'needs_precondition');
  // W94 收紧了「可解析」的前提：原先本用例取 `workItemIds` 的第一个元素（实际是 `W00`），
  // 而基线工作项表里有 **11 行（W00–W10）第 3 列是路径／包名、没有关联验收** ⇒ 它们的
  // 达成状态**不可判定** ⇒ 现在 MUST 具名失败，MUST NOT 静默放行。
  // ⇒ 本用例改用「有非空关联验收、且那些验收**全部仍未达成**」的工作项（实测 `W61` → H11／H12 均 `pending`）。
  // 这不是放宽断言，而是把前提换成**本轮口径下真正还悬空**的那个样本。
  const stillWaiting = [...ledgerDeps.workItemAcceptance].find(
    ([, ids]) =>
      ids.length > 0 &&
      ids.every(item => UNMET_STATUSES.includes(ledgerDeps.acceptanceStatus.get(item))),
  );
  assert.ok(
    stillWaiting,
    'the baseline must name at least one work item whose acceptance ids are all still unmet',
  );
  carrier.precondition = [stillWaiting[0]];
  assert.deepEqual(
    ledgerErrors(legal),
    [],
    'a resolvable precondition outside its own disposition must not be reported',
  );

  // **不可判定 MUST NOT 被当成「仍未达成」而放行**（铁律二：「没看到」与「出错了」类型上分开）。
  const undecidable = structuredClone(pendingLedger);
  const noBacking = [...ledgerDeps.workItemAcceptance].find(([, ids]) => ids.length === 0);
  assert.ok(noBacking, 'the baseline must carry a work item with no acceptance ids');
  undecidable.entries.find(entry => entry.id === carrier.id).precondition = [noBacking[0]];
  assert.ok(
    ledgerErrors(undecidable).some(
      error =>
        error.includes(carrier.id) &&
        error.includes('precondition') &&
        error.includes(noBacking[0]) &&
        error.includes('cannot be decided'),
    ),
    'a work item with no acceptance ids must be reported, not treated as still unmet',
  );
});

test('a precondition must still be unmet — waiting on a met precondition is a stale claim', async () => {
  // W94 探针实证：把 `PD-26.precondition` 换成 `["S03"]`（registry 里 `status: tested`）
  // ⇒ 改前 **51/51 全绿**。即「还在等」是一条**判据放过的空声明**——等一个已经达成的前置，
  // 与等一个真正悬空的前置，判据此前给的是同一个答案。
  // 这条缝的代价是**遗忘**：前置一旦达成，台账不会有任何信号，条目会一直挂在"等前置"上，
  // 而它实际上已经可以裁定了。六个态里只有这一个态的**有效期会随仓库自身进展而过期**。

  // 非空洞自检：取值域 MUST 真的覆盖 registry 在用的取值，否则「已达成」恒不成立。
  const inUse = new Set(acceptanceStatus.values());
  assert.ok(inUse.size > 1, 'the registry must carry more than one status value');
  for (const status of inUse) {
    assert.ok(
      ACCEPTANCE_STATUSES.includes(status),
      `status ${status} is in use by the registry but is not declared in the status domain`,
    );
    assert.ok(
      MET_STATUSES.includes(status) || UNMET_STATUSES.includes(status),
      `status ${status} must be claimed by either MET or UNMET`,
    );
  }

  // 本轮是**纯判据**改动 ⇒ 4 条前置 MUST 全部真的仍未达成（零数据改动的前提）。
  const waiting = pendingLedger.entries.filter(entry => entry.disposition === 'needs_precondition');
  assert.ok(waiting.length > 0, 'the ledger must carry entries that wait on preconditions');
  for (const entry of waiting) {
    for (const item of entry.precondition ?? []) {
      const statuses = /^W\d{2}$/.test(item)
        ? (workItemAcceptance.get(item) ?? []).map(id => acceptanceStatus.get(id))
        : [acceptanceStatus.get(item)];
      assert.ok(
        statuses.length > 0 && statuses.every(value => UNMET_STATUSES.includes(value)),
        `${entry.id}: precondition ${item} must still be unmet as committed`,
      );
    }
  }

  // ① **已达成 ⇒ 红**（探针 ① 的回归守卫）。
  const metId = [...acceptanceStatus].find(([, status]) => MET_STATUSES.includes(status))?.[0];
  assert.ok(metId, 'the registry must carry at least one met acceptance item');
  const stale = structuredClone(pendingLedger);
  const victim = stale.entries.find(entry => entry.disposition === 'needs_precondition');
  victim.precondition = [metId];
  assert.ok(
    ledgerErrors(stale).some(
      error =>
        error.includes(victim.id) &&
        error.includes('precondition') &&
        error.includes(metId) &&
        error.includes('already'),
    ),
    'a precondition that is already met must be reported as a stale claim',
  );

  // ② **W 系列经关联验收派生 ⇒ 红**。基线工作项表**没有状态列**，只有
  // `编号 | 内容 | 关联验收 | 发布线` ⇒ 措辞只说到派生那一层，MUST NOT 冒充"工作项已完成"。
  const backed = [...workItemAcceptance].find(([, ids]) => ids.length > 0);
  assert.ok(backed, 'the baseline must name at least one work item with acceptance ids');
  const fakeMet = new Map(acceptanceStatus);
  for (const id of backed[1]) fakeMet.set(id, 'tested');
  const derived = structuredClone(pendingLedger);
  const withBacking = derived.entries.find(entry => entry.disposition === 'needs_precondition');
  withBacking.precondition = [backed[0]];
  assert.ok(
    validatePendingDecisions(derived, {...ledgerDeps, acceptanceStatus: fakeMet}).some(
      error =>
        error.includes(withBacking.id) &&
        error.includes(backed[0]) &&
        error.includes('backed by') &&
        error.includes('already met'),
    ),
    'a work item whose acceptance ids are all met must be reported through the derivation',
  );

  // ③ **触发条件按字段存在**：已达成的前置挂到**别的**处置上 ⇒ 一样红。
  const ghost = structuredClone(pendingLedger);
  const other = ghost.entries.find(entry => entry.disposition !== 'needs_precondition');
  other.precondition = [metId];
  assert.ok(
    ledgerErrors(ghost).some(
      error => error.includes(other.id) && error.includes(metId) && error.includes('already'),
    ),
    'a met precondition must be reported wherever its field appears, not only under its own disposition',
  );

  // ④ **反向（不该红时不红）：`not_applicable` 仍是未达成**。
  // 依据是**实测反例**而非字面直觉：`S12` 的 `notes` 明写「依赖真实平台规则核对」⇒ 它**仍未闭合**。
  // 把它算成已达成，会漏掉整整一类「不适用但仍悬空」的前置。
  const unmetId = [...acceptanceStatus].find(([, status]) => status === 'not_applicable')?.[0];
  assert.ok(unmetId, 'the registry must carry a not_applicable acceptance item as the counter-example');
  const stillOk = structuredClone(pendingLedger);
  stillOk.entries.find(entry => entry.id === victim.id).precondition = [unmetId];
  assert.deepEqual(
    ledgerErrors(stillOk),
    [],
    'not_applicable is still unmet — it must not be reported (counter-example: S12 depends on real platform rules)',
  );

  // ⑤ **取值域外的 `status` ⇒ 具名失败**，MUST NOT 被当成「仍未达成」而放行。
  const domainBreach = new Map(acceptanceStatus);
  domainBreach.set('H05', 'deferred');
  const outside = structuredClone(pendingLedger);
  outside.entries.find(entry => entry.id === victim.id).precondition = ['H05'];
  assert.ok(
    validatePendingDecisions(outside, {...ledgerDeps, acceptanceStatus: domainBreach}).some(
      error => error.includes('outside the declared status domain'),
    ),
    'a status outside the declared domain must be reported, not silently treated as unmet',
  );

  // ⑥ **两张事实表的形状 MUST 被守住**：给了但给错形状 ⇒ 具名报，而不是在 `.get` 上抛异常。
  // 只查 `undefined` 会放过「给了但给错形状」——那时判据会在判据内部崩掉而不是报一条错。
  assert.ok(
    validatePendingDecisions(pendingLedger, {...ledgerDeps, acceptanceStatus: {}}).some(
      error => error.includes('acceptanceStatus must be a Map'),
    ),
    'a wrongly shaped acceptanceStatus must be reported',
  );
  assert.ok(
    validatePendingDecisions(pendingLedger, {...ledgerDeps, workItemAcceptance: []}).some(
      error => error.includes('workItemAcceptance must be a Map'),
    ),
    'a wrongly shaped workItemAcceptance must be reported',
  );
});

test('a landing must name a path the repository actually tracks', async () => {
  // W88：`decided_in_repo` 的必填字段是**落地点**。它 MUST 是可判的，否则「结清」只是换了个词。
  // 判据的输入是**仓库成员集**（`.git/index` 派生，W75），不是工作区清单——
  // 这条正是 W75 的 F2 根因的**直接**回归守卫。
  const clean = structuredClone(pendingLedger);
  const settled = clean.entries.find(entry => entry.disposition === 'decided_in_repo');
  assert.ok(settled, 'the ledger must carry at least one settled entry');
  assert.ok(
    settled.landing.length > 0 && settled.landing.every(item => tracked.paths.has(item)),
    'as committed, every landing path must be a tracked file',
  );

  // ① 不存在的路径 ⇒ 具名（条目 + 字段 + 值）。
  const ghost = structuredClone(pendingLedger);
  const ghostEntry = ghost.entries.find(entry => entry.disposition === 'decided_in_repo');
  ghostEntry.landing = [...ghostEntry.landing, 'crates/nai-security/src/no-such-file.rs'];
  // W93：耦合断言按**字段存在**触发 ⇒ `landing` 加一项就必须同时补一个锚点，
  // 否则会多出一条「长度不等」的错。补上它，本注入红的**唯一**原因才是那个不存在的路径
  // （W92 教训：「红」不等于「红在我以为的地方」——多一条错就可能把定位引向别处）。
  ghostEntry.landing_anchor = [...ghostEntry.landing_anchor, 'probe'];
  assert.ok(
    ledgerErrors(ghost).some(
      error =>
        error.includes(ghostEntry.id) &&
        error.includes('landing') &&
        error.includes('no-such-file.rs'),
    ),
    'a landing path that does not exist must be reported with the entry, the field and the value',
  );

  // ② **磁盘上存在 ≠ 仓库成员**：这一条把「用 `.git/index`」与「用 `readdir`」分开。
  // 若判据改成遍历工作区，`node_modules` 会被算成仓库成员 ⇒ 本注入将**静默通过**。
  const probeDir = 'node_modules';
  assert.ok(
    (await stat(probeDir)).isDirectory(),
    'the probe needs a directory that exists on disk so the two readings can disagree',
  );
  assert.ok(
    !tracked.paths.has(probeDir),
    'the probe needs a path the index does not hold — otherwise it proves nothing',
  );
  const untracked = structuredClone(pendingLedger);
  const untrackedEntry = untracked.entries.find(entry => entry.disposition === 'decided_in_repo');
  untrackedEntry.landing = [probeDir];
  // 同上：补锚点使两侧等长。锚点检查本身也会因 `node_modules` 是**目录**而报「不可读」
  // （读不到 MUST 具名失败，MUST NOT 静默降级）——那是新判据的预期行为，且同样含 `probeDir`，
  // 不掩盖本用例真正要验的那一条「路径未被跟踪」。
  untrackedEntry.landing_anchor = ['probe'];
  assert.ok(
    ledgerErrors(untracked).some(
      error => error.includes(untrackedEntry.id) && error.includes('landing') && error.includes(probeDir),
    ),
    'a path that exists on disk but is untracked must be rejected — the index decides, not the filesystem',
  );

  // **反向注入**（不该红时不红）：一个**可解析**的路径出现在**别的**处置上 ⇒ MUST NOT 红。
  const legal = structuredClone(pendingLedger);
  const carrier = legal.entries.find(entry => entry.disposition === 'needs_external');
  carrier.landing = ['crates/nai-security/src/lib.rs'];
  // W93 **合法地改了这个用例的前提**：耦合断言按字段存在触发，与处置无关 ⇒
  // 挂到别的处置上的 `landing` 同样 MUST 配锚点（锚点也得真的在那个文件里）。
  // 只给 `landing` 不给 `landing_anchor` 现在会红，而红的原因是**缺内容证据**，
  // 不是「放错位置」——本用例要验的是后者，故必须把前者的条件补齐（与 W92「清空一个态」
  // 暴露既有用例隐含依赖同族，MUST 在报告与 ADR 里明写）。
  carrier.landing_anchor = ['NativeVault'];
  assert.deepEqual(
    ledgerErrors(legal),
    [],
    'a resolvable landing outside its own disposition must not be reported',
  );
});

test('a landing must carry content evidence that is actually in the file', async () => {
  // W93 母题：`decided_in_repo` 断言「决定已作出**且已落地**」，而原先判据只验 `landing`
  // 的每个元素是**已跟踪路径**。核实阶段跑过两个探针（都改真实文件 → 跑 `node --test`）：
  //   ① 把 `PD-22.landing` 换成 `README.md`（真实存在、已被 git 跟踪）⇒ **50/50 全绿**；
  //   ② `landing` 不动，删掉 `PD-24` 在规格里的落地句 ⇒ **50/50 全绿**。
  // ⇒ 本用例是那两个探针的**回归守卫**：同样的改动现在 MUST 变红。
  //
  // 锚点**守不住**什么（如实披露，MUST NOT 被读作"已落地已被证明"）：它验的是
  // 「你指的那个文件里有这个串」，**不**验「这个串只应出现在那里」。把 `landing` 指到另一个
  // 恰好也含该锚点的文件，判据仍会绿——这是**签名 + 观测**的分工，不是全知验证。
  const clean = structuredClone(pendingLedger);
  const settled = clean.entries.filter(entry => entry.disposition === 'decided_in_repo');
  assert.ok(settled.length > 0, 'the ledger must carry settled entries');
  for (const entry of settled) {
    assert.ok(
      Array.isArray(entry.landing_anchor) && entry.landing_anchor.length === entry.landing.length,
      `${entry.id}: every landing site must carry exactly one content anchor`,
    );
    for (let index = 0; index < entry.landing.length; index += 1) {
      const content = readTrackedText(entry.landing[index]);
      assert.ok(
        content !== undefined && content.includes(entry.landing_anchor[index]),
        `${entry.id}: anchor ${index} must actually be present in ${entry.landing[index]}`,
      );
    }
  }

  // ① **探针①的回归守卫**：落地点换成另一个已跟踪文件，锚点留在原处 ⇒ MUST 红。
  const swapped = structuredClone(pendingLedger);
  const target = swapped.entries.find(entry => entry.disposition === 'decided_in_repo');
  target.landing = ['README.md'];
  assert.ok(
    ledgerErrors(swapped).some(
      error => error.includes(target.id) && error.includes('landing_anchor') && error.includes('README.md'),
    ),
    'pointing the landing at another tracked file must be reported — a path alone is not evidence',
  );

  // ② **探针②的回归守卫**：落地点不动，把锚点改成文件里没有的串 ⇒ MUST 红。
  const revoked = structuredClone(pendingLedger);
  const revokedEntry = revoked.entries.find(entry => entry.disposition === 'decided_in_repo');
  revokedEntry.landing_anchor = ['这段内容绝不会出现在那个文件里'];
  assert.ok(
    ledgerErrors(revoked).some(
      error =>
        error.includes(revokedEntry.id) &&
        error.includes('landing_anchor') &&
        error.includes('is not present in'),
    ),
    'a landing whose content is gone must be reported — this is the hole the probes walked through',
  );

  // ③ 两侧长度不等 ⇒ 对应关系不存在 ⇒ MUST 红。
  const short = structuredClone(pendingLedger);
  const shortEntry = short.entries.find(entry => entry.disposition === 'decided_in_repo');
  assert.ok(shortEntry.landing.length > 1, 'the probe needs a settled entry with more than one site');
  shortEntry.landing_anchor = shortEntry.landing_anchor.slice(0, -1);
  assert.ok(
    ledgerErrors(short).some(
      error => error.includes(shortEntry.id) && error.includes('correspond one-to-one'),
    ),
    'an anchor list that does not match its landing list must be reported',
  );

  // ④ 有 `landing` 却没有 `landing_anchor` ⇒ MUST 红（锚点若可选，这就是绕过判据的那条缝）。
  const bare = structuredClone(pendingLedger);
  const bareEntry = bare.entries.find(entry => entry.disposition === 'decided_in_repo');
  delete bareEntry.landing_anchor;
  assert.ok(
    ledgerErrors(bare).some(
      error => error.includes(bareEntry.id) && error.includes('landing_anchor is missing'),
    ),
    'a landing without content evidence must be reported',
  );

  // ⑤ 锚点给空串 ⇒ MUST 红。
  const blank = structuredClone(pendingLedger);
  const blankEntry = blank.entries.find(entry => entry.disposition === 'decided_in_repo');
  blankEntry.landing_anchor = [''];
  assert.ok(
    ledgerErrors(blank).some(
      error => error.includes(blankEntry.id) && error.includes('must be non-empty strings'),
    ),
    'an empty anchor must be reported — an empty string is present in every file',
  );

  // ⑥ **按字段存在触发**（不按处置）：锚点挂在**别的**处置上且不在文件里 ⇒ MUST 红。
  const elsewhere = structuredClone(pendingLedger);
  const carrier = elsewhere.entries.find(entry => entry.disposition === 'needs_external');
  carrier.landing = ['crates/nai-security/src/lib.rs'];
  carrier.landing_anchor = ['这段内容绝不在 lib.rs 里'];
  assert.ok(
    ledgerErrors(elsewhere).some(
      error => error.includes(carrier.id) && error.includes('landing_anchor'),
    ),
    'an anchor outside its own disposition must still be checked — ownership is a property of the field',
  );

  // ⑦ **读不到文件 MUST 具名失败**，MUST NOT 静默降级（铁律二：「没看到」与「出错了」分开）。
  const blind = {...ledgerDeps, readTrackedText: () => undefined};
  assert.ok(
    validatePendingDecisions(pendingLedger, blind).some(error => error.includes('unreadable')),
    'an unreadable landing file must be reported, not silently passed',
  );
  // 抛错同理：MUST NOT 把宿主异常当成「通过」。
  const throwing = {...ledgerDeps, readTrackedText: () => { throw new Error('EACCES'); }};
  assert.ok(
    validatePendingDecisions(pendingLedger, throwing).some(error => error.includes('could not read')),
    'a reader that throws must be reported, not swallowed',
  );
});

test('a repository-path reference must resolve wherever its field appears, not only under its own disposition', async () => {
  // W83 的同一条纪律，泛化到 W88 新增的第三类（`REPO_PATH_FIELDS`）：
  // 「一个字段由谁解析它的引用」是**字段的属性**，与这条数据当前是什么处置**无关**。
  // 若把路径校验写成 `disposition === 'decided_in_repo' && …`，`landing` 挂到别的处置上就**完全静默**。
  let injected = 0;
  for (const field of REPO_PATH_FIELDS) {
    const ownDisposition = [...REQUIRED_FIELD].find(([, owned]) => owned === field)?.[0];
    assert.ok(ownDisposition, `${field} must be the required field of exactly one disposition`);
    const host = structuredClone(pendingLedger);
    const target = host.entries.find(entry => entry.disposition !== ownDisposition);
    assert.ok(target, `an entry whose disposition is not ${ownDisposition} must exist to host the probe`);
    for (const bad of ['no/such/file.rs', 'node_modules']) {
      const probe = structuredClone(host);
      probe.entries.find(entry => entry.id === target.id)[field] = [bad];
      assert.ok(
        ledgerErrors(probe).some(
          error =>
            error.includes(target.id) && error.includes(field) && error.includes(bad),
        ),
        `${field} on a ${target.disposition} entry must name the entry, the field and the value`,
      );
      injected += 1;
    }
    // 宿主条目**不缺**它自己的必填字段 ⇒ 报错只能来自路径解析，不会被别的检查顺带挡住。
    assert.ok(
      ledgerErrors(host).length === 0,
      `the host entry without the injected field must stay clean for ${field}`,
    );
  }
  assert.ok(injected >= 2, 'the injection must actually run over the declared repository-path fields');
});

test('a landing must be a list, and that list must be the required field of exactly one disposition', async () => {
  // 必填字段的「清单型 / 文本型」判定 MUST 由**类别表**决定，MUST NOT 靠写死字段名。
  // 改前的实现写着 `field === 'precondition'` ⇒ 新字段 `landing` 会被当成**文本**判：
  // 一个非空字符串（而不是数组）会被当成合格，而它一个有效路径都没给。
  const asText = structuredClone(pendingLedger);
  const entry = asText.entries.find(item => item.disposition === 'decided_in_repo');
  entry.landing = 'crates/nai-security/src/lib.rs';
  assert.ok(
    ledgerErrors(asText).some(
      error => error.includes(entry.id) && error.includes('landing'),
    ),
    'a landing given as a bare string must be reported — the required shape is a list',
  );

  // 空列表同样不是「落地点」。
  const empty = structuredClone(pendingLedger);
  const emptied = empty.entries.find(item => item.disposition === 'decided_in_repo');
  emptied.landing = [];
  assert.ok(
    ledgerErrors(empty).some(
      error => error.includes(emptied.id) && error.includes('landing'),
    ),
    'an empty landing must be reported — a settled item that points nowhere has not settled',
  );

  // 类别归属是**声明**：每个路径清单字段都必须是**恰好一个**处置的必填字段。
  // 少了这条，`REPO_PATH_FIELDS` 可以掺进一个谁都不需要的字段（闭世界对它仍恒真）。
  for (const field of REPO_PATH_FIELDS) {
    const owners = [...REQUIRED_FIELD].filter(([, owned]) => owned === field);
    assert.equal(
      owners.length,
      1,
      `${field} must be the required field of exactly one disposition`,
    );
  }
});

test('the reference scan domain is a declared single source over every reference category', async () => {
  // 逐字钉内容：`REFERENCE_FIELDS` 是**独立写死**的声明，不是
  // `[...CITATION_FIELDS, ...ID_LIST_FIELDS]` 派生出来的——派生出的「并集 == 它」是**恒真式**
  // （W80：只钉「关系」不够，要逐字钉内容）。
  assert.deepEqual(
    [...REFERENCE_FIELDS].sort(),
    [
      'also_in',
      'basis',
      'closure',
      'introduced_in',
      'landing',
      'note',
      'precondition',
      'question',
      'recommendation',
    ],
    'the reference scan domain must be exactly the nine declared fields',
  );

  // 与类别划分**双向相等**：① 声明了却无类别认领 ⇒ 它的引用**没人解析**；
  // ② 被类别认领却不在声明集里 ⇒ 引用扫描会**跳过它**。（方向②正是本轮洞的近亲：
  // `precondition` 曾在类别里，却只在一种处置下被扫。）
  assert.deepEqual(
    [...REFERENCE_FIELDS].sort(),
    [...new Set([...CITATION_FIELDS, ...ID_LIST_FIELDS, ...REPO_PATH_FIELDS])].sort(),
    'the reference domain must equal the union of the three reference categories',
  );
  assert.deepEqual(shapeErrors(), [], 'the reference domain must be consistent as committed');

  // **非空洞**：声明的每个引用字段都真的在台账里出现过。少了这条，`REFERENCE_FIELDS` 可以掺进
  // 永远打不到数据的幽灵字段——而「双向相等」对它仍然成立。
  const present = new Set(pendingLedger.entries.flatMap(entry => Object.keys(entry)));
  for (const field of REFERENCE_FIELDS) {
    assert.ok(
      present.has(field),
      `${field} is declared as a reference field but appears in no entry — a phantom field`,
    );
  }
});

test('the ledger cannot shrink, be reclassified, or address a stale baseline', async () => {
  const shrunk = structuredClone(pendingLedger);
  const lost = shrunk.entries.pop();
  assert.ok(
    ledgerErrors(shrunk).some(error => error.includes(`${shrunk.entries.length} entries`)),
    `dropping ${lost.id} must be reported by count`,
  );

  // 总数不变但**类别被改**：只校验总数的实现看不见它。
  const reclassified = structuredClone(pendingLedger);
  const moved = reclassified.entries.find(item => item.disposition === 'answered_by_baseline');
  moved.disposition = 'needs_external';
  moved.closure = '外部确认';
  assert.ok(
    ledgerErrors(reclassified).some(error => error.includes('answered_by_baseline covers')),
    'moving an answered item out of its class must be reported even when the total still matches',
  );

  const stale = structuredClone(pendingLedger);
  stale.baseline = 'V3.9';
  assert.ok(
    ledgerErrors(stale).some(error => error.includes('declares baseline')),
    'a ledger written against an older baseline must be reported',
  );

  const duplicated = structuredClone(pendingLedger);
  duplicated.entries[1].id = duplicated.entries[0].id;
  assert.ok(
    ledgerErrors(duplicated).some(error => error.includes('duplicate entry id')),
    'duplicate ids must be reported',
  );
});

// ---------------------------------------------------------------------------
// 一个预算一份定义（w77）
//
// 不变式：**每类有界工作只有一个预算定义点，且该定义紧挨着干这件事的人。**
// 判据读 Rust 源码、不读自己（判据在 `.mjs`，被扫的是 `.rs`）—— W73 的消费守卫就是
// 因为把自己文件里的字面量算成了「消费」而**自我满足**。
//
// 这里的遍历**故意不与 `tests/contracts/schema.test.mjs` 的 `codeFiles` 合并**：那个函数
// 服务的是「可扫描扩展名闭包」，它的排除集与扩展名表是一组约束；本文件只要 `.rs` 与
// 「预算定义」这两个概念。把两者绑在一起，任一方调整排除集都会牵动另一方
// （W66「不合并不同维度」的同一条理由）。
const RUST_SCAN_SKIP_DIRS = ['node_modules', 'target', 'graphify-out', 'dist'];

async function rustSources(dir = '.', out = []) {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const path = dir === '.' ? entry.name : `${dir}/${entry.name}`;
    if (entry.isDirectory()) {
      if (entry.name.startsWith('.') || RUST_SCAN_SKIP_DIRS.includes(entry.name)) continue;
      await rustSources(path, out);
    } else if (entry.name.endsWith('.rs')) {
      out.push(path);
    }
  }
  return out.sort();
}

/// 整行注释不算命中：注释里提一句常量名，既不是**定义**它也不是**使用**它。
/// 只识别 `//` 起首的行 —— 行尾注释仍算命中只是**多报**（方向安全），而误切会伤到
/// `https://` 这类内容（W74 记下的理由）。
const withoutLineComments = (text) =>
  text.split('\n').filter((line) => !/^\s*\/\//.test(line)).join('\n');

const rustTexts = new Map(
  await Promise.all(
    (await rustSources()).map(async (path) => [
      path,
      withoutLineComments(await readFile(path, 'utf8')),
    ]),
  ),
);

/// 返回 `path × count` 的清单（空数组＝全仓 0 处）。**清单的形状让断言能同时钉住
/// 「有几处」与「在哪几处」**——只断言总数对「位置被换掉」是瞎的（W66 教训）。
function rustHits(literal) {
  const hits = [];
  for (const [path, text] of rustTexts) {
    const count = text.split(literal).length - 1;
    if (count > 0) hits.push(`${path} × ${count}`);
  }
  return hits;
}

test('the bring-up budget is defined exactly once, in the crate that does the work', () => {
  assert.deepEqual(
    rustHits('pub const BRINGUP_OPERATION'),
    ['crates/nai-storage/src/lib.rs × 1'],
    'bring-up work is performed by Store::open in nai-storage, so the definition lives there and nowhere else',
  );
  assert.match(
    rustTexts.get('crates/nai-storage/src/lib.rs'),
    /pub const BRINGUP_OPERATION: Duration = Operation::CEILING;/,
    'the bring-up budget MUST reference the ceiling constant rather than repeat the number',
  );
});

test('no Rust source hides a second copy of the operation ceiling', () => {
  assert.deepEqual(
    rustHits('Operation::new(Duration::from_secs(30))'),
    [],
    'a caller that wants the ceiling MUST reference Operation::CEILING instead of the literal',
  );
  assert.deepEqual(
    rustHits('const OPERATION_CEILING'),
    [],
    'a private shadow of Operation::CEILING is exactly the "second, drifting copy" that must not exist',
  );
});

// ── W78 · §51.3 能力包清单的打包粒度 ─────────────────────────────────────
// 对账的前三块都只到**集合**层：§57.3 的「能力包」行、registry 的 `capability_pack` 行、门控常量，
// 各记一份集合。可 §51.3 的八行说的是**每一个编号属于哪一个包**——那是**划分**。
// 划分对集合层的断言是瞎的：`M05–M08` 同时属于两个包时，并集一模一样（ADR 0056 决策 2）。

/// §51.3 表后「保留在 R1 的基础纪律」列出的编号（V3.11 正文）。
/// 写在这里，是为了把「解析为空」与「解析结果变了」分开——两者都该失败，但原因完全不同。
const R1_RETAINED = ['H01', 'H02', 'H03', 'H04', 'H15', 'H17', 'H18'];

/// 一张最小的 §51.3 存根：用于单测**解析器本身**（不依赖仓库里那份正文）。
const packStub = (rows, after) =>
  ['### 51.3 能力包清单（本轮）', '', '| 能力包 | 验收 | 依据 |', '| --- | --- | --- |', ...rows, '', after, ''].join('\n');

test('§51.3 parses to a partition of the capability-pack line, with a disjoint R1 half', async () => {
  const registry = await readRegistry();
  const packs = parseBaseline(baseline.text).capabilityPacks;
  assert.equal(packs.found, true, '§51.3 heading must be found');
  assert.deepEqual(packs.errors, []);
  assert.equal(packs.rows.length, 8, '§51.3 declares eight capability packs');
  assert.equal(packs.union.size, 29, 'the eight rows cover twenty-nine ids');
  // 划分：没有编号同时属于两个包。
  assert.deepEqual(packs.overlaps, [], 'the pack split is a partition, not a union');
  // 三向一致：§51.3 的并集 == 在册的能力包线 == 门控常量。
  const onPackLine = [
    ...new Set(
      (registry.acceptance ?? [])
        .filter(item => item.release_line === 'capability_pack')
        .map(item => item.id),
    ),
  ].sort();
  assert.deepEqual([...packs.union].sort(), onPackLine);
  assert.deepEqual(
    [...new Set(CAPABILITY_PACK_IDS)].sort(),
    [...packs.union].sort(),
    'the gate constant mirrors §51.3, so it must equal the parsed union',
  );
  // 分界判据的另一侧：非空、且与能力包不相交。
  assert.deepEqual(
    [...packs.retained].sort(),
    R1_RETAINED,
    "§51.3's other half is the R1-retained discipline",
  );
  assert.deepEqual(
    [...packs.retained].filter(id => packs.union.has(id)),
    [],
    'an id cannot be both a capability pack and R1-retained discipline',
  );
});

test('a pack row that shrinks, or an id in two packs, is reported by name', async () => {
  const registry = await readRegistry();
  // 行数不变、内容被改小 —— 只钉行数的断言在这里是瞎的（W66 教训的第二次适用）。
  const shrunk = mutated(
    '| 扩展与技能（含声明式技能） | M01–M12 |',
    '| 扩展与技能（含声明式技能） | M01–M02 |',
  );
  const shrunkErrors = against(registry, shrunk);
  assert.ok(
    shrunkErrors.some(
      error =>
        error.includes('§51.3 row 扩展与技能（含声明式技能） declares') &&
        error.includes('M01, M02, expected') &&
        error.includes('M03'),
    ),
    'a row that quietly drops ids must name the row and both sides',
  );
  assert.ok(
    shrunkErrors.some(error => error.includes("§51.3's pack union disagrees with tests/registry.yaml")),
    'the set-level check must fire as well, not only the row-level one',
  );

  // 并集看起来仍然完整，而某个编号已经属于两个包。
  const overlapped = mutated(
    '| harness 黄金集 | H16 |',
    '| harness 黄金集 | H16 |\n| 影子包 | H09、H10 | 注入用 |',
  );
  assert.ok(
    against(registry, overlapped).some(
      error =>
        error.includes('puts H09 in both 多步工具循环 and 影子包') &&
        error.includes('partition, not a union'),
    ),
    'an id claimed by two packs must be reported with both pack labels',
  );
});

test('the boundary between packs and R1-retained discipline is asserted from both sides', async () => {
  const registry = await readRegistry();
  const parsed = parseBaseline(baseline.text);
  // 把一个编号从「保留在 R1 的基础纪律」挪进包行：两侧同时越界。
  const movedH18 = mutated('| harness 黄金集 | H16 |', '| harness 黄金集 | H16、H18 |');
  assert.ok(
    against(registry, movedH18).some(
      error => error.includes('lists H18 as both a capability pack and R1-retained discipline'),
    ),
    'the boundary criterion must be checked from both sides, not only from the pack side',
  );

  // 解析器本身：R1 那一侧为空集时，「互不相交」会恒真 ⇒ MUST 具名失败而不是静默通过。
  const emptyHalf = parseCapabilityPacks(
    packStub(['| harness 黄金集 | H16 | 后置 |'], '**保留在 R1 的基础纪律**：没有列出编号。'),
  );
  assert.equal(emptyHalf.retained.size, 0);
  assert.ok(
    against(registry, {...parsed, capabilityPacks: emptyHalf}).some(error =>
      error.includes('lists no R1-retained discipline ids'),
    ),
    'a criterion that holds because nothing was parsed is worse than no criterion',
  );

  // 解析器本身：章节号被改 ⇒ 解析失败，MUST NOT 因为并集为空而通过。
  const renamed = parseCapabilityPacks(
    '### 51.9 改名了\n\n| 能力包 | 验收 | 依据 |\n| --- | --- | --- |\n| a | H16 | b |\n',
  );
  assert.equal(renamed.found, false);
  assert.ok(renamed.errors.some(error => error.includes('§51.3 heading not found')));

  // 解析器本身：依据为空 ⇒ 具名失败（「为什么可以后置」无人复核）。
  // 这条判据**逐行判、与行数无关**，故存根只有一行也必须被抓住。
  const noBasis = parseCapabilityPacks(
    packStub(['| harness 黄金集 | H16 |  |'], '**保留在 R1 的基础纪律**（H01、H02）。'),
  );
  assert.ok(
    against(registry, {...parsed, capabilityPacks: noBasis}).some(error =>
      error.includes('§51.3 row harness 黄金集 has no basis'),
    ),
    'a pack whose reason is unwritten must be named even when the table has the wrong shape',
  );
});

// ── W79 · 节界：按章节号取表的搜索域 MUST 封闭在本节内 ─────────────────────
//
// `sectionTable` 是三个解析器（§48.5／§51.3／§57.3）共同的地基。朴素写法「向后找第一张表」
// 在**表不在本节内**时（被删、被挪、或被一个更浅的标题挡开）取到的是**别人**的表，而且返回值
// 形状与成功时一模一样。实测：一段「§57.3 标题在、本节内无表、后面跟着 §57.9 的一张表」的文本
// 会让 `parseReleaseLines` 返回 `found = true`、`errors = []`，把 §57.9 的行当成 §57.3 自己的行。
// ⇒ W78 那句「§51.2 刻意未读（它是散文、无表）」因此**只靠文档与记性**维持，不是一条判据。

/// 三张被解析的表：节号 → 生产代码里用的那个章节谓词。
const PARSED_TABLES = [
  ['48.5', /^#+\s*48\.5\b/],
  ['51.3', /^#+\s*51\.3\b/],
  ['57.3', /^#+\s*57\.3\b/],
];

test('a section is searched inside its own bounds, never in a neighbour’s table', () => {
  // ① 真实正文：三张表都落在**本节边界内**，且谓词命中的确实是标题（`level` 非 0）。
  // 边界若被收得过紧（例如把标题行本身当成节界），下面三条会**同时**报「本节内无表」
  // ——这就是这条判据的非空洞性。
  const lines = baseline.text.split('\n');
  for (const [number, predicate] of PARSED_TABLES) {
    const table = sectionTable(lines, line => predicate.test(line));
    assert.equal(table.found, true, `§${number} heading must be found`);
    assert.equal(table.hasTable, true, `§${number} must hold its table inside its own bounds`);
    assert.equal(table.level, 3, `§${number} is a level-3 heading in V3.11`);
    assert.ok(
      table.tableStart > table.start && table.tableStart < table.end,
      `§${number}'s table must lie strictly inside [${table.start}, ${table.end})`,
    );
  }

  // ② 合成：本节无表、下一节有表 ⇒ MUST 具名报「本节内无表」，MUST NOT 借用下一节的表。
  const borrow = [
    '### 57.3 发布线汇总（收缩后，正本）',
    '',
    '本节刻意没有表格（模拟表被删、或被一个更浅的标题挡开）。',
    '',
    '### 57.9 另一节',
    '',
    '| 线 | 验收 |',
    '| --- | --- |',
    '| Slice-0 | S01–S04 |',
    '',
    '计数：`slice_0` 4 = **4**',
    '',
  ].join('\n');
  const borrowed = parseReleaseLines(borrow);
  assert.equal(borrowed.found, false, 'a section without a table of its own must not read as parsed');
  assert.equal(borrowed.lines.size, 0, "a neighbour's table MUST NOT be adopted");
  assert.deepEqual(borrowed.errors, ['§57.3 has no table inside its section']);
});

test('a deeper heading inside a section does not cut it short', () => {
  // 边界按**层级**定：`####` 不截断本节。写成「遇到任何标题就停」时第一条立刻红
  // ——本节里明明有表，却报「本节内无表」，那是**假阳性**方向。
  const deeper = [
    '### 57.3 发布线汇总（收缩后，正本）',
    '',
    '#### 57.3.1 细节',
    '',
    '| 线 | 验收 |',
    '| --- | --- |',
    '| Slice-0 | S01–S04 |',
    '',
    '计数：`slice_0` 4 = **4**',
    '',
  ].join('\n');
  const parsed = parseReleaseLines(deeper);
  assert.equal(parsed.found, true, 'a deeper heading must not end the section');
  assert.deepEqual([...parsed.lines.get('slice_0')], ['S01', 'S02', 'S03', 'S04']);
  assert.deepEqual(parsed.errors, [], 'the count line after the table must still be read');

  // 而**同级**的标题确实截断：同一张表落到 `###` 之后就取不到了。
  const sibling = parseReleaseLines(deeper.replace('#### 57.3.1 细节', '### 57.9 另一节'));
  assert.equal(sibling.found, false);
  assert.deepEqual(sibling.errors, ['§57.3 has no table inside its section']);
});

test('a prose section has no table of its own, and that premise is asserted', () => {
  // W78 的规格**刻意不读 §51.2**，理由是「它是散文、没有表格」。本轮把这条理由从**记性**
  // 变成**会被检查的前提**：给 §51.2 加一张表，这条断言就红——那正是应该回去重读
  // ADR 0056 遗留 2 的时刻。（§57.5 选它作第二个样本，是因为它的下一节 `## 58` 恰好带着两张表，
  // 正是「邻节有表」这种最容易借错的形状。）
  const lines = baseline.text.split('\n');
  const prose = [
    ['51.2', sectionTable(lines, line => /^#+\s*51\.2\b/.test(line))],
    ['57.5', sectionTable(lines, line => /^#+\s*57\.5\b/.test(line))],
  ];
  for (const [number, table] of prose) {
    assert.equal(table.found, true, `§${number} heading must exist`);
    assert.equal(
      table.hasTable,
      false,
      `§${number} is prose: a table added there must fail this, and the neighbour's table must not be adopted`,
    );
  }
  // 前提的另一半：§51.3 的表**确实**被读到了，否则上一条可能只是「什么都没解析出来」。
  assert.equal(parseCapabilityPacks(baseline.text).rows.length, 8);
});

/* ----------------------------------------------------------------------------------------------
 * W84：测试文件的可达性闭世界
 *
 * 门控靠「逐条列举 + 两个 glob」把 Node 测试接上电，而**列举的盲区是它的补集**：
 * 一个放进 `tests/kernel/` 的测试文件（已在 `.git/index` 里、含 `assert.fail`）此前**静默不跑**
 * ——门控 `EXIT=0`、目标数与用例数不变、其用例名在日志里零出现。
 *
 * 下面的注入用例**不改真实文件**：它们喂一份**合成的**在册集与 host 正文给判据，
 * 因此每一组的「我改的确实是我以为的那一处」是构造出来的，而不是靠内存里的写法。
 * 唯一的真实仓库用例在最后（F0），它用 `readTrackedPaths` 读 `.git/index`。
 * -------------------------------------------------------------------------------------------- */

/// 合成世界：两个站点、两份 host 正文、三个在册测试文件。
const fakeSites = () => [
  { id: 'alpha', host: 'gate/a.txt', marker: 'tests/one.test.mjs', base: '.', tier: 'portable' },
  { id: 'beta', host: 'gate/b.json', marker: 'tests/group/*.test.mjs', base: '.', tier: 'full' },
];
const fakeHosts = () => new Map([
  ['gate/a.txt', 'run --test tests/one.test.mjs\n'],
  ['gate/b.json', '"test": "node --test tests/group/*.test.mjs"\n'],
]);
const fakeTracked = () => new Set([
  'tests/one.test.mjs',
  'tests/group/two.test.mjs',
  'tests/group/three.test.mjs',
]);
const readFrom = hosts => host => (hosts.has(host) ? hosts.get(host) : null);

test('the reachability judge is green on a well-formed world, and names nothing', () => {
  // F0（合成）：两个站点都逐字落在 host 里、都覆盖到东西 ⇒ 无错误。
  const errors = reachabilityErrors({
    tracked: fakeTracked(),
    sites: fakeSites(),
    unreachable: [],
    readHost: readFrom(fakeHosts()),
  });
  assert.deepEqual(errors, [], 'a well-formed registration world must produce no errors');
});

test('a test file nobody registers is reported by name', () => {
  // F1：这正是本轮要修的洞——在册却不被任何站点覆盖的文件此前**完全静默**。
  const tracked = fakeTracked();
  tracked.add('tests/lonely.test.mjs');
  const errors = reachabilityErrors({
    tracked,
    sites: fakeSites(),
    unreachable: [],
    readHost: readFrom(fakeHosts()),
  });
  assert.ok(
    errors.some(error => error.includes('tests/lonely.test.mjs') && error.includes('no registration site covers it')),
    `a tracked test file with no site must be named; got ${JSON.stringify(errors)}`,
  );
});

test('a site whose marker is absent from its host is a declared lie', () => {
  // F2：改一个字符 —— 站点还在表里，但 host 里已经没有那段原文。
  const hosts = fakeHosts();
  hosts.set('gate/a.txt', 'run --test tests/renamed.test.mjs\n');
  const errors = reachabilityErrors({
    tracked: fakeTracked(),
    sites: fakeSites(),
    unreachable: [],
    readHost: readFrom(hosts),
  });
  assert.ok(
    errors.some(error => error.includes('alpha') && error.includes('does not contain it verbatim')),
    `a site whose marker is missing from its host must be named; got ${JSON.stringify(errors)}`,
  );
});

test('a narrowed glob drops exactly the files it stopped matching', () => {
  // F3：把一个 glob 缩成单文件 —— 被丢下的**那些**文件必须落进闭世界。
  const sites = fakeSites();
  sites[1].marker = 'tests/group/two.test.mjs';
  const hosts = fakeHosts();
  hosts.set('gate/b.json', '"test": "node --test tests/group/two.test.mjs"\n');
  const errors = reachabilityErrors({
    tracked: fakeTracked(),
    sites,
    unreachable: [],
    readHost: readFrom(hosts),
  });
  assert.ok(
    errors.some(error => error.includes('tests/group/three.test.mjs')),
    `the file dropped by the narrowed glob must be named; got ${JSON.stringify(errors)}`,
  );
  assert.ok(
    !errors.some(error => error.includes('tests/group/two.test.mjs')),
    'the file the narrow glob still matches must not be reported',
  );
});

test('a site that covers nothing is a hollow declaration', () => {
  // F4：marker 逐字存在，但解析后在在册集合上匹配不到任何东西。
  const sites = fakeSites();
  sites[0].marker = 'tests/absent.test.mjs';
  const hosts = fakeHosts();
  hosts.set('gate/a.txt', 'run --test tests/absent.test.mjs\n');
  const errors = reachabilityErrors({
    tracked: fakeTracked(),
    sites,
    unreachable: [],
    readHost: readFrom(hosts),
  });
  assert.ok(
    errors.some(error => error.includes('alpha') && error.includes('matches no tracked file')),
    `a site that covers nothing must be named; got ${JSON.stringify(errors)}`,
  );
});

test('a stale exemption in the unreachable ledger is reported', () => {
  // F5：台账里的条目**已经可达** ⇒ 过期豁免。台账不是垃圾桶。
  const errors = reachabilityErrors({
    tracked: fakeTracked(),
    sites: fakeSites(),
    unreachable: [{ path: 'tests/one.test.mjs', reason: '说是没人跑' }],
    readHost: readFrom(fakeHosts()),
  });
  assert.ok(
    errors.some(error => error.includes('tests/one.test.mjs') && error.includes('stale exemption')),
    `an already-covered ledger entry must be named; got ${JSON.stringify(errors)}`,
  );
});

test('a genuine unreachable entry is accepted, and the ledger must carry a reason', () => {
  // F7（反向注入）：一条**真**未被覆盖的条目 + 非空理由 ⇒ **不红**。
  // 这是「假阳性方向」的证据：本轮拒绝的是**过期/无理由**的条目，不是**所有**台账条目。
  //
  // 注意「未覆盖」必须由**站点侧**造成（缩窄 glob），MUST NOT 把文件移出在册集合——
  // 台账条目要描述**仓库内容**，把文件移出去会让条目本身变成非法的（实测过这个坑）。
  const sites = fakeSites();
  sites[1].marker = 'tests/group/two.test.mjs';
  const hosts = fakeHosts();
  hosts.set('gate/b.json', '"test": "node --test tests/group/two.test.mjs"\n');
  const tracked = fakeTracked(); // `three` 仍在册，只是没有任何站点跑它

  const accepted = reachabilityErrors({
    tracked,
    sites,
    unreachable: [{ path: 'tests/group/three.test.mjs', reason: '仅由手工命令跑，尚未接入门控' }],
    readHost: readFrom(hosts),
  });
  assert.deepEqual(accepted, [], 'a genuine, reasoned ledger entry must not fail the judge');
  // 而同一条目**没有理由**时必须红——否则台账会变成「加进去就再也不用管」。
  const reasonless = reachabilityErrors({
    tracked,
    sites,
    unreachable: [{ path: 'tests/group/three.test.mjs', reason: '  ' }],
    readHost: readFrom(hosts),
  });
  assert.ok(
    reasonless.some(error => error.includes('non-empty reason')),
    `a ledger entry without a reason must be named; got ${JSON.stringify(reasonless)}`,
  );
});

test('a test extension outside the recognized list is reported', () => {
  // F6：文件层的闭世界对 `foo.test.ts` **看不见**（它连「未被覆盖」都不会被报出），
  // 因为 `isTestFile` 为假。⇒ 扩展名层必须自己有一条断言。
  const tracked = fakeTracked();
  tracked.add('tests/x/typed.test.ts');
  const errors = reachabilityErrors({
    tracked,
    sites: fakeSites(),
    unreachable: [],
    readHost: readFrom(fakeHosts()),
  });
  assert.ok(
    errors.some(error => error.includes('tests/x/typed.test.ts') && error.includes('blind spot')),
    `an unscanned test extension must be named; got ${JSON.stringify(errors)}`,
  );
  // 前提的另一半：这个文件确实**不**是「被识别的测试文件」，否则上一条可能只是碰巧命中。
  assert.equal(isTestFile('tests/x/typed.test.ts'), false);
});

test('an empty coverage set is reported, so "nothing covers anything" cannot pass', () => {
  // F9：站点表清空 ⇒ 若没有这条断言，文件层闭世界会因台账为空而**全部静默通过**。
  const errors = reachabilityErrors({
    tracked: fakeTracked(),
    sites: [],
    unreachable: [],
    readHost: readFrom(fakeHosts()),
  });
  assert.ok(
    errors.some(error => error.includes('no registration site covers any tracked file')),
    `an empty coverage set must be reported; got ${JSON.stringify(errors)}`,
  );
});

test('site order does not matter, and a missing input is an error rather than a silence', () => {
  // F8（反向注入）：站点表顺序打乱 ⇒ 不红（判据只看集合，不看顺序）。
  const reordered = [...fakeSites()].reverse();
  assert.deepEqual(
    reachabilityErrors({
      tracked: fakeTracked(),
      sites: reordered,
      unreachable: [],
      readHost: readFrom(fakeHosts()),
    }),
    [],
    'site order must not change the verdict',
  );
  // 入参缺失 ⇒ **具名报错**，MUST NOT 静默降级（与 `validatePendingDecisions` 同纪律）。
  const missing = reachabilityErrors({ sites: fakeSites() });
  assert.ok(
    missing.some(error => error.includes('tracked-path set') || error.includes('readHost')),
    `missing inputs must be named, not silently skipped; got ${JSON.stringify(missing)}`,
  );
});

test('the site table is pinned verbatim and the ledger is declared empty', () => {
  // 表级：逐字钉住三张常量表。派生写法（例如把扩展名由「仓库里现有文件」算出）会让
  // 「清单 == 它自己」恒真，一个字都没说。
  assert.deepEqual(TEST_EXTENSIONS, ['.test.mjs']);
  assert.deepEqual(TEST_TIERS, ['portable', 'full', 'ci']);
  assert.deepEqual(LOCALLY_UNREACHABLE, [], 'the unreachable ledger is declared empty by this round');
  assert.deepEqual(siteShapeErrors(), [], 'the shipped site table must be self-consistent');
  // 八个站点，逐字写死 id：新增/删除站点会让这条红，而不是静静地改变覆盖集。
  // （W95 新增 `xtask-engineering-workflow`——它正是这条断言的应有行为：新增一个测试文件
  // 而不登记站点，会让 W84 的闭世界与这条逐字清单**同时**红。）
  assert.deepEqual(
    TEST_REGISTRATION_SITES.map(site => site.id).sort(),
    [
      'ci-contract-suite',
      'desktop-test',
      'root-test-ui',
      'xtask-contracts-suite',
      'xtask-engineering-platform',
      'xtask-engineering-schema',
      'xtask-engineering-workflow',
      'xtask-registry',
    ],
  );
  for (const site of TEST_REGISTRATION_SITES) {
    assert.ok(TEST_TIERS.includes(site.tier), `site ${site.id} must have a tier in the closed set`);
    // **非空洞**：每个站点在**合成世界**里都能解析成一个 matcher（形状可用），
    // 真实覆盖与否由 F0 与注入用例分别证明。
    assert.equal(typeof markerMatcher(site).matches, 'function');
  }
});

test('the site table rejects duplicate ids, unknown tiers, and repeated host+marker pairs', () => {
  const duplicated = [...fakeSites(), { ...fakeSites()[0] }];
  assert.ok(
    siteShapeErrors(duplicated).some(error => error.includes('declared twice')),
    'a duplicated id must be reported',
  );
  const badTier = fakeSites();
  badTier[0].tier = 'nightly';
  assert.ok(
    siteShapeErrors(badTier).some(error => error.includes('which is not one of')),
    'a tier outside the closed set must be reported',
  );
  const repeated = [...fakeSites(), { ...fakeSites()[0], id: 'gamma' }];
  assert.ok(
    siteShapeErrors(repeated).some(error => error.includes('duplicates the host+marker')),
    'two sites with the same host+marker are redundant and must be reported',
  );
});

test('the real repository reaches every tracked test file', async () => {
  // F0：真实仓库——用 `.git/index` 的已跟踪路径集（W75 的纯 JS 设施，不派生子进程）。
  const tracked = await readTrackedPaths('.');
  assert.equal(tracked.ok, true, tracked.detail ?? 'the tracked-path set must be readable');
  const errors = reachabilityErrors({
    tracked: tracked.paths,
    sites: TEST_REGISTRATION_SITES,
    unreachable: LOCALLY_UNREACHABLE,
    readHost: host => {
      try {
        return readFileSync(host, 'utf8');
      } catch {
        return null;
      }
    },
  });
  assert.deepEqual(errors, [], 'the reachability closed world must hold for the real repository');

  // 前提的另一半：覆盖集**确实非空**，否则上面的空错误可能只是「什么都没解析出来」。
  const covered = new Set();
  for (const site of TEST_REGISTRATION_SITES) {
    for (const filePath of resolveSite(site, tracked.paths)) covered.add(filePath);
  }
  assert.ok(covered.size >= 9, `expected at least nine covered test files, got ${covered.size}`);
});

