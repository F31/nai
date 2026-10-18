/// **归档记录的结构判据**（W149，ADR 0123）。
///
/// 问题形状：`openspec validate --archived` 对**归档过**的变更只回答「能不能解析」，
/// 不回答「它有没有把话说完」。实测（145 份归档 proposal）：
///
/// - `2026-10-09-engine-assembly` 用 `## What` 而不是 `## What Changes`；
/// - `2026-10-10-w31-golden-set-payload` 全中文标题（`为什么` / `变更` / `影响` / `否决方案`）。
///
/// 两份在 `validate --archived` 里都是 ✓（**145 / 0**）⇒ 现有门控对这条结构性缺口**零信号**。
/// 而 `archive.js` 侧的 `validateChange()` 是 **non-blocking**（ADR 0117）：新归档缺 Why 只会打一行
/// 警告，之后任何门控都不会再提。本判据补的就是这一段——**归档之后仍然有人问**。
///
/// 三条决定（理由见 ADR 0123）：
///
/// 1. **判全部归档记录，不按时间划界**。「2026-10-18 之后归档的才算」是一条按日期开的豁免：
///    它不具名、不自清理、且允许被回溯性地绕过。例外一律进台账（具名 + `since_baseline` + 自清理）。
/// 2. **标题归一化只到大小写与 `Changes` 可选**。`## What` 与 `## 为什么` **不**被接受：
///    把它们并进 `REQUIRED_SECTIONS` 等于用「宽容」替代「一致」，而归档记录的读者是未来的自己。
/// 3. **空段等于缺段，但措辞不同**。「标题在」不等于「这件事付过账」——一个空 `## Why` 比没有更糟，
///    它看起来像答过了。
///
/// 判据形状是**纯函数 + 可注入**：域（哪些归档变更、它们的 proposal 文本）由调用方给，
/// 判据只做判定。真实语料侧由测试从 `.git/index` 派生（W75），因此「未跟踪的归档目录」不在域内。

/// 归档根目录（MUST 带尾随斜杠，`startsWith` 用它）。
export const ARCHIVE_ROOT = 'openspec/changes/archive/';

/// 每份归档 proposal MUST 有的段。
///
/// `pattern` 是**标题归一化**的唯一出处：大小写不敏感 + `Changes` 的 `s` 可有可无。
/// MUST NOT 在这里加 `^what$` / `^为什么$` —— 那两处正是台账存在的理由。
export const REQUIRED_SECTIONS = [
  { id: 'why', label: 'Why', pattern: /^why$/i },
  { id: 'what', label: 'What Changes', pattern: /^what\s+changes?$/i },
];

/// **例外台账**：具名、相称、自清理（MEMORY §7）。
///
/// 每条 `{change, missing, sinceBaseline, reason}`：
/// - `change`：归档目录名；
/// - `missing`：**这份文档实际缺哪几段**（`REQUIRED_SECTIONS` 的 id 集合）——台账 MUST 描述它盖住的
///   那个缺口。写成 `[]` 或用一句「整份不合规」都在判据里具名失败：**描述不准的例外等于没盖住**；
/// - `sinceBaseline`：进入台账时的基线版本。判据校验它非空，长度不由本判据管；
/// - `reason`：为什么**不修**（修 = 改已发布的历史记录）。
///
/// 台账**不是豁免**：文档补齐之后这条 MUST 被删掉，否则判据会红（自清理）。
export const PROPOSAL_EXCEPTIONS = [
  {
    change: '2026-10-09-engine-assembly',
    missing: ['what'],
    sinceBaseline: 'V3.11',
    reason:
      '归档于本判据之前，正文标题写作 `## What`（Why 在，235 字符）；改标题等于改已发布的历史记录，故不修',
  },
  {
    change: '2026-10-10-w31-golden-set-payload',
    missing: ['why', 'what'],
    sinceBaseline: 'V3.11',
    reason:
      '归档于本判据之前，全文中文标题（`为什么` / `变更` / `影响` / `否决方案`）；补英文标题等于改历史记录，故不修',
  },
];

/// 从**已跟踪路径**里取归档变更名（目录层）。归档目录里**任意**已跟踪文件都算，不只是 proposal——
/// 只认 `proposal.md` 会让「某次归档整个漏掉 proposal」在域外（那正是本判据要报的东西）。
export function archivedChangeNames(trackedPaths) {
  const names = new Set();
  for (const path of trackedPaths) {
    if (typeof path !== 'string' || !path.startsWith(ARCHIVE_ROOT)) continue;
    const rest = path.slice(ARCHIVE_ROOT.length);
    const slash = rest.indexOf('/');
    if (slash <= 0) continue;
    names.add(rest.slice(0, slash));
  }
  return [...names].sort();
}

/// 某个归档变更的 proposal 路径（与 `archivedChangeNames` 的口径一致）。
export function proposalPathOf(change) {
  return `${ARCHIVE_ROOT}${change}/proposal.md`;
}

/// 把一份 proposal 切成 `id → {title, body}`。
///
/// 标题只认行首 `## `；`body` 取到下一个 `## ` 之前并 `trim`。同一段出现两次时取**第一处**
/// ——重复标题本身不在本判据的域内（那是另一条判据的事，重复登记只会让这里的结论变模糊）。
export function sectionsOf(text) {
  const lines = String(text).split(/\r?\n/);
  const heads = [];
  lines.forEach((line, index) => {
    const match = /^##[ \t]+(.+?)[ \t]*$/.exec(line);
    if (match) heads.push({ title: match[1], index });
  });
  const sections = new Map();
  for (const section of REQUIRED_SECTIONS) {
    const head = heads.find(item => section.pattern.test(item.title));
    if (!head) continue;
    const next = heads.find(item => item.index > head.index);
    const body = lines.slice(head.index + 1, next ? next.index : lines.length).join('\n').trim();
    sections.set(section.id, { title: head.title, body });
  }
  return sections;
}

const labelsOf = ids =>
  ids.map(id => REQUIRED_SECTIONS.find(section => section.id === id).label).join(' + ');

/// 判定入口。
///
/// `changes`：`Map<归档变更名, proposal 文本 | null>`（`null` = 该归档目录没有 `proposal.md`）。
/// 返回 `{errors, changesChecked, exceptionsUsed}`；`exceptionsUsed` 由判据自己记账，
/// 调用方 MUST NOT 另行统计（「谁构造入参」只能是这一处）。
export function archivedProposalErrors({ changes, exceptions = PROPOSAL_EXCEPTIONS } = {}) {
  if (!changes || typeof changes.size !== 'number') {
    return {
      errors: [
        'no archive was provided, so "every archived proposal states why it exists" would hold over an empty set',
      ],
      changesChecked: 0,
      exceptionsUsed: 0,
    };
  }
  if (changes.size === 0) {
    return {
      errors: [
        'the archive holds no change at all, so "every archived proposal states why it exists" would hold over an empty set',
      ],
      changesChecked: 0,
      exceptionsUsed: 0,
    };
  }

  const errors = [];
  const used = new Set();
  const names = [...changes.keys()].sort();

  for (const name of names) {
    const text = changes.get(name);
    if (text === null || text === undefined) {
      errors.push(
        `${name}: the archive directory carries no proposal.md — what this change was for is then only recoverable from git history, and a record that has to be reconstructed is not a record`,
      );
      continue;
    }

    const sections = sectionsOf(text);
    const missing = [];
    const messages = [];
    for (const section of REQUIRED_SECTIONS) {
      const found = sections.get(section.id);
      if (!found) {
        missing.push(section.id);
        messages.push(
          `${name}: the proposal has no "## ${section.label}" section — the archive is the only place this change will ever be read from, so a section nobody wrote cannot be recovered later`,
        );
        continue;
      }
      if (found.body === '') {
        missing.push(section.id);
        messages.push(
          `${name}: the "## ${found.title}" section is empty — a heading is not a statement; keeping the heading and dropping the sentence is what makes a gap look answered`,
        );
      }
    }

    const exception = exceptions.find(entry => entry.change === name);
    if (!exception) {
      errors.push(...messages);
      continue;
    }
    used.add(name);

    if (missing.length === 0) {
      errors.push(
        `${name}: the proposal now carries every required section (${labelsOf(REQUIRED_SECTIONS.map(s => s.id))}), so the exception is stale — an exception that outlives its reason is an exemption, and this one is no longer paying for anything`,
      );
      continue;
    }
    const declared = [...exception.missing].sort();
    const actual = [...missing].sort();
    if (declared.join(',') !== actual.join(',')) {
      // 描述不符时**只报这一条**，并同时给出两边：再叠上 `messages` 会让「台账写错了」和
      // 「文档缺段了」看起来是同一件事，而这里要判的恰恰是前者。
      errors.push(
        `${name}: the exception declares [${declared.join(', ')}] missing, but the proposal is actually missing [${actual.join(', ')}] — an exception MUST describe the gap it covers; one that names a different gap is not covering this document, it is hiding it`,
      );
    }
  }

  for (const exception of exceptions) {
    if (used.has(exception.change)) continue;
    errors.push(
      `the exception for "${exception.change}" matches no archived change — an exception with no subject keeps the ledger from being read, and the next reader has no way to tell a live exception from a forgotten one`,
    );
  }

  return { errors, changesChecked: names.length, exceptionsUsed: used.size };
}
