import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { readRegistry, readCatalogIds, validateRegistry } from '../scripts/contracts/registry.mjs';
import {
  readBaseline,
  parseBaseline,
  reconcile,
  pointerErrors,
  declaredBaselineName,
} from '../scripts/contracts/baseline.mjs';

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

test('registry.yaml is machine-readable and internally consistent', async () => {
  const registry = await readRegistry();
  const catalogIds = await readCatalogIds();
  const errors = validateRegistry(registry, catalogIds);
  assert.deepEqual(errors, []);
  // Cover V3.10 §16 + §30.4 + §48.5 + §51.3 + §57.3 acceptance families and
  // release lines.
  assert.ok(registry.acceptance.length >= 87, 'expected >=87 acceptance items');
  assert.ok(registry.mapping.d01_to_d14.D14, 'expected D14 mapping');
  const slice0 = registry.acceptance.filter(a => a.release_line === 'slice_0');
  assert.ok(['S01','S02','S03','S04','R01','R02','R03','R07','A01','A04'].every(id =>
    slice0.some(a => a.id === id)), 'Slice-0 exit acceptance missing');
  // Every family the baseline defines as a complete range must be registered in
  // full. The extension/marketplace family M (§37.5, §38.10) is not optional —
  // ADR 0042 decision 4 left it unregistered once and the omission was recorded
  // as a defect. The harness family H (V3.10 §40-§47, §48.5) starts from that
  // lesson rather than repeating it: "not started" reads as `pending`, never as
  // a missing row.
  const byId = new Map(registry.acceptance.map(a => [a.id, a]));
  for (const family of ['M', 'H']) {
    for (const n of Array.from({length: 18}, (_, i) => `${family}${String(i + 1).padStart(2, '0')}`)) {
      assert.ok(byId.has(n), `baseline acceptance ${n} is not registered`);
    }
  }
  // The V3.10 scope split has to be readable, not just asserted in prose: the
  // pack line holds exactly the §51.3 set, and none of those ids sits on a
  // first-release line (INV-33).
  const packLine = registry.acceptance.filter(a => a.release_line === 'capability_pack').map(a => a.id);
  assert.equal(packLine.length, 29, 'expected the V3.10 §51.3 capability-pack set');
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
      error => error.includes('outside the V3.10 §51.3 set') && error.includes('H17'),
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
  assert.deepEqual(reconcile(registry, parsed), []);

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
  assert.deepEqual(reconcile(registry, renamedTitle), []);

  const renumbered = mutated('### 57.3 发布线汇总（收缩后，正本）', '### 59.3 发布线汇总（收缩后，正本）');
  const renumberedErrors = reconcile(registry, renumbered);
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
  assert.deepEqual(reconcile(registry, renumberedFamily), ['§48.5 heading not found']);
});

test('§57.3 cannot disagree with itself between its table and its count line', async () => {
  const registry = await readRegistry();
  // 同一节里「表格」与「计数行」是两种写法，可以各自漂移——W71 收录 B07 时改的就是这两处。
  // 锚点带上 `计数：` 前缀：§58.5 会**逐字复述**同一条计数行（那本身就是一句散文形式的断言），
  // 不带前缀会命中两处、改错地方。
  const countOff = mutated('计数：`slice_0` 10 + `r1` **37**', '计数：`slice_0` 10 + `r1` **36**');
  assert.ok(
    reconcile(registry, countOff).some(
      error => error.includes('§57.3 is self-inconsistent') && error.includes('`r1`') && error.includes('36'),
    ),
    'a count that disagrees with its own row must be reported',
  );
  const totalOff = mutated('+ `experimental` 2 = **88**', '+ `experimental` 2 = **87**');
  assert.ok(
    reconcile(registry, totalOff).some(error => error.includes('total 87')),
    'a total that disagrees with the sum of its rows must be reported',
  );
  // 漏掉一条线、或凭空多出一条线，都不是「数字对得上」能盖住的。
  const omitted = mutated('+ `experimental` 2 = **88**', ' = **88**');
  assert.ok(
    reconcile(registry, omitted).some(error => error.includes('count line omits `experimental`')),
    'a release line missing from the count line must be reported',
  );
  const invented = mutated('+ `experimental` 2 = **88**', '+ `experimental` 2 + `r4` 0 = **88**');
  assert.ok(
    reconcile(registry, invented).some(error => error.includes('unknown release line: r4')),
    'a release line that does not exist must be reported',
  );
});

test('release-line placement drift is reported by id, from either side', async () => {
  const registry = await readRegistry();
  const parsed = parseBaseline(baseline.text);

  // 注册表侧：把 B07 从 r1 挪到 r2，基线（冻结）仍写 r1。
  const moved = structuredClone(registry);
  moved.acceptance.find(item => item.id === 'B07').release_line = 'r2';
  const movedErrors = reconcile(moved, parsed);
  assert.ok(
    movedErrors.some(
      error => error.includes('B07 is registered on r2') && error.includes('declares it on r1'),
    ),
    'a moved id must be reported with both lines',
  );

  // 注册表侧：新增一条基线未声明的 id —— 这正是 W68 新增 B07 的形状。
  const added = structuredClone(registry);
  added.acceptance.push({...added.acceptance[0], id: 'S13', release_line: 'r1'});
  const addedErrors = reconcile(added, parsed);
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
  const removedErrors = reconcile(removed, parsed);
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
    reconcile(registry, lagging).some(error => error.includes('B07 is registered on r1') && error.includes('absent from §57.3')),
    'the baseline lagging behind the registry must be reported for the id that drifted',
  );
});

test('§48.5 is the source of truth for family completeness, for every family it declares', async () => {
  const registry = await readRegistry();
  // 缩回一个家族 ⇒ 报「停在了在册项之前」；这正是 W68（B07 未收录）的形状。
  const shrunk = mutated('| B | B01–B07 | 第 16 章、53 |', '| B | B01–B06 | 第 16 章、53 |');
  const shrunkErrors = reconcile(registry, shrunk);
  assert.ok(
    shrunkErrors.some(error => error.includes('§48.5 family B stops short of registered ids: B07')),
    'a family range that excludes a registered id must be reported by id',
  );
  // 扩到不存在的 id ⇒ 报「声明了未在册的 id」。
  const widened = mutated('| B | B01–B07 | 第 16 章、53 |', '| B | B01–B08 | 第 16 章、53 |');
  assert.ok(
    reconcile(registry, widened).some(
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
  const withLedger = entries => {
    const next = structuredClone(registry);
    next.baseline_deviations = entries;
    return next;
  };
  // 没有差异却登记了一条 ⇒ 陈旧条目 MUST 失败，否则台账只会越积越长、再也不被清理。
  assert.ok(
    reconcile(withLedger([{id: 'S01', registered: 'W99', reason: '陈旧'}]), parsed).some(
      error => error.includes('baseline_deviations lists S01') && error.includes('clear the entry'),
    ),
    'a stale ledger entry must fail',
  );
  // 条目本身要具名：id 合法、登记轮次可读、理由非空。
  const malformed = reconcile(
    withLedger([{id: 'ZZ9', registered: 'round-1', reason: ''}]),
    parsed,
  );
  assert.ok(malformed.some(error => error.includes('invalid id')), 'a bad id must be reported');
  assert.ok(
    reconcile(withLedger([{id: 'S02', registered: 'round-1', reason: 'x'}]), parsed).some(
      error => error.includes('must name the registering round'),
    ),
    'the registering round must be named',
  );
  assert.ok(
    reconcile(withLedger([{id: 'S02', registered: 'W68', reason: '   '}]), parsed).some(
      error => error.includes('must carry a non-empty reason'),
    ),
    'the reason must be non-empty',
  );
  assert.ok(
    reconcile(withLedger({id: 'S01'}), parsed).some(error => error.includes('must be a list')),
    'the ledger must be a list',
  );
  assert.ok(
    reconcile(
      withLedger([{id: 'S02', registered: 'W68', reason: 'a'}, {id: 'S02', registered: 'W68', reason: 'b'}]),
      parsed,
    ).some(error => error.includes('declares S02 twice')),
    'a duplicate entry must be reported',
  );
  // 逃生通道确实可用：真实差异 + 相称条目 ⇒ 一致。否则实现轮次会被门控锁死
  // （基线被 ADR 批准后不可变，实现轮次无法就地改 §57.3）。
  const realDeviation = structuredClone(registry);
  realDeviation.acceptance.find(item => item.id === 'B07').release_line = 'r2';
  realDeviation.baseline_deviations = [
    {id: 'B07', registered: 'W68', reason: '场景回放需要更完整的讲品证据，归属待下一轮升基线收录'},
  ];
  assert.deepEqual(reconcile(realDeviation, parsed), []);
  // 但条目 MUST 与差异相称：差异是 B07、条目却写 S02 ⇒ 两条都要报。
  const mismatched = structuredClone(realDeviation);
  mismatched.baseline_deviations = [
    {id: 'S02', registered: 'W68', reason: '张冠李戴'},
  ];
  const mismatchedErrors = reconcile(mismatched, parsed);
  assert.ok(
    mismatchedErrors.some(error => error.includes('B07 is registered on r2')),
    'the real difference must still be reported when the ledger names something else',
  );
  assert.ok(
    mismatchedErrors.some(error => error.includes('baseline_deviations lists S02')),
    'and the mismatched entry must be reported as stale',
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
