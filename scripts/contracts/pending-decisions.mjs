/// 待裁定项台账的校验：`tests/pending-decisions.yaml`。
///
/// 这个模块存在的理由：基线有四章「待决策与待确认」（§18.2 / §39.7 / §48.7 / §57.5，一条累积链），
/// 它们此前只有**载体**、没有**台账**。「待裁定」因此读起来像「无人负责」——没有状态、没有依据、
/// 没有闭合条件，于是**已经由基线正文回答的条目会一直悬着**，而外部条件可以永远既不算完成也不算失败。
///
/// 与 `baseline.mjs` 的 `normalizeDeviations` 同一条纪律：**入参显式注入，缺任一项即报错**。
/// 一条会因为入参缺失而恒真的断言，比没有断言更坏。

/// 六个处置取值。用封闭枚举而不是自由文本，理由与 `release_lines`／`status` 一致：
/// 枚举让「这条属于哪一类」可判定。
///
/// **W88 新增第六态 `decided_in_repo`**：原先五态只表达「**何处**能闭合」（基线答了／可在仓库内决定／
/// 等前置／等外部／不在范围），表达不了「**这条已经闭合了、并且落在仓库的哪里**」。
/// PD-09（静态数据加密所用的系统密钥保护机制）正是这个形状：它的推荐早已由 W55 的
/// 平台矩阵 `secret-protection` 能力行与 `crates/nai-security` 落地，而台账里它**没有可结清的状态**
/// ——只能永远挂着一条「推荐」。⇒ 「决定已作出且已落地」是**第六类**，不是 `decidable_in_repo` 的一种注解。
/// 「已落地的对象」与「可在仓库内决定的对象」是两类**不同**的东西，把前者写成后者的注解，
/// 正是 W87 §九那条「一个载体承载两件事」的同族缺陷。
///
/// 它**不是** `answered_by_baseline` 的近亲：`answered_by_baseline` 说「基线正文回答了它」，
/// 而 PD-09 恰恰相反——§11.5 明写「（具体机制待选型验证）」，基线**没有**回答它，是仓库回答了它。
/// 把 PD-09 写成 `answered_by_baseline` 会是一条**不实声明**（这正是本轮核实阶段排除掉的方案）。
export const DISPOSITIONS = [
  'answered_by_baseline',
  'decidable_in_repo',
  'decided_in_repo',
  'needs_precondition',
  'needs_external',
  'out_of_scope',
];

/// 处置 → **必填字段**。这是台账的核心设计：把「这条有没有人管」换成一个可判定的问题
/// ——「那个字段在不在」。一条 `needs_external` 若没有 `closure`，与一条空注释没有区别。
///
/// 六态各自付账的字段与判据的形状（W88 补齐第六行）：
/// - `answered_by_baseline` → `basis`（**哪一节**回答了它；MUST 至少引用一个真实存在的 `§NNN`）
/// - `decidable_in_repo` → `recommendation`（**建议怎么做**；承载 `§NNN`，逐条解析）
/// - `decided_in_repo` → `landing`（**落在仓库的哪里**；每个元素 MUST 是**仓库成员路径**）
/// - `needs_precondition` → `precondition`（**等哪些前置**；id MUST 解析得到）
/// - `needs_external` → `closure`（**外部闭合条件**）
/// - `out_of_scope` → `note`（**为什么不在范围**）
///
/// 注意 `decided_in_repo` 的必填字段是**落地点**而不是「决定文本」——与 `answered_by_baseline`
/// 用 `basis`（答在哪一节）而不是「答案原文」是**同一条纪律**：台账钉的是「**管到哪**」这个可判事实，
/// 而内容的语义是否真的落地**刻意留给人读**（ADR 0054 决策 2）。决定的内容有它的家：ADR 与报告。
export const REQUIRED_FIELD = new Map([
  ['answered_by_baseline', 'basis'],
  ['decidable_in_repo', 'recommendation'],
  ['decided_in_repo', 'landing'],
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
/// 它与 `CITATION_FIELDS`、`REPO_PATH_FIELDS` 一起，必须**恰好覆盖** `REQUIRED_FIELD` 的全部取值。
export const ID_LIST_FIELDS = ['precondition'];

/// **仓库路径**：元素是「仓库成员路径」而非 `§NNN`、也不是登记 id 的字段（W88 新增）。
///
/// 为什么需要一个新类别而不是把 `landing` 塞进 `ID_LIST_FIELDS`：`ID_LIST_FIELDS` 的元素按
/// **登记 id** 解析（`W\d{2}` / `[SRABOPMHX]\d{2}`），而落地点是**文件路径**。
/// 两者虽同为「必须解析得开」，但**解析器不同** ⇒ 归两个类别（W66「不合并不同维度」）。
///
/// 为什么落地点用**路径**而不是作业项编号：实测基线的工作项表**不含** W41–W55
/// （§47.6 的编号说明：V3.7 计划止于 W40，W41–W44／W46–W55 由后续实际执行轮次占用，
/// 第四部分从 W56 起编号）⇒ PD-09 的落地轮次 **W55 无法作为 `W\d{2}` 解析**。
/// 路径则**恒可判**：成员集由 `.git/index` 派生（`git-index.mjs`，W75），
/// 「已声明 ≠ 存在」这条纪律因此对落地点也成立。
export const REPO_PATH_FIELDS = ['landing'];

/// **结构字段**：由各自的专属检查判定、且**不可能**承载 `§NNN` 的字段——
/// `id` 由 `^PD-\d{2}$` 判、`disposition` 由 `DISPOSITIONS` 枚举判，两者的字符集里根本没有 `§`。
/// （`question` **不在**这里：它有专属的「非空」检查，但**同时**承载 `§NNN` ⇒ 归 `CITATION_FIELDS`。
/// 一个字段可以「既被扫、又有专属检查」——分类判的是「`§NNN` 由谁负责」。）
export const STRUCTURAL_FIELDS = ['id', 'disposition'];

/// **承载「必须可解析的引用」的全部字段**——`CITATION_FIELDS`（解析 `§NNN`）∪
/// `ID_LIST_FIELDS`（解析登记 id）∪ `REPO_PATH_FIELDS`（解析仓库成员路径）。这是引用扫描的
/// **单一来源**：校验循环遍历**它**，而不是各写一份。
///
/// **逐字写死，MUST NOT 由 `[...CITATION_FIELDS, ...ID_LIST_FIELDS, ...REPO_PATH_FIELDS]` 派生**：
/// 派生出的「声明集 == 三类别之并」是**恒真**的（W80 §九结论 5）。写死之后，`shapeErrors()` 的
/// 「双向相等」才是**内容断言**：新加一个路径清单字段却忘了排进扫描域，会被**具名**点名。
///
/// **触发条件 MUST 只由「字段是否存在」决定，MUST NOT 由处置决定**（W80 在 § 侧确立；
/// W83 把它扩展到 id 侧；W88 的 `landing` 出生即遵此律）。`precondition` 此前只在
/// `disposition === 'needs_precondition'` 时被校验——挂到别的处置上**完全静默**（实测：`PD-01`
/// （`needs_external`）加 `precondition: ["W99"]` ⇒ 空错误）。一个字段由谁解析它的引用，是
/// **字段的属性**，与「这条数据当前是什么处置」无关；把「谁负责」写成 `disposition === 'X' && …`
/// 的合取，等于让「这个字段有主」变成**条件性的**。
export const REFERENCE_FIELDS = [
  'basis',
  'introduced_in',
  'also_in',
  'recommendation',
  'closure',
  'note',
  'question',
  'precondition',
  'landing',
];

/// 台账里**允许出现**的全部字段。这是**闭世界**断言的基准：出现在条目里、却不在此集合中的字段
/// ⇒ **具名失败**（`validatePendingDecisions`）。
///
/// **逐字写死，MUST NOT 由四类派生**：派生出的「四类别之并 == `LEDGER_FIELDS`」是**恒真**的，
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
  'landing',
  'precondition',
  'closure',
  'note',
];

/// **信封**（台账顶层）里允许出现的全部键。这是**第二层**闭世界的基准（W82）。
///
/// W81 的闭世界只覆盖**条目内部**——于是顶层成了盲区：台账顶层只认三个键，三个**各有**具名检查，
/// 而**第四个键没有任何人管**。实测给顶层加 `revision: "见 §99.9"` ⇒ 校验返回**空错误**
/// （值干净的 `"见 §40.4"` 同样空错误）。⇒ **已知键各有人管 ≠ 键集封闭**。
///
/// 同样**逐字写死、MUST NOT 由「现有检查读了哪些键」派生**：派生出的闭世界是恒真的。
///
/// 与 `LEDGER_FIELDS` MUST **不相交**（`shapeErrors()`）：一个字段名只属于一个层级。
/// 最尖锐的反例是把一个**条目字段名**（`question`）写到顶层——实测两个闭世界**都不管**它
/// （条目闭世界看不见它、信封闭世界还不存在），于是静默通过。
export const TOP_LEVEL_FIELDS = ['schema_version', 'baseline', 'entries'];

/// 校验器自身两张常量表的一致性。**这不是「自检的装饰」**——它挡的是**未来某次合法编辑**
/// （加一个处置）导致的**静默失去覆盖**：
/// - 若某处置进了 `DISPOSITIONS` 却漏在 `REQUIRED_FIELD` 里，`REQUIRED_FIELD.get` 返回 `undefined`、
///   `if (field)` 短路 ⇒ 该处置的条目**没有必填字段**，校验对它沉默；
/// - 若某处置的必填字段不属于任何检查类别（`§NNN` / 登记 id / **仓库路径**），它的引用**永不被解析**。
///
/// 两种都是**危险方向的瞎**：断言还在，覆盖没了，且没有任何信号（W73 假契约、W66「删字段照样绿」同族）。
///
/// W82 增补第三条：**两个闭世界（信封 / 条目）的字段名集 MUST 不相交**——一个名字只属于一个层级。
/// 详见函数末尾的注释（它挡的是「把一个层级写错」导致两个闭世界同时在缝里放行）。
///
/// W88 增补第四条：**新增一个检查类别时，`REFERENCE_FIELDS` 的双向相等会立刻点名它**——
/// 这一次是**正向**适用（W88 的 `decided_in_repo` 的第一步「加处置」正是本条要拦的那种合法编辑）。
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
  const repoPath = new Set(REPO_PATH_FIELDS);
  const structural = new Set(STRUCTURAL_FIELDS);

  // 四类 MUST **两两不相交**：一个字段不能「由两个检查同时负责」——否则「谁在管它」不再是单一回答。
  // （W88 把第三类 `REPO_PATH_FIELDS` 加进来：`landing` 若同时被 id 校验与路径校验认领，
  // 两条检查会各自报一次，而「谁负责它」就没有单一答案。）
  const categories = [
    ['citation', cited],
    ['id-list', idList],
    ['repo-path', repoPath],
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

  // 每个必填字段都必须被**某**检查覆盖：要么扫 `§NNN` 引用、要么按 id 校验、要么按仓库成员校验。
  // 这条正是 W80 想要的不变式——把 `CITATION_FIELDS` 换回旧的三个字段，`note` 就会在这里被点名。
  for (const [disposition, field] of REQUIRED_FIELD) {
    if (!cited.has(field) && !idList.has(field) && !repoPath.has(field)) {
      errors.push(
        `required field ${field} for ${disposition} is covered by no check — it would be unchecked`,
      );
    }
  }

  // **闭世界**：四类别之并 MUST **恰等于** `LEDGER_FIELDS`（这份表是**独立写死**的，
  // 所以这不是恒真式）。两个方向都被点名：
  // ① 某字段进了 `LEDGER_FIELDS` 却没被任何一类认领 ⇒ 数据允许它出现、却无人负责它；
  // ② 某字段被分类了却不在 `LEDGER_FIELDS` 里 ⇒ 那份分类永远打不到东西（闭世界断言会先拒掉数据）。
  const allowed = new Set(LEDGER_FIELDS);
  const classified = new Set([...cited, ...idList, ...repoPath, ...structural]);
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

  // **引用扫描域 MUST 与类别划分双向相等**（W83，W88 扩展到第三类）：扫描域是**声明**
  // （`REFERENCE_FIELDS`），类别划分是**谁解析这个字段的引用**。两个方向都具名：
  // ① 声明了却无类别认领 ⇒ 它的引用**没人解析**（`precondition` 曾是这个形状的另一半：
  //    在类别里，却只在一种处置下被扫）；
  // ② 被类别认领却不在声明集里 ⇒ 引用扫描会**跳过它**（类别表说它有主，扫描却不看它）。
  // W88 的 `landing` 若只加进 `REPO_PATH_FIELDS` 而漏在 `REFERENCE_FIELDS` 里，
  // 方向②会**具名**点名它——这正是「新类别出生时就该被这条断言挡住」的值。
  const reference = new Set(REFERENCE_FIELDS);
  for (const field of reference) {
    if (!cited.has(field) && !idList.has(field) && !repoPath.has(field)) {
      errors.push(
        `reference field ${field} is declared but no check category owns it — its references would be resolved by nobody`,
      );
    }
  }
  for (const field of [...cited, ...idList, ...repoPath]) {
    if (!reference.has(field)) {
      errors.push(
        `field ${field} is owned by a check category but is not a reference field — the reference scan would skip it`,
      );
    }
  }

  // **两个层级 MUST 不相交**（W82）：一个字段名只属于一个层级。
  // 这条挡的是「把某个层级写错」——最尖锐的形状是把一个**条目字段名**写到顶层
  // （实测 `question: "§99.9"` 放顶层时两个闭世界都不管它，静默通过）。
  // 一旦有人为了「让那个顶层 `question` 合法」而把它加进 `TOP_LEVEL_FIELDS`，
  // 那个洞就**原样重开、且完全无声**——这条断言是它唯一的守卫。
  const envelope = new Set(TOP_LEVEL_FIELDS);
  for (const field of envelope) {
    if (allowed.has(field)) {
      errors.push(
        `field ${field} is declared as both an envelope field and an entry field — a name must belong to exactly one level`,
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
///
/// **W88 的迁移**：`decidable_in_repo` **5 → 4**、新增 `decided_in_repo` **1**。
/// 这不是「把一个数改成另一个数」——总数不变（34），只有类别移动了一格，
/// 而「总数对『被改类别』是瞎的」正是这张表存在的理由（W66 教训）。
///
/// **W92 的迁移**：`decidable_in_repo` **3 → 0**（三条全部迁出）、`decided_in_repo` **2 → 4**
/// （PD-22／PD-24 落到活规格）、`needs_external` **20 → 21**（PD-34 订正）。
///
/// ⚠️ 这一格归零**不是**「没有断言」：它是本轮的**成果断言**——「没有任何条目还停在
/// 『可在仓库内决定』的门口」。但它同时让**按处置取样本**的测试用例**失去了作用对象**
/// （实测 `tests/registry.test.mjs` 有两处 `find(entry => entry.disposition === X)`）——
/// 所以本轮的测试改动把样本定位改成**按字段存在**：守卫 MUST NOT 依赖「某个态一定非空」，
/// 否则一次**合法的数据变更**就会**改变判据的覆盖面**（W66「删字段照样绿」的同族形状）。
export const DISPOSITION_COUNTS = new Map([
  ['answered_by_baseline', 4],
  ['decidable_in_repo', 0],
  ['decided_in_repo', 4],
  ['needs_precondition', 4],
  ['needs_external', 21],
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
/// `{baselineVersion, baselineText, acceptanceIds, workItemIds, repositoryFiles}`。
///
/// **缺任一项即报错**，不做「缺了就跳过对应检查」的降级——那会让判据在被削弱时保持沉默。
/// W88 把 `repositoryFiles`（仓库成员路径集，由 `.git/index` 派生）加入必填依赖：
/// 少了它，`landing` 的「路径存在」会退化成恒真，而那正是本轮要钉的东西。
export function validatePendingDecisions(ledger, deps = {}) {
  const errors = [];
  const required = [
    'baselineVersion',
    'baselineText',
    'acceptanceIds',
    'workItemIds',
    'repositoryFiles',
  ];
  const missing = required.filter(key => deps[key] === undefined);
  if (missing.length > 0) {
    // 具名报出缺什么，而不是让后面的检查「因为没有输入所以看起来通过」。
    errors.push(`validation inputs are required and were not provided: ${missing.join(', ')}`);
    return errors;
  }

  if (!ledger || typeof ledger !== 'object') {
    return ['pending-decisions ledger must be a mapping'];
  }

  // **信封闭世界**（W82）：台账顶层出现的每个键 MUST ∈ `TOP_LEVEL_FIELDS`。
  // 与条目闭世界同一条纪律：**只看键名、不看值**——若写成「未知键**且**值含 `§NNN`」才报，
  // 判据的输入（值）就不等于被断言的对象（键集）。⇒ 值干净的未知键同样必须失败。
  // 三个已知信封键各有具名检查，但「已知键各有人管」**不等于**「键集封闭」：
  // 新增的第一个键此前落进盲区（实测顶层 `revision: "见 §99.9"` ⇒ 校验返回空错误）。
  const allowedTopLevel = new Set(TOP_LEVEL_FIELDS);
  for (const key of Object.keys(ledger)) {
    if (!allowedTopLevel.has(key)) {
      errors.push(
        `unknown envelope field ${key} — a top-level field the checker does not know can carry §NNN unchecked`,
      );
    }
  }
  // 信封字段 MUST **必需**：闭世界只挡「不认识的键」，它对「认识的键没人读」是瞎的。
  // 声明一个字段就必须在数据里有它 ⇒ 往 `TOP_LEVEL_FIELDS` 里错加一个幽灵字段会**立刻红**
  // （每个真实台账都缺它），而不是「静静地什么都不检查」。
  for (const field of TOP_LEVEL_FIELDS) {
    if (!Object.hasOwn(ledger, field)) {
      errors.push(
        `envelope field ${field} is missing — a declared envelope field with no value is a field nobody reads`,
      );
    }
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
  // 引用扫描按 `REFERENCE_FIELDS` 遍历，再按类别分派到三个解析器——分类判的是
  // 「这个字段的引用由谁解析」，与这条数据当前是什么处置**无关**。
  const citationSet = new Set(CITATION_FIELDS);
  const idListFields = new Set(ID_LIST_FIELDS);
  const repoPathFields = new Set(REPO_PATH_FIELDS);

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

    // 每态一个必填字段。**哪些字段是「清单型」（数组）由类别表决定，MUST NOT 靠写死字段名**：
    // 原先这里写着 `field === 'precondition'`，于是 W88 新增的 `landing` 会被当成**文本**判——
    // 「给了非空文本但一个有效路径都没有」的形状会**通过**。⇒ 清单型 = `ID_LIST_FIELDS ∪ REPO_PATH_FIELDS`。
    const field = REQUIRED_FIELD.get(entry.disposition);
    if (field) {
      const value = entry[field];
      const listLike = idListFields.has(field) || repoPathFields.has(field);
      const ok = listLike ? Array.isArray(value) && value.length > 0 : nonEmptyText(value);
      if (!ok) {
        errors.push(
          `${id}: disposition ${entry.disposition} requires a non-empty ${field}`,
        );
      }
    }

    // 台账里出现的**每一个「必须可解析的引用」**都必须解析得开。
    //
    // 扫描域取自 `REFERENCE_FIELDS`（**单一来源**），触发条件是**字段是否存在**——
    // 循环体里**没有 `disposition`**。`disposition` 只决定「哪个字段是必填的」（预设断言），
    // 不决定「哪个字段的引用要解析」（内容断言）。两者的触发条件 MUST 分开：
    // W80 把这一条应用到了 `§NNN` 侧（`recommendation`／`closure`／`note` 曾被整片漏扫），
    // W83 把它扩展到 id 侧，W88 的新类别 `REPO_PATH_FIELDS` 出生即遵此律。
    //
    // 错误措辞 MUST 是「条目 + 字段 + 值」三件齐：同一个坏引用可能同时出现在多个字段里，
    // 只说条目无法定位（W79：错误的措辞会把定位引向错误的方向）。字段名由 `${field}` 生成，
    // MUST NOT 写死——写死的字面量在新增一个清单字段的那一刻会**说谎**。
    for (const field of REFERENCE_FIELDS) {
      const value = entry[field];
      const values = Array.isArray(value) ? value : typeof value === 'string' ? [value] : [];
      if (citationSet.has(field)) {
        // `§NNN` 引用：每个都必须真实存在于当前基线。这条把「凭印象编一个章节号」排除掉；
        // 语义上是否真的回答了该问题由 ADR 与报告承载，不由脚本假装判定。
        for (const text of values) {
          for (const ref of sectionRefs(text)) {
            if (sections.size > 0 && !sections.has(ref)) {
              errors.push(
                `${id}: ${field} references §${ref}, which does not exist in the baseline`,
              );
            }
          }
        }
      } else if (repoPathFields.has(field)) {
        // **仓库成员路径**：每个元素都必须是仓库里**已跟踪**的一个文件/目录路径。
        // 「已声明 ≠ 存在」（W75）：`landing` 说「落在这里」，而这条把「这里」变回一件**可判**的事——
        // 指一个不存在的路径、或工作区里一个未提交的临时文件，都会被具名拒绝。
        // 判据的输入（跟踪集）来自 `.git/index`，MUST NOT 用 `readdir` 代替：
        // 后者会把未跟踪文件算成仓库成员（W75 的 F2 根因）。
        for (const item of values) {
          if (!nonEmptyText(item)) {
            errors.push(`${id}: ${field} entries must be non-empty strings`);
          } else if (!deps.repositoryFiles.has(item)) {
            errors.push(
              `${id}: ${field} ${item} is not a file tracked by this repository`,
            );
          }
        }
      } else {
        // 登记 id 引用：每个都必须解析为基线作业项或注册表验收 id。
        for (const item of values) {
          if (!nonEmptyText(item)) {
            errors.push(`${id}: ${field} entries must be non-empty strings`);
          } else if (/^W\d{2}$/.test(item)) {
            if (!deps.workItemIds.has(item)) {
              errors.push(`${id}: ${field} ${item} is not a work item named by the baseline`);
            }
          } else if (/^[SRABOPMHX]\d{2}$/.test(item)) {
            // 形状是验收 id、但注册表里没有它——与「写的根本不是 id」是两种不同的错，分开报。
            if (!deps.acceptanceIds.has(item)) {
              errors.push(`${id}: ${field} ${item} is not a registry acceptance id`);
            }
          } else {
            errors.push(
              `${id}: ${field} ${item} is neither a registry acceptance id nor a baseline work item`,
            );
          }
        }
      }
    }

    // answered_by_baseline 必须**指回**至少一个章节——否则「有依据」是空的。
    if (entry.disposition === 'answered_by_baseline' && sectionRefs(entry.basis).length === 0) {
      errors.push(`${id}: answered_by_baseline must cite at least one §NNN it is answered by`);
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
