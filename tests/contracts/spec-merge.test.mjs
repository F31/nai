import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { readTrackedPaths, describeFailure } from '../../scripts/contracts/git-index.mjs';
import { ARCHIVE_ROOT } from '../../scripts/contracts/archived-proposal.mjs';
import {
  DELTA_MARKS,
  SPECS_ROOT,
  deltaPathOf,
  deltaRefsOf,
  deltaSections,
  mainRequirements,
  mainSpecPathOf,
  mainSpecRefsOf,
  specMergeErrors,
} from '../../scripts/contracts/spec-merge.mjs';

/// W150：**归档增量与主规格的对账**（ADR 0124）。
///
/// 判据守的是一段**没人看管**的距离：`openspec archive` 只把 delta 挪进归档目录，不保证
/// 已写进主规格。实测（W150）手工改掉主规格里一条 `### Requirement:` 名后，两层 validate
/// 仍 **43 / 0** 与 **146 / 0** ⇒ 零信号。
///
/// 本文件分三层，**各自有独立输入域**：
/// 1. 真实语料（域来自 `.git/index`）——只证明「今天为绿」，且两个方向各自真的走了；
/// 2. 合成样本——每条缺陷各有一个「只有它错时才红」的样本（集合重放 / 两方向 / 段标记 /
///    空 delta / 整体缺席 / 空集）；
/// 3. 域本身（`deltaRefsOf` / `mainSpecRefsOf`）——重放顺序只由归档目录名定。

const delta = (change, capability, text) => ({ change, capability, text });

const errorsOf = (deltas, specs) => specMergeErrors({ deltas, specs }).errors;

/// 一份合规主规格。`names` 是 `## Requirements` 下的 Requirement 名。
const mainSpec = (...names) =>
  [
    '# cap Specification',
    '',
    '## Purpose',
    '',
    '它为什么存在。',
    '',
    '## Requirements',
    '',
    ...names.flatMap(name => [`### Requirement: ${name}`, '', '- 正文。', '']),
    '',
  ].join('\n');

/// 一份合规 delta。`mark` ∈ `DELTA_MARKS`。
const deltaDoc = (mark, ...names) =>
  [
    `# Spec delta: ${names.join(' + ')}`,
    '',
    `## ${mark} Requirements`,
    '',
    ...names.flatMap(name => [
      `### Requirement: ${name}`,
      '',
      '- 正文。',
      '',
      '#### Scenario: 场景',
      '',
      '- **WHEN** 有人读它',
      '- **THEN** 它说得清',
      '',
    ]),
    '',
  ].join('\n');

/// 真实语料：delta 与主规格都从 `.git/index` 派生（未跟踪的归档目录不在域内）。
async function loadCorpus() {
  const index = await readTrackedPaths('.');
  assert.ok(index.ok, describeFailure(index));
  const refs = deltaRefsOf(index.paths);
  const deltas = [];
  for (const ref of refs) deltas.push({ ...ref, text: await readFile(ref.path, 'utf8') });
  const specs = new Map();
  for (const [capability, path] of mainSpecRefsOf(index.paths)) {
    specs.set(capability, await readFile(path, 'utf8'));
  }
  return { refs, deltas, specs };
}

test('every archived spec delta is merged into the main spec, in both directions', async () => {
  const { refs, deltas, specs } = await loadCorpus();
  const result = specMergeErrors({ deltas, specs });

  assert.deepEqual(result.errors, [], 'the shipped archive must be merged in, or the gap must be named');

  // 非空洞五件：域非空、走遍了域、每个主规格都真的被比对到、重放出的总量非零、
  // 且「重放出的总量」与「主规格里实际有多少条」这两个**独立算出**的数字相等。
  assert.ok(refs.length > 0, 'the archive is not empty');
  assert.equal(result.deltasChecked, refs.length, 'every archived delta must be replayed, not a sample');
  assert.equal(
    result.changesChecked,
    new Set(refs.map(ref => ref.change)).size,
    'every archived change that carries a delta must be accounted for',
  );
  assert.equal(
    result.capabilitiesCompared,
    specs.size,
    'every main spec must be compared — a capability that only exists in the archive would shrink this number',
  );
  let inMain = 0;
  for (const text of specs.values()) inMain += mainRequirements(text).requirements.length;
  assert.ok(inMain > 0, 'the main specs carry requirements');
  assert.equal(
    result.requirementsReplayed,
    inMain,
    'the replayed set and the main specs must have the same size — one number comes from the deltas, the other from the specs, so equality is not something the judge can produce by itself',
  );

  // 重放顺序 = 归档目录名字典序。日期前缀让字典序等于时间序；这条断言钉住「顺序来自目录名」，
  // 而不是「调用方顺手排的」。
  const order = [...new Set(refs.map(ref => ref.change))];
  assert.deepEqual(order, [...order].sort(), 'the replay order must be the archive directory order');
});

test('the replay is ordered, so a requirement added and later removed is not expected in the main spec', () => {
  // 基线：合规文档不红（否则下面每一条红都可能是别的原因）。
  const ok = errorsOf(
    [delta('2026-01-01-a', 'cap', deltaDoc('ADDED', 'Kept'))],
    new Map([['cap', mainSpec('Kept')]]),
  );
  assert.deepEqual(ok, []);

  // W1：集合并集会把 `Gone` 留在期望集里 ⇒ 一条「加了又删」的历史会被判成「归档了没合入」。
  const addedThenRemoved = errorsOf(
    [
      delta('2026-01-01-a', 'cap', deltaDoc('ADDED', 'Gone')),
      delta('2026-01-02-b', 'cap', deltaDoc('REMOVED', 'Gone')),
    ],
    new Map([['cap', mainSpec()]]),
  );
  assert.deepEqual(
    addedThenRemoved,
    [],
    'a set union over the deltas would keep "Gone" alive and red a removal that was applied correctly',
  );

  // `MODIFIED` 的先存性**不判**：实测有 2 条真实的「先改后加」（w02-w03 admission-hardening、
  // engine-broker frame-delivery 改的是各自 delta 之前就存在的 Requirement）。
  const modifiedFirst = errorsOf(
    [delta('2026-01-01-a', 'cap', deltaDoc('MODIFIED', 'Older'))],
    new Map([['cap', mainSpec('Older')]]),
  );
  assert.deepEqual(
    modifiedFirst,
    [],
    'a MODIFIED requirement that predates its own delta is legal history, not a gap',
  );

  assert.deepEqual(DELTA_MARKS, ['ADDED', 'MODIFIED', 'REMOVED'], 'the mark closed set is the replay vocabulary');
});

test('archived but never merged is named, and so is merged with no archive behind it', () => {
  // 方向一：delta 有、主规格没有。这是本判据存在的理由。
  const missing = errorsOf(
    [delta('2026-01-01-a', 'cap', deltaDoc('ADDED', 'Wanted'))],
    new Map([['cap', mainSpec()]]),
  );
  assert.equal(missing.length, 1, JSON.stringify(missing));
  assert.match(
    missing[0],
    /^cap: the archived delta 2026-01-01-a adds "Wanted", but the main spec does not carry it/,
  );

  // `MODIFIED` 掉的那条也走同一方向，但动词要说对：它没被合入时，缺的是「改」，不是「加」。
  const modifiedMissing = errorsOf(
    [delta('2026-01-01-a', 'cap', deltaDoc('MODIFIED', 'Touched'))],
    new Map([['cap', mainSpec()]]),
  );
  assert.equal(modifiedMissing.length, 1, JSON.stringify(modifiedMissing));
  assert.match(modifiedMissing[0], /the archived delta 2026-01-01-a modifies "Touched"/);

  // 方向二：主规格有、任何 delta 都没写过它。
  const stray = errorsOf(
    [delta('2026-01-01-a', 'cap', deltaDoc('ADDED', 'Wanted'))],
    new Map([['cap', mainSpec('Wanted', 'Stray')]]),
  );
  assert.equal(stray.length, 1, JSON.stringify(stray));
  assert.match(stray[0], /carries "Stray", but no archived delta ever introduced it/);

  // `REMOVED` 要有自己的措辞：它**有**归档出处，只是那份出处说「删掉」。用「从没经过归档」
  // 报它，读者会以为文档写错了，而实际错的是「删了却没删」。
  const stale = errorsOf(
    [
      delta('2026-01-01-a', 'cap', deltaDoc('ADDED', 'Gone')),
      delta('2026-01-02-b', 'cap', deltaDoc('REMOVED', 'Gone')),
    ],
    new Map([['cap', mainSpec('Gone')]]),
  );
  assert.equal(stale.length, 1, JSON.stringify(stale));
  assert.match(stale[0], /the archived delta 2026-01-02-b removed "Gone", but the main spec still carries it/);
  assert.doesNotMatch(stale[0], /no archived delta ever introduced it/, 'a removal has provenance, it is just a removal');

  // 能力整体缺席只报**一条**：逐条报会把「这份主规格根本不存在」稀释成 N 个数字。
  // 两个方向在同一个样本里各报一条（错误按能力名排序，`cap` 在 `gone` 前）。
  const absent = errorsOf(
    [delta('2026-01-01-a', 'gone', deltaDoc('ADDED', 'One', 'Two', 'Three'))],
    new Map([['cap', mainSpec()]]),
  );
  assert.equal(absent.length, 2, JSON.stringify(absent));
  assert.match(absent[0], /^cap: a main spec exists at openspec\/specs\/cap\/spec\.md, but no archived delta ever wrote it/);
  assert.match(
    absent[1],
    /^gone: 3 requirement\(s\) were assembled from 1 archived delta\(s\), but there is no main spec at openspec\/specs\/gone\/spec\.md/,
  );
});

test('a section mark or a requirement the judge cannot place is named, never skipped', () => {
  // 段标记闭世界：`## CHANGED Requirements` 不是判据认识的动作 ⇒ 具名失败，
  // 而不是「找不到就跳过」（跳过等于把下面那条 Requirement 藏起来）。
  const unknownMark = errorsOf(
    [delta('2026-01-01-a', 'cap', deltaDoc('CHANGED', 'X'))],
    new Map([['cap', mainSpec('X')]]),
  );
  assert.equal(unknownMark.length, 3, JSON.stringify(unknownMark));
  assert.match(unknownMark[0], /"## CHANGED Requirements" is not one of the section marks this judge knows/);
  assert.match(unknownMark[1], /"X" sits outside any ADDED\/MODIFIED\/REMOVED section/);
  assert.match(unknownMark[2], /carries "X", but no archived delta ever introduced it/);

  // 落在第一个 `##` 之前的 Requirement 没有归属 ⇒ 没有任何动作能作用在它身上。
  const orphan = errorsOf(
    [delta('2026-01-01-a', 'cap', '### Requirement: Loose\n\n## ADDED Requirements\n\n### Requirement: Kept\n')],
    new Map([['cap', mainSpec('Kept')]]),
  );
  assert.equal(orphan.length, 1, JSON.stringify(orphan));
  assert.match(orphan[0], /"Loose" sits outside any ADDED\/MODIFIED\/REMOVED section/);

  // `## Purpose` 是允许的（实测 53 处），不能被闭世界误伤。
  assert.deepEqual(deltaSections('## Purpose\n\n为什么。\n\n## ADDED Requirements\n\n### Requirement: A\n').problems, []);

  // 空 delta：一份什么都没说的记录，与「没合入」在读者眼里完全一样。
  const emptyDelta = errorsOf(
    [delta('2026-01-01-a', 'cap', '## ADDED Requirements\n\n')],
    new Map([['cap', mainSpec()]]),
  );
  assert.equal(emptyDelta.length, 1, JSON.stringify(emptyDelta));
  assert.match(emptyDelta[0], /the archived delta declares no requirement at all/);

  // 主规格侧：没有 `## Requirements`、以及 Requirement 落在别的段里。
  const noSection = errorsOf(
    [delta('2026-01-01-a', 'cap', deltaDoc('ADDED', 'X'))],
    new Map([['cap', '# cap Specification\n\n## Purpose\n\n说明。\n']]),
  );
  assert.equal(noSection.length, 2, JSON.stringify(noSection));
  assert.match(noSection[0], /the main spec has no "## Requirements" section/);

  // 落在别的段里的 Requirement 只报**一条**：它既不在 `## Requirements` 里（不参与比对），
  // 也就不该再被「没有归档出处」报第二次——同一条缺口报两遍会把计数变得没有意义。
  const outsideSection = errorsOf(
    [delta('2026-01-01-a', 'cap', deltaDoc('ADDED', 'In'))],
    new Map([['cap', '# cap Specification\n\n## Requirements\n\n### Requirement: In\n\n## Notes\n\n### Requirement: Out\n']]),
  );
  assert.equal(outsideSection.length, 1, JSON.stringify(outsideSection));
  assert.match(outsideSection[0], /"Out" sits outside "## Requirements" in the main spec/);

  // 归档目录在、spec.md 不在：无法重放的记录看起来和已合入的一模一样。
  const noFile = errorsOf([delta('2026-01-01-a', 'cap', null)], new Map([['cap', mainSpec()]]));
  assert.equal(noFile.length, 1, JSON.stringify(noFile));
  assert.match(noFile[0], /the archived delta file is missing/);

  // 空集不是「全绿」：判据 MUST NOT 把「没东西可判」写成「都合入了」。
  for (const empty of [{}, { deltas: [] }, { deltas: [delta('a', 'cap', deltaDoc('ADDED', 'X'))], specs: new Map() }]) {
    const errors = specMergeErrors(empty).errors;
    assert.equal(errors.length, 1, JSON.stringify(errors));
    assert.match(errors[0], /over an empty set/);
  }
});

test('the replay order and the two domains come from the archive layout, not from the caller', () => {
  const paths = [
    `${ARCHIVE_ROOT}b/specs/cap2/spec.md`,
    `${ARCHIVE_ROOT}a/specs/cap1/spec.md`,
    `${ARCHIVE_ROOT}a/specs/cap0/spec.md`,
    `${ARCHIVE_ROOT}a/proposal.md`,
    `${ARCHIVE_ROOT}a/tasks.md`,
    `${ARCHIVE_ROOT}a/specs/cap1/notes.md`,
    'openspec/changes/2026-10-18-still-active/specs/cap/spec.md',
    `${SPECS_ROOT}cap1/spec.md`,
    `${SPECS_ROOT}weird/nested/spec.md`,
    `${SPECS_ROOT}notes.md`,
    undefined,
  ];

  assert.deepEqual(
    deltaRefsOf(paths).map(ref => `${ref.change}/${ref.capability}`),
    ['a/cap0', 'a/cap1', 'b/cap2'],
    'only <change>/specs/<capability>/spec.md is a delta; the domain must be sorted by (change, capability) '
      + 'and must not reach into the active changes or the rest of the specs tree',
  );
  assert.equal(deltaRefsOf(paths)[1].path, deltaPathOf('a', 'cap1'), 'the path helper and the domain agree');

  assert.deepEqual(
    [...mainSpecRefsOf(paths)],
    [['cap1', mainSpecPathOf('cap1')]],
    'a main spec is exactly specs/<capability>/spec.md — a nested path or a loose file is not one',
  );
});
