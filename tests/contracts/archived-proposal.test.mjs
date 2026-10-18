import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { readTrackedPaths, describeFailure } from '../../scripts/contracts/git-index.mjs';
import {
  ARCHIVE_ROOT,
  PROPOSAL_EXCEPTIONS,
  REQUIRED_SECTIONS,
  archivedChangeNames,
  archivedProposalErrors,
  proposalPathOf,
  sectionsOf,
} from '../../scripts/contracts/archived-proposal.mjs';

/// W149：**归档记录的结构判据**（ADR 0123）。
///
/// 判据守的是一个**已经发生**的缺口：145 份归档 proposal 里 2 份缺段，而
/// `openspec validate --archived` 对它们都是 ✓（**145 / 0**）⇒ 现有门控零信号。
/// 而 `archive.js` 的 `validateChange()` 是 non-blocking（ADR 0117）：新归档缺 Why 只会
/// 打一行警告，之后没有任何门控会再提。
///
/// 本文件分三层，**各自有独立输入域**：
/// 1. 真实语料（域来自 `.git/index`）——只证明「今天为绿 + 台账与语料对得上」；
/// 2. 合成样本——每条缺陷各有一个「只有它错时才红」的样本；
/// 3. 域本身（`archivedChangeNames`）——归档目录里**任意**已跟踪文件都让该变更进入域，
///    所以「某次归档整个漏掉 proposal.md」也在域内。

const DOC_OK = [
  '## Why',
  '',
  '因为归档记录是唯一还会被读的地方。',
  '',
  '## What Changes',
  '',
  '- 加一条判据。',
  '',
  '## Impact',
  '',
  '- 无。',
  '',
].join('\n');

const withSections = (why, what) =>
  ['## Why', '', why, '', '## What Changes', '', what, ''].join('\n');

/// 合成样本默认**不带台账**：台账只在专判它的用例里出现，否则「例外指向不存在的变更」
/// 会把每一条合成用例都多带两条噪声，让「这条缺陷红了」不再可读。
const errorsOf = (changes, exceptions = []) =>
  archivedProposalErrors({ changes, exceptions }).errors;

/// 真实语料：域从 `.git/index` 派生，`null` 表示该归档目录没有 `proposal.md`。
async function loadArchive() {
  const index = await readTrackedPaths('.');
  assert.ok(index.ok, describeFailure(index));
  const names = archivedChangeNames(index.paths);
  const tracked = new Set(index.paths);
  const changes = new Map();
  for (const name of names) {
    const path = proposalPathOf(name);
    changes.set(name, tracked.has(path) ? await readFile(path, 'utf8') : null);
  }
  return { names, changes };
}

test('every archived proposal still states why it exists, and the ledger matches the corpus', async () => {
  const { names, changes } = await loadArchive();
  const result = archivedProposalErrors({ changes });

  assert.deepEqual(result.errors, [], 'the shipped archive must be clean or the ledger must name the gap');

  // 非空洞四件：域非空、判据走遍了域、台账每条都真的指到了一份归档变更、且规则不是只被台账满足。
  assert.ok(names.length > 0, 'the archive is not empty');
  assert.equal(changes.size, names.length, 'every archived change must be judged, not a sample of them');
  assert.equal(result.changesChecked, names.length, 'the judge must account for each change it walked over');
  assert.equal(result.exceptionsUsed, PROPOSAL_EXCEPTIONS.length, 'every exception must be in use');
  assert.ok(
    names.length > PROPOSAL_EXCEPTIONS.length,
    'the rule must hold for changes beyond the ones the ledger covers, otherwise the ledger is the rule',
  );

  // 台账不许凭空指路：每个具名变更 MUST 真的是一份归档目录（否则「例外」在语料里没有主体）。
  for (const exception of PROPOSAL_EXCEPTIONS) {
    assert.ok(names.includes(exception.change), `${exception.change} must exist in the archive`);
    for (const id of exception.missing) {
      assert.ok(
        REQUIRED_SECTIONS.some(section => section.id === id),
        `${exception.change}: "${id}" is not one of the required sections, so the ledger names a gap this judge cannot see`,
      );
    }
  }
});

test('a missing or empty section is named, and the ledger cannot cover what it does not describe', () => {
  // 基线：合规文档不红（否则下面每一条红都可能是别的原因）。
  assert.deepEqual(errorsOf(new Map([['ok', DOC_OK]])), []);

  // 缺段与空段是两种缺陷，措辞不同：「标题在」不等于「这件事付过账」。
  const noWhy = errorsOf(new Map([['a', '## What Changes\n\n- 有内容。\n']]));
  assert.equal(noWhy.length, 1, JSON.stringify(noWhy));
  assert.match(noWhy[0], /^a: the proposal has no "## Why" section/);

  const emptyWhy = errorsOf(new Map([['b', withSections('', '- 有内容。')]]));
  assert.equal(emptyWhy.length, 1, JSON.stringify(emptyWhy));
  assert.match(emptyWhy[0], /is empty — a heading is not a statement/);

  // 归一化的边界**两侧**都要钉：`What changes` 接受，`What` 与中文标题不接受。
  assert.deepEqual(
    errorsOf(new Map([['c', '## Why\n\n- 有。\n\n## What changes\n\n- 有内容。\n']])),
    [],
    'case alone must not turn a compliant proposal red',
  );
  const whatOnly = errorsOf(new Map([['d', '## Why\n\n- 有。\n\n## What\n\n- 有内容。\n']]));
  assert.equal(whatOnly.length, 1, JSON.stringify(whatOnly));
  assert.match(whatOnly[0], /no "## What Changes" section/);

  const chinese = errorsOf(new Map([['e', '## 为什么\n\n- 有。\n\n## 变更\n\n- 有内容。\n']]));
  assert.equal(chinese.length, 2, JSON.stringify(chinese));

  // 归档目录没有 proposal.md：这是文件层，台账不覆盖它（台账只管「缺哪一段」）。
  const noFile = errorsOf(new Map([['f', null]]));
  assert.equal(noFile.length, 1, JSON.stringify(noFile));
  assert.match(noFile[0], /carries no proposal\.md/);

  // 台账相称：声明的缺口必须与实测缺口一致，否则「盖住的」不是这份文档。
  // 每个用例只带**它自己那一条**台账：带上全部条目会让「例外无主体」混进来，红因就不可读了。
  const ledgerFor = change => PROPOSAL_EXCEPTIONS.filter(entry => entry.change === change);

  const covered = errorsOf(
    new Map([['2026-10-09-engine-assembly', '## Why\n\n- 有。\n']]),
    ledgerFor('2026-10-09-engine-assembly'),
  );
  assert.deepEqual(covered, [], 'a ledger entry that describes the gap it covers must not be red');

  const mismatched = errorsOf(
    new Map([['2026-10-09-engine-assembly', '## Why\n\n- 有。\n\n## What Changes\n\n- 有内容。\n']]),
    ledgerFor('2026-10-09-engine-assembly'),
  );
  assert.equal(mismatched.length, 1, JSON.stringify(mismatched));
  assert.match(mismatched[0], /so the exception is stale/);
  assert.match(mismatched[0], /an exception that outlives its reason is an exemption/);

  const wrongGap = errorsOf(
    new Map([['2026-10-10-w31-golden-set-payload', '## Why\n\n- 有。\n']]),
    ledgerFor('2026-10-10-w31-golden-set-payload'),
  );
  assert.equal(wrongGap.length, 1, JSON.stringify(wrongGap));
  // 两边都按 id 排序输出（`what` < `why`），所以断言里用的是排序后的顺序，不是台账的书写顺序。
  assert.match(wrongGap[0], /declares \[what, why\] missing, but the proposal is actually missing \[what\]/);

  // 台账无主体：例外指向一个不在域内的变更。
  const orphan = errorsOf(new Map([['ok', DOC_OK]]), PROPOSAL_EXCEPTIONS);
  assert.equal(orphan.length, PROPOSAL_EXCEPTIONS.length, JSON.stringify(orphan));
  for (const error of orphan) assert.match(error, /matches no archived change/);

  // 空集不是「全绿」：判据 MUST NOT 把「没东西可判」写成「都合规」。
  for (const empty of [new Map(), undefined]) {
    const errors = errorsOf(empty);
    assert.equal(errors.length, 1, JSON.stringify(errors));
    assert.match(errors[0], /over an empty set/);
  }
});

test('the archive domain is every tracked file under it, so a change with no proposal is still in scope', () => {
  const names = archivedChangeNames([
    `${ARCHIVE_ROOT}b/tasks.md`,
    `${ARCHIVE_ROOT}a/proposal.md`,
    `${ARCHIVE_ROOT}a/specs/x/spec.md`,
    `${ARCHIVE_ROOT}no-slash-at-all`,
    'openspec/changes/2026-10-18-still-active/proposal.md',
    'docs/reports/x.md',
    undefined,
  ]);
  assert.deepEqual(
    names,
    ['a', 'b'],
    'the domain must come from any tracked file under archive/ (not only proposal.md), must be sorted, '
      + 'and must not reach into the active changes or the rest of the repository',
  );

  // 「目录在、proposal 不在」必须**在域内**并被判红，否则本判据只能看见已经完整的归档。
  const decisions = archivedProposalErrors({ changes: new Map([['b', null]]) });
  assert.equal(decisions.changesChecked, 1);
  assert.match(decisions.errors[0], /carries no proposal\.md/);

  // 段解析本身：同名的第二处不算数（取第一处），标题后的空白不影响识别。
  const sections = sectionsOf('## Why  \n\n- 一。\n\n## Why\n\n- 二。\n\n## What Changes\n\n- 有。\n');
  assert.equal(sections.get('why').body, '- 一。');
  assert.ok(sections.has('what'));
});
