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

/// **会承载 `§NNN` 引用、因而其引用必须可解析的全部字段。**
///
/// 这是扫描域的**单一来源**。W76 立这条断言时把域手写成了三个 `sectionRefs(...)` 调用，
/// 于是 `recommendation`／`closure`／`note` 被**整片漏扫**——实测 17 条引用无人核
/// （把 `PD-24.recommendation` 的 `§40.4` 改成 `§99.9`，校验返回空错误）。
/// ⇒ 域 MUST 由清单派生，MUST NOT 在循环里手写：手写的那一份会漂移。
///
/// `note` 也在其中：它不是 `out_of_scope` 的专属字段，实测 5 条里有 4 条挂在别的处置上（注解）。
/// 扫描按「字段是否存在」触发，不按处置触发——否则注解型的 `note` 会被漏掉。
///
/// `question`（W81 补入）同样是**复述字段**——它复述基线各章的提问。它此前只有「非空」检查、
/// **不扫引用**，而「复述处可以随便编」这条推理对它最直接适用。实测当前 0 条含 `§` ⇒ 纳入扫描
/// 不引入红；方向为安全侧（将来某条引用了不存在的章节会被抓，而不是放行）。
export const CITATION_FIELDS = [
  'basis',
  'introduced_in',
  'also_in',
  'recommendation',
  'closure',
  'note',
  'question',
];

/// 必填字段里**承载登记 id 而非 `§NNN`** 的那一个（它按 id 校验，不走引用扫描）。
/// 它与 `CITATION_FIELDS` 一起，必须**恰好覆盖** `REQUIRED_FIELD` 的全部取值。
export const ID_LIST_FIELDS = ['precondition'];

/// **结构字段**：由各自的专属检查判定、且**不可能**承载 `§NNN` 的字段——
/// `id` 由 `^PD-\d{2}$` 判、`disposition` 由 `DISPOSITIONS` 枚举判，两者的字符集里根本没有 `§`。
/// （`question` **不在**这里：它有专属的「非空」检查，但**同时**承载 `§NNN` ⇒ 归 `CITATION_FIELDS`。
/// 一个字段可以「既被扫、又有专属检查」——分类判的是「`§NNN` 由谁负责」。）
export const STRUCTURAL_FIELDS = ['id', 'disposition'];

/// 台账里**允许出现**的全部字段。这是**闭世界**断言的基准：出现在条目里、却不在此集合中的字段
/// ⇒ **具名失败**（`validatePendingDecisions`）。
///
/// **逐字写死，MUST NOT 由三类派生**：派生出的「三类别之并 == `LEDGER_FIELDS`」是**恒真**的，
/// 一个字都没说。它必须是**独立的一份**声明，`shapeErrors()` 的「并集 == 它」才是**内容断言**
/// （W80 §九结论 5：只钉「关系」不够，要**逐字钉内容**）。
///
/// 为什么需要闭世界：**「按清单扫」的守卫，其盲区恰好是清单的补集**——`CITATION_FIELDS` 挡得住
/// 「清单上的字段被编造」，挡不住**新增字段**。实测给某条加 `rationale: "见 §99.9"` ⇒ 校验返回
/// **空错误**（W81 探针）。⇒ 只要还有「清单之外的字段不受检」这个状态，W80 修掉的那个洞就还在。
export const LEDGER_FIELDS = [
  'id',
  'question',
  'introduced_in',
  'also_in',
  'disposition',
  'basis',
  'recommendation',
  'precondition',
  'closure',
  'note',
];

/// 校验器自身两张常量表的一致性。**这不是「自检的装饰」**——它挡的是**未来某次合法编辑**
/// （加一个处置）导致的**静默失去覆盖**：
/// - 若某处置进了 `DISPOSITIONS` 却漏在 `REQUIRED_FIELD` 里，`REQUIRED_FIELD.get` 返回 `undefined`、
///   `if (field)` 短路 ⇒ 该处置的条目**没有必填字段**，校验对它沉默；
/// - 若某处置的必填文本字段不在 `CITATION_FIELDS` 里，它的引用**永不被解析**。
///
/// 两种都是**危险方向的瞎**：断言还在，覆盖没了，且没有任何信号（W73 假契约、W66「删字段照样绿」同族）。
export function shapeErrors() {
  const errors = [];
  const dispositions = new Set(DISPOSITIONS);
  const requiredFor = new Set(REQUIRED_FIELD.keys());

  for (const disposition of dispositions) {
    if (!requiredFor.has(disposition)) {
      errors.push(
        `disposition ${disposition} has no required field — the ledger would accept it silently`,
      );
    }
  }
  for (const disposition of requiredFor) {
    if (!dispositions.has(disposition)) {
      errors.push(`a required field is defined for ${disposition}, which is not a disposition`);
    }
  }

  const cited = new Set(CITATION_FIELDS);
  const idList = new Set(ID_LIST_FIELDS);
  const structural = new Set(STRUCTURAL_FIELDS);

  // 三类 MUST **两两不相交**：一个字段不能「由两个检查同时负责」——否则「谁在管它」不再是单一回答。
  const categories = [
    ['citation', cited],
    ['id-list', idList],
    ['structural', structural],
  ];
  for (let i = 0; i < categories.length; i += 1) {
    for (let j = i + 1; j < categories.length; j += 1) {
      const [leftName, left] = categories[i];
      const [rightName, right] = categories[j];
      for (const field of left) {
        if (right.has(field)) {
          errors.push(`field ${field} is claimed by two check categories (${leftName}, ${rightName})`);
        }
      }
    }
  }

  // 每个必填字段都必须被**某**检查覆盖：要么扫 §NNN 引用、要么按 id 校验。
  // 这条正是 W80 想要的不变式——把 `CITATION_FIELDS` 换回旧的三个字段，`note` 就会在这里被点名。
  for (const [disposition, field] of REQUIRED_FIELD) {
    if (!cited.has(field) && !idList.has(field)) {
      errors.push(
        `required field ${field} for ${disposition} is covered by no check — it would be unchecked`,
      );
    }
  }

  // **闭世界**：三类别之并 MUST **恰等于** `LEDGER_FIELDS`（这份表是**独立写死**的，
  // 所以这不是恒真式）。两个方向都被点名：
  // ① 某字段进了 `LEDGER_FIELDS` 却没被任何一类认领 ⇒ 数据允许它出现、却无人负责它；
  // ② 某字段被分类了却不在 `LEDGER_FIELDS` 里 ⇒ 那份分类永远打不到东西（闭世界断言会先拒掉数据）。
  const allowed = new Set(LEDGER_FIELDS);
  const classified = new Set([...cited, ...idList, ...structural]);
  for (const field of allowed) {
    if (!classified.has(field)) {
      errors.push(
        `field ${field} is allowed by LEDGER_FIELDS but no check category claims it — it would be unchecked`,
      );
    }
  }
  for (const field of classified) {
    if (!allowed.has(field)) {
      errors.push(
        `field ${field} is classified but not listed in LEDGER_FIELDS — the closed-world check would reject it`,
      );
    }
  }
  return errors;
}

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

  // 校验器自身的常量表先自洽——否则下面的逐条检查可能在「危险方向」上已经瞎了而不自知。
  errors.push(...shapeErrors());

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
  const allowedFields = new Set(LEDGER_FIELDS);

  for (const entry of entries) {
    const id = entry?.id;
    const where = typeof id === 'string' && id.length > 0 ? id : '<entry without id>';
    if (!/^PD-\d{2}$/.test(String(id ?? ''))) {
      errors.push(`invalid entry id: ${JSON.stringify(id)}`);
      continue;
    }
    if (seen.has(id)) errors.push(`duplicate entry id: ${id}`);
    seen.add(id);

    // **闭世界**：条目里出现的每个键 MUST ∈ `LEDGER_FIELDS`。
    // 只看**字段名**、**不看值**——若写成「未知字段**且**其值含 `§NNN`」才报，那么一个含引用的字段
    // 会在将来某次被扫到时**无声通过**。判据的输入 MUST 等于被断言的对象（这里断言的是「字段集封闭」，
    // 不是「这条引用坏不坏」）。⇒ 值干净的未知字段同样必须失败。
    for (const field of Object.keys(entry)) {
      if (!allowedFields.has(field)) {
        errors.push(
          `${id}: unknown field ${field} — a field the checker does not know can carry §NNN unchecked`,
        );
      }
    }

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
    //
    // 扫描域取自 `CITATION_FIELDS`（**包括** `recommendation`／`closure`／`note`）——
    // W80 之前只用三个 `sectionRefs(...)` 手写，那三个字段整片漏扫、17 条引用无人核。
    // 错误措辞 MUST 是「条目 + 字段 + 章节号」三件齐：同一个坏章节号可能同时出现在多个字段里，
    // 只说条目无法定位（W79：错误的措辞会把定位引向错误的方向）。
    for (const field of CITATION_FIELDS) {
      const value = entry[field];
      const values = Array.isArray(value) ? value : typeof value === 'string' ? [value] : [];
      for (const text of values) {
        for (const ref of sectionRefs(text)) {
          if (sections.size > 0 && !sections.has(ref)) {
            errors.push(
              `${id}: ${field} references §${ref}, which does not exist in the baseline`,
            );
          }
        }
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
