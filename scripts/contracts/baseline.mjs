import { readFile, readdir } from 'node:fs/promises';
import path from 'node:path';
import { RELEASE_LINES } from './registry.mjs';

/// 冻结基线与在册状态的对账。
///
/// **它存在的原因**：基线文档的 §48.5「验收总表索引」与 §57.3「发布线汇总（收缩后，正本）」
/// **声明**了各家族、各发布线有哪些验收项、各多少条；而 `tests/registry.yaml` 是**活的**。
/// W68 在注册表新增 B07 之后，这两张表直到 W71 升基线才被收录——中间两轮**没有任何断言**
/// 会发现它们已经不一致，发现它的是人逐行读了一遍。这个模块就是那条断言。
///
/// **为什么是「对账」而不是「相等」**：基线一旦被 ADR 批准即**不可变**（AGENTS.md），所以
/// 新增一条验收项时，实现轮次**不可能**就地改基线。若断言写成硬相等，实现轮次将无法通过门控
/// ——那不是严格，是死锁。⇒ 规则是 `在册 == 基线声明 ∪ 已登记偏差`，而偏差**不是豁免**：
/// 每条必须在 `tests/registry.yaml` 的 `baseline_deviations` 里具名登记（id + 登记轮次 + 理由），
/// 且**必须对应一处真实差异**——基线收录后不清空台账，门控会因「陈旧条目」失败。
/// ⇒「已登记但尚未收录」从一个说法变成一个**可判定**的状态。
///
/// **W73 补上寿命**：对账只保证「差异必被解释」，不保证「解释不会永远留着」。
/// 故台账条目还带 `since_baseline`，且**任一条目 MUST 等于当前基线版本**——
/// 升基线时每条必须被处置（收录或显式结转）。理由与「为什么锚事件而不锚轮数」见 ADR 0051。

const BASELINE_NAME_RE = /^直播数字人 \+ 智能体系统技术方案 V(\d+)\.(\d+)\.md$/;
const ID_TOKEN = /([SRABOPMHX])(\d{2})/g;
const ID_RANGE = /([SRABOPMHX])(\d{2})\s*[–—-]\s*([SRABOPMHX])(\d{2})/g;
/// 基线版本串的形状（`readBaseline()` 产出 `V3.10` 这种形式）。台账的跨版判据按它比较。
const VERSION_RE = /^V[0-9]+\.[0-9]+$/;

/// §57.3 表格的「线」列标签 ⇒ `release_lines` 键。
///
/// 这张映射**照抄基线**（`Slice-0` 与计数行的 `slice_0` 拼写并不相同），所以必须有断言把它
/// 逐项钉住，否则改小它就是静默失效——同理见 §48.5 的家族列。
const RELEASE_LINE_LABELS = new Map([
  ['Slice-0', 'slice_0'],
  ['R1', 'r1'],
  ['R2', 'r2'],
  ['R3', 'r3'],
  ['实验', 'experimental'],
  ['能力包', 'capability_pack'],
]);

/// 展开一格里的 id 记号：`R08–R10` ⇒ R08、R09、R10；`A12–A14` 同理；单点照收。
///
/// 跨家族的「范围」（如 `S01–R04`）不是范围，退化为两个端点——基线里不存在这种写法，
/// 这里取保守处置而不是静默吞掉。
function expandIds(cell) {
  const ids = new Set();
  const rest = cell.replace(ID_RANGE, (match, head, from, tail, to) => {
    if (head !== tail) {
      ids.add(head + from);
      ids.add(tail + to);
      return ' ';
    }
    for (let n = Number(from); n <= Number(to); n += 1) {
      ids.add(head + String(n).padStart(2, '0'));
    }
    return ' ';
  });
  for (const match of rest.matchAll(ID_TOKEN)) ids.add(match[1] + match[2]);
  return ids;
}

function cellsOf(line) {
  const trimmed = line.trim();
  if (!trimmed.startsWith('|')) return null;
  const parts = trimmed.split('|').map(cell => cell.trim());
  // 首尾空段来自行首行尾的竖线。
  return parts.slice(1, parts.length - 1);
}

function isTableLine(line) {
  const trimmed = line.trim();
  return trimmed.startsWith('|');
}

function isSeparator(cells) {
  return cells.every(cell => /^:?-{2,}:?$/.test(cell));
}

/// 从 `lines` 的 `heading` 处向后取第一张表（遇到第一个空行即止），再取表后的首个非空行。
function sectionTable(lines, headingPredicate) {
  const start = lines.findIndex(headingPredicate);
  if (start === -1) return {found: false};
  let index = start + 1;
  while (index < lines.length && !isTableLine(lines[index])) index += 1;
  const rows = [];
  while (index < lines.length && isTableLine(lines[index])) {
    const cells = cellsOf(lines[index]);
    if (cells && !isSeparator(cells)) rows.push(cells);
    index += 1;
  }
  // 表头行（第一行）保留在结果里，由调用方跳过。
  while (index < lines.length && lines[index].trim() === '') index += 1;
  return {found: true, rows, after: index < lines.length ? lines[index].trim() : ''};
}

/// 解析 §48.5 的家族索引表：`| 系列 | 范围 | 位置 |`。
///
/// 只展开**第二列**：第三列将来若出现形如 `A12` 的定位（如 ADR 编号附近）不该被算成验收项。
export function parseFamilyIndex(text) {
  const lines = text.split('\n');
  const table = sectionTable(lines, line => /^#+\s*48\.5\b/.test(line));
  if (!table.found) {
    return {found: false, families: new Map(), familiesInOrder: [], errors: ['§48.5 heading not found']};
  }
  const families = new Map();
  const familiesInOrder = [];
  const errors = [];
  for (const cells of table.rows.slice(1)) {
    const label = cells[0].replace(/\*/g, '').trim();
    if (!/^[SRABOPMHX]$/.test(label)) {
      errors.push(`§48.5 row has an unreadable family label: ${JSON.stringify(cells[0])}`);
      continue;
    }
    const ids = expandIds(cells[1] ?? '');
    if (ids.size === 0) {
      errors.push(`§48.5 row ${label} declares no acceptance ids`);
      continue;
    }
    if (families.has(label)) {
      errors.push(`§48.5 declares the ${label} family twice`);
      continue;
    }
    families.set(label, ids);
    familiesInOrder.push(label);
  }
  return {found: true, families, familiesInOrder, errors};
}

/// 解析 §57.3 的发布线正本：表格 + 计数行。
export function parseReleaseLines(text) {
  const lines = text.split('\n');
  const table = sectionTable(lines, line => /^#+\s*57\.3\b/.test(line));
  if (!table.found) {
    return {
      found: false,
      lines: new Map(),
      counts: new Map(),
      total: null,
      labelsSeen: [],
      rowLabels: [],
      errors: ['§57.3 heading not found'],
    };
  }
  const errors = [];
  const declared = new Map();
  const labelsSeen = [];
  const rowLabels = [];
  for (const cells of table.rows.slice(1)) {
    const label = cells[0].replace(/\*/g, '').trim();
    rowLabels.push(label);
    const key = RELEASE_LINE_LABELS.get(label);
    if (key === undefined) {
      errors.push(`§57.3 row has an unreadable release-line label: ${JSON.stringify(cells[0])}`);
      continue;
    }
    const ids = expandIds(cells[1] ?? '');
    if (ids.size === 0) {
      errors.push(`§57.3 row ${label} declares no acceptance ids`);
      continue;
    }
    if (declared.has(key)) {
      errors.push(`§57.3 declares the ${key} line twice`);
      continue;
    }
    declared.set(key, ids);
    labelsSeen.push(key);
  }
  // 计数行：`计数：`slice_0` 10 + `r1` **37** + … = **88**（…）`
  const counts = new Map();
  let total = null;
  if (!table.after.startsWith('计数：')) {
    errors.push('§57.3 has no count line directly after its table');
  } else {
    for (const match of table.after.matchAll(/`([a-z0-9_]+)`\s*\**(\d+)\**/g)) {
      counts.set(match[1], Number(match[2]));
    }
    const totalMatch = /=\s*\**(\d+)\**/.exec(table.after);
    if (totalMatch) total = Number(totalMatch[1]);
    else errors.push('§57.3 count line has no total after "="');
  }
  return {found: true, lines: declared, counts, total, labelsSeen, rowLabels, errors};
}

/// 解析整份基线文档中与验收口径有关的两张表。
export function parseBaseline(text) {
  const familyIndex = parseFamilyIndex(text);
  const releaseLines = parseReleaseLines(text);
  return {familyIndex, releaseLines};
}

export async function readBaseline(root = '.') {
  const dir = path.join(root, 'docs');
  const names = await readdir(dir);
  const candidates = [];
  for (const name of names) {
    const match = BASELINE_NAME_RE.exec(name);
    if (match) {
      candidates.push({name, major: Number(match[1]), minor: Number(match[2])});
    }
  }
  if (candidates.length === 0) {
    throw new Error('no baseline document (docs/直播数字人 + 智能体系统技术方案 V*.md) found');
  }
  candidates.sort((a, b) => a.major - b.major || a.minor - b.minor);
  const newest = candidates[candidates.length - 1];
  const file = path.join(dir, newest.name);
  const text = await readFile(file, 'utf8');
  return {
    version: `V${newest.major}.${newest.minor}`,
    name: newest.name,
    path: file,
    text,
    knownNames: candidates.map(candidate => candidate.name),
  };
}

/// `openspec/config.yaml` 里声明的唯一基线文件名。
///
/// 文档目录里「版本号最大的那份」是一个**自维护**的选择，但若有人留下一份废弃草稿（如
/// `… V3.11.md`），选择就会悄悄指向它。所以选择结果 MUST 与配置声明的指针一致——两者不一致
/// 时门控具名报出，而不是让门控悄悄对着草稿跑。
export function declaredBaselineName(configText) {
  const match = /docs\/(直播数字人 \+ 智能体系统技术方案 V\d+\.\d+\.md)/.exec(configText);
  return match ? match[1] : null;
}

/// 偏差台账的规范化与自检。条目形状：
/// `{id, since_baseline, registered, carried_at?, reason}`。
///
/// **跨版不变式（ADR 0051 决策 2）**：`since_baseline` MUST 等于当前基线版本。
/// 「紧接着订正」在实现轮次内没有可执行落点（基线被 ADR 批准后不可变，就地订正有 MUST 1:1
/// 行替换的硬约束 ⇒ 改 §57.3 只能发生在一次专门的升基线轮次里），故它被读作「不晚于下一次
/// 基线修订轮次」；可判定形式就是这一条——跨版只有两种处置：收录（删条目），或显式结转
/// （改 `since_baseline` + 记 `carried_at` + 非空理由）。
///
/// `version` 是**必需**入参而不是可选：缺了它这条判据恒真，而一条会因为入参缺失而恒真的断言
/// 比没有断言更坏。缺失时由 `reconcile` 具名报出。
export function normalizeDeviations(raw, errors, version) {
  const out = [];
  if (raw === undefined) return out;
  if (!Array.isArray(raw)) {
    errors.push('baseline_deviations must be a list');
    return out;
  }
  const knownVersion = VERSION_RE.test(String(version ?? ''));
  const seen = new Set();
  for (const entry of raw) {
    if (!entry || typeof entry !== 'object') {
      errors.push('baseline_deviations entry must be a mapping');
      continue;
    }
    if (!/^[SRABOPMHX][0-9]{2}$/.test(entry.id ?? '')) {
      errors.push(`baseline_deviations entry has an invalid id: ${JSON.stringify(entry.id)}`);
      continue;
    }
    if (seen.has(entry.id)) {
      errors.push(`baseline_deviations declares ${entry.id} twice`);
      continue;
    }
    seen.add(entry.id);
    if (!/^W[0-9]+$/.test(String(entry.registered ?? ''))) {
      errors.push(`baseline_deviations ${entry.id} must name the registering round (W<number>)`);
    }
    if (typeof entry.reason !== 'string' || entry.reason.trim() === '') {
      errors.push(`baseline_deviations ${entry.id} must carry a non-empty reason`);
    }
    // 条目 MUST 声明它相对于哪一份基线成立；与当前基线不一致 = 跨版存活。
    if (!VERSION_RE.test(String(entry.since_baseline ?? ''))) {
      errors.push(
        `baseline_deviations ${entry.id} must name the baseline it deviates from (since_baseline: V<major>.<minor>)`,
      );
    } else if (knownVersion && entry.since_baseline !== version) {
      errors.push(
        `baseline_deviations ${entry.id} was registered against ${entry.since_baseline}, but the current baseline is ${version}` +
          `; collect it at this baseline round, or carry it forward explicitly (update since_baseline, add carried_at and a reason)`,
      );
    }
    if (entry.carried_at !== undefined) {
      if (!/^W[0-9]+$/.test(String(entry.carried_at ?? ''))) {
        errors.push(`baseline_deviations ${entry.id} carried_at must be a round (W<number>)`);
      } else if (
        /^W[0-9]+$/.test(String(entry.registered ?? '')) &&
        Number(String(entry.carried_at).slice(1)) <= Number(String(entry.registered).slice(1))
      ) {
        errors.push(
          `baseline_deviations ${entry.id} carried_at (${entry.carried_at}) must be later than its registering round (${entry.registered})`,
        );
      }
    }
    out.push({
      id: entry.id,
      since_baseline: entry.since_baseline,
      registered: entry.registered,
      carried_at: entry.carried_at,
      reason: entry.reason,
    });
  }
  return out;
}

function lineOfIds(registry) {
  const byLine = new Map();
  for (const id of RELEASE_LINES) byLine.set(id, new Set());
  for (const item of registry.acceptance ?? []) {
    if (!byLine.has(item.release_line)) byLine.set(item.release_line, new Set());
    byLine.get(item.release_line).add(item.id);
  }
  return byLine;
}

/// 基线声明与注册表实测的对账。返回具名错误列表；空列表 = 一致。
///
/// `version` 是**当前基线版本**（`readBaseline()` 的 `version`），必需：
/// 台账的跨版判据没有它就不可判定，而「不可判定」MUST 失败，MUST NOT 静默通过。
export function reconcile(registry, parsed, version) {
  const errors = [];
  if (!VERSION_RE.test(String(version ?? ''))) {
    errors.push(
      `reconcile needs the current baseline version (got ${JSON.stringify(version ?? null)}); ` +
        'the ledger\'s cross-version rule cannot be decided without it',
    );
  }
  const deviations = normalizeDeviations(registry.baseline_deviations, errors, version);
  const registeredIds = new Set(deviations.map(deviation => deviation.id));

  const declaredLines = parsed.releaseLines.lines;
  const measuredLines = lineOfIds(registry);

  // 解析本身的错误先落。它们不是设计漂移，必须与「声明落后于在册」分开报——否则
  // 「§57.3 改名了」会被读成「注册表多了 88 条」。
  errors.push(...parsed.releaseLines.errors);
  errors.push(...parsed.familyIndex.errors);

  // ── ① §57.3 表内自洽：计数行必须等于同节表格展开后的条数 ──────────────
  // 两张声明同处一节、由不同的写法给出（表格 vs 计数行），它们可以各自漂移。
  if (parsed.releaseLines.found) {
    for (const id of RELEASE_LINES) {
      const ids = declaredLines.get(id);
      if (!ids) {
        errors.push(`§57.3 declares no row for the ${id} release line`);
        continue;
      }
      const declaredCount = parsed.releaseLines.counts.get(id);
      if (declaredCount === undefined) {
        errors.push(`§57.3 count line omits \`${id}\``);
      } else if (declaredCount !== ids.size) {
        errors.push(
          `§57.3 is self-inconsistent: \`${id}\` counts ${declaredCount} but its row lists ${ids.size}`,
        );
      }
    }
    const sum = [...declaredLines.values()].reduce((acc, ids) => acc + ids.size, 0);
    if (parsed.releaseLines.total !== sum) {
      errors.push(
        `§57.3 total ${parsed.releaseLines.total} does not equal the sum of its rows (${sum})`,
      );
    }
    for (const key of parsed.releaseLines.counts.keys()) {
      if (!RELEASE_LINES.includes(key)) {
        errors.push(`§57.3 count line names an unknown release line: ${key}`);
      }
    }
    if (declaredLines.size !== RELEASE_LINES.length) {
      errors.push(
        `§57.3 declares ${declaredLines.size} release lines, expected ${RELEASE_LINES.length}`,
      );
    }
  }

  // ── ② 发布线正本 vs 在册：id 级的集合对账 ───────────────────────────
  const declaredLineOf = new Map();
  for (const [line, ids] of declaredLines) {
    for (const id of ids) {
      if (declaredLineOf.has(id)) {
        errors.push(`§57.3 declares ${id} on both ${declaredLineOf.get(id)} and ${line}`);
      }
      declaredLineOf.set(id, line);
    }
  }
  const measuredLineOf = new Map();
  for (const [line, ids] of measuredLines) {
    for (const id of ids) measuredLineOf.set(id, line);
  }

  const issues = [];
  for (const [id, line] of measuredLineOf) {
    if (!declaredLineOf.has(id)) issues.push({kind: 'extra', id, line});
    else if (declaredLineOf.get(id) !== line) {
      issues.push({kind: 'moved', id, from: declaredLineOf.get(id), to: line});
    }
  }
  for (const [id, line] of declaredLineOf) {
    if (!measuredLineOf.has(id)) issues.push({kind: 'missing', id, line});
  }
  const render = issue => {
    if (issue.kind === 'extra') return `${issue.id} is registered on ${issue.line} but absent from §57.3`;
    if (issue.kind === 'missing') return `${issue.id} is declared on §57.3's ${issue.line} but not registered`;
    return `${issue.id} is registered on ${issue.to} but §57.3 declares it on ${issue.from}`;
  };
  for (const issue of issues) {
    if (registeredIds.has(issue.id)) continue;
    errors.push(
      `${render(issue)}; either correct §57.3 at the next baseline round or register it in tests/registry.yaml baseline_deviations`,
    );
  }
  // 台账 MUST NOT 含陈旧条目：基线收录之后不清空，门控必须失败。
  const issueIds = new Set(issues.map(issue => issue.id));
  for (const deviation of deviations) {
    if (!issueIds.has(deviation.id)) {
      errors.push(
        `baseline_deviations lists ${deviation.id}, but it is not a difference from §57.3/§48.5 any more (collected at a baseline round? clear the entry)`,
      );
    }
  }

  // ── ③ 家族索引 vs 在册：家族完整性的判据来自 §48.5，而不是一张写死的表 ──
  const declaredFamilies = parsed.familyIndex.families;
  const measuredFamilies = new Map();
  for (const id of measuredLineOf.keys()) {
    const family = id[0];
    if (!measuredFamilies.has(family)) measuredFamilies.set(family, new Set());
    measuredFamilies.get(family).add(id);
  }
  if (parsed.familyIndex.found) {
    for (const family of measuredFamilies.keys()) {
      if (!declaredFamilies.has(family)) {
        errors.push(`§48.5 declares no row for family ${family}, but the registry registers its ids`);
      }
    }
    for (const [family, declared] of declaredFamilies) {
      const measured = measuredFamilies.get(family) ?? new Set();
      const extra = [...measured].filter(id => !declared.has(id) && !registeredIds.has(id));
      const missing = [...declared].filter(id => !measured.has(id) && !registeredIds.has(id));
      if (extra.length > 0) {
        errors.push(
          `§48.5 family ${family} stops short of registered ids: ${extra.join(', ')}; either widen the range at the next baseline round or register the ids in baseline_deviations`,
        );
      }
      if (missing.length > 0) {
        errors.push(
          `§48.5 family ${family} declares ids that are not registered: ${missing.join(', ')}`,
        );
      }
    }
    if (declaredFamilies.size === 0) {
      errors.push('§48.5 parsed to no families at all');
    }
  }
  return errors;
}

/// 基线文件选择的自洽性：目录里版本号最大的那份 MUST 就是配置声明的唯一基线。
export function pointerErrors(baseline, configText) {
  const errors = [];
  const declared = declaredBaselineName(configText);
  if (declared === null) {
    errors.push('openspec/config.yaml does not declare a baseline document path');
    return errors;
  }
  if (declared !== baseline.name) {
    errors.push(
      `openspec/config.yaml declares ${declared}, but the newest baseline in docs/ is ${baseline.name}`,
    );
  }
  if (baseline.knownNames.length < 2) {
    errors.push(
      `only ${baseline.knownNames.length} baseline document(s) in docs/; "newest wins" is not being exercised`,
    );
  }
  return errors;
}
