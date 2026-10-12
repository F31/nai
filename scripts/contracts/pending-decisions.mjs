/// 待裁定项台账的校验：`tests/pending-decisions.yaml`。
///
/// 这个模块存在的理由：基线有四章「待决策与待确认」（§18.2 / §39.7 / §48.7 / §57.5，一条累积链），
/// 它们此前只有**载体**、没有**台账**。「待裁定」因此读起来像「无人负责」——没有状态、没有依据、
/// 没有闭合条件，于是**已经由基线正文回答的条目会一直悬着**，而外部条件可以永远既不算完成也不算失败。
///
/// 与 `baseline.mjs` 的 `normalizeDeviations` 同一条纪律：**入参显式注入，缺任一项即报错**。
/// 一条会因为入参缺失而恒真的断言，比没有断言更坏。

/// 五个处置取值。用封闭枚举而不是自由文本，理由与 `release_lines`／`status` 一致：
/// 枚举让「这条属于哪一类」可判定。
export const DISPOSITIONS = [
  'answered_by_baseline',
  'decidable_in_repo',
  'needs_precondition',
  'needs_external',
  'out_of_scope',
];

/// 处置 → **必填字段**。这是本轮的核心设计：把「这条有没有人管」换成一个可判定的问题
/// ——「那个字段在不在」。一条 `needs_external` 若没有 `closure`，与一条空注释没有区别。
export const REQUIRED_FIELD = new Map([
  ['answered_by_baseline', 'basis'],
  ['decidable_in_repo', 'recommendation'],
  ['needs_precondition', 'precondition'],
  ['needs_external', 'closure'],
  ['out_of_scope', 'note'],
]);

/// 基线四章的**新增**条数（累积链去重后，每条只记在首次出现的那一章）。
/// 这是照抄基线事实的一张表，因此**必须另有** `DISPOSITION_COUNTS` 把分布也写死——
/// 只校验总数对「内容被改小／被改类别」是瞎的（W66 教训）。
export const CHAPTER_COUNTS = new Map([
  ['§18.2', 10],
  ['§39.7', 11],
  ['§48.7', 10],
  ['§57.5', 3],
]);
export const TOTAL_ENTRIES = 34;

/// 处置分布，逐项写死。改了任何一条的处置类别都会被抓住——否则「把 answered 全改成 external」
/// 会让台账看起来仍然完整。
export const DISPOSITION_COUNTS = new Map([
  ['answered_by_baseline', 4],
  ['decidable_in_repo', 5],
  ['needs_precondition', 4],
  ['needs_external', 20],
  ['out_of_scope', 1],
]);

/// 抽取文本里出现的全部 `§NNN` 引用（`NN` 或 `NN.M`）。
export function sectionRefs(text) {
  return [...String(text ?? '').matchAll(/§(\d+(?:\.\d+)?)/g)].map(match => match[1]);
}

/// 从基线文本抽出全部章节号。标题形如 `## 58 …` 与 `### 58.6 …`。
export function baselineSections(baselineText) {
  const out = new Set();
  for (const match of String(baselineText ?? '').matchAll(/^#{2,3}[\s\u3000]+(\d+(?:\.\d+)?)/gm)) {
    out.add(match[1]);
  }
  return out;
}

/// 从基线文本抽出全部工作项编号（表格里的 `| W61 |` 形状）。
export function baselineWorkItems(baselineText) {
  const out = new Set();
  for (const match of String(baselineText ?? '').matchAll(/\|\s*(W\d{2})\s*\|/g)) {
    out.add(match[1]);
  }
  return out;
}

const nonEmptyText = value => typeof value === 'string' && value.trim().length > 0;

/// 校验台账。`deps` 显式注入：
/// `{baselineVersion, baselineText, acceptanceIds, workItemIds}`。
///
/// **缺任一项即报错**，不做「缺了就跳过对应检查」的降级——那会让判据在被削弱时保持沉默。
export function validatePendingDecisions(ledger, deps = {}) {
  const errors = [];
  const required = ['baselineVersion', 'baselineText', 'acceptanceIds', 'workItemIds'];
  const missing = required.filter(key => deps[key] === undefined);
  if (missing.length > 0) {
    // 具名报出缺什么，而不是让后面的检查「因为没有输入所以看起来通过」。
    errors.push(`validation inputs are required and were not provided: ${missing.join(', ')}`);
    return errors;
  }

  if (!ledger || typeof ledger !== 'object') {
    return ['pending-decisions ledger must be a mapping'];
  }
  if (ledger.schema_version !== 1) errors.push('schema_version must be 1');

  // 台账必须声明它是对着哪个基线写的。升基线后若忘了更新，这里会红——这正是想要的：
  // 一份对着旧基线写的台账，其「§NNN 存在」的结论对新基线不成立。
  if (ledger.baseline !== deps.baselineVersion) {
    errors.push(
      `ledger declares baseline ${JSON.stringify(ledger.baseline)} but the current baseline is ${deps.baselineVersion}`,
    );
  }

  const sections = baselineSections(deps.baselineText);
  if (sections.size === 0) {
    errors.push('no baseline sections were found in the baseline text — the reference check would be vacuous');
  }

  const entries = ledger.entries;
  if (!Array.isArray(entries) || entries.length === 0) {
    errors.push('entries must be a non-empty list');
    return errors;
  }

  const seen = new Set();
  const byChapter = new Map([...CHAPTER_COUNTS.keys()].map(key => [key, 0]));
  const byDisposition = new Map(DISPOSITIONS.map(key => [key, 0]));

  for (const entry of entries) {
    const id = entry?.id;
    const where = typeof id === 'string' && id.length > 0 ? id : '<entry without id>';
    if (!/^PD-\d{2}$/.test(String(id ?? ''))) {
      errors.push(`invalid entry id: ${JSON.stringify(id)}`);
      continue;
    }
    if (seen.has(id)) errors.push(`duplicate entry id: ${id}`);
    seen.add(id);

    if (!nonEmptyText(entry.question)) errors.push(`${id}: question must be non-empty`);

    // introduced_in 决定这一条记在哪一章，故它必须是 CHAPTER_COUNTS 的键之一。
    if (!CHAPTER_COUNTS.has(entry.introduced_in)) {
      errors.push(`${id}: introduced_in must be one of ${[...CHAPTER_COUNTS.keys()].join(', ')}`);
    } else {
      byChapter.set(entry.introduced_in, byChapter.get(entry.introduced_in) + 1);
    }

    if (!DISPOSITIONS.includes(entry.disposition)) {
      errors.push(`${id}: invalid disposition ${JSON.stringify(entry.disposition)}`);
    } else {
      byDisposition.set(entry.disposition, byDisposition.get(entry.disposition) + 1);
    }

    if (!Array.isArray(entry.also_in ?? [])) {
      errors.push(`${id}: also_in must be a list when present`);
    }

    // 每态一个必填字段。
    const field = REQUIRED_FIELD.get(entry.disposition);
    if (field) {
      const value = entry[field];
      const ok =
        field === 'precondition'
          ? Array.isArray(value) && value.length > 0
          : nonEmptyText(value);
      if (!ok) {
        errors.push(
          `${id}: disposition ${entry.disposition} requires a non-empty ${field}`,
        );
      }
    }

    // 台账里出现的**每一个** §NNN 都必须真实存在于当前基线。这条把「凭印象编一个章节号」排除掉；
    // 语义上是否真的回答了该问题由 ADR 与报告承载，不由脚本假装判定。
    const refs = [
      ...sectionRefs(entry.basis),
      ...sectionRefs(entry.introduced_in),
      ...(Array.isArray(entry.also_in) ? entry.also_in.flatMap(sectionRefs) : []),
    ];
    for (const ref of refs) {
      if (sections.size > 0 && !sections.has(ref)) {
        errors.push(`${id}: references §${ref}, which does not exist in the baseline`);
      }
    }

    // answered_by_baseline 必须**指回**至少一个章节——否则「有依据」是空的。
    if (entry.disposition === 'answered_by_baseline' && sectionRefs(entry.basis).length === 0) {
      errors.push(`${id}: answered_by_baseline must cite at least one §NNN it is answered by`);
    }

    // 前置必须落在真实存在的东西上。
    if (entry.disposition === 'needs_precondition' && Array.isArray(entry.precondition)) {
      for (const item of entry.precondition) {
        if (!nonEmptyText(item)) {
          errors.push(`${id}: precondition entries must be non-empty strings`);
        } else if (/^W\d{2}$/.test(item)) {
          if (!deps.workItemIds.has(item)) {
            errors.push(`${id}: precondition ${item} is not a work item named by the baseline`);
          }
        } else if (/^[SRABOPMHX]\d{2}$/.test(item)) {
          // 形状是验收 id、但注册表里没有它——与「写的根本不是 id」是两种不同的错，分开报。
          if (!deps.acceptanceIds.has(item)) {
            errors.push(`${id}: precondition ${item} is not a registry acceptance id`);
          }
        } else {
          errors.push(
            `${id}: precondition ${item} is neither a registry acceptance id nor a baseline work item`,
          );
        }
      }
    }
  }

  // 覆盖性：逐章写死，MUST NOT 由「台账自己说自己全」保证。
  for (const [chapter, expected] of CHAPTER_COUNTS) {
    const actual = byChapter.get(chapter) ?? 0;
    if (actual !== expected) {
      errors.push(`chapter ${chapter} has ${actual} entries, the baseline introduces ${expected}`);
    }
  }
  if (entries.length !== TOTAL_ENTRIES) {
    errors.push(`ledger holds ${entries.length} entries, the baseline chain resolves to ${TOTAL_ENTRIES}`);
  }
  // 处置分布同样逐项写死：总数相等但有两条互换了类别，上面的检查是看不见的。
  for (const [disposition, expected] of DISPOSITION_COUNTS) {
    const actual = byDisposition.get(disposition) ?? 0;
    if (actual !== expected) {
      errors.push(
        `disposition ${disposition} covers ${actual} entries, the reviewed set has ${expected}`,
      );
    }
  }

  return errors;
}
