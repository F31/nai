/// **归档增量与主规格的对账**（W150，ADR 0124）。
///
/// 问题形状：`openspec archive` 把一份变更的 `specs/<capability>/spec.md`（delta）挪进
/// `openspec/changes/archive/`，但它**不保证**已经把那几条 `### Requirement:` 写进
/// `openspec/specs/<capability>/spec.md`（主规格）。两层现有门控都不看这段距离：
///
/// - `openspec validate --all --strict` 只校验主规格自身的形状；
/// - `validate --archived` 只回答「归档记录能不能解析」。
///
/// 实测（W150）：手工把 `openspec/specs/local-autonomy/spec.md` 里一条 `### Requirement:` 改名后，
/// 两层仍是 **43 passed / 0 failed** 与 **146 passed / 0 failed** ⇒ 对「归档了但没合入」**零信号**。
/// 而这是一个会真实发生的失败：「归档了」在人的记忆里等于「做完了」，在仓库里却不等于「生效了」。
///
/// 判据形状（决定与理由见 ADR 0124）：
///
/// 1. **顺序重放，不是集合并集**。delta 的语义是 `ADDED` / `MODIFIED` / `REMOVED` 三类动作，按
///    归档目录名（日期前缀）排序后依次作用在一个集合上。并集会让「先加后删」与「从没加过」
///    变成同一件事——而 `REMOVED` 的存在正是为了区分它们（今天语料里 `REMOVED` 是 **0** 处，
///    这一点如实登记，不假装它被语料验证过）。
/// 2. **两方向各自具名**。`delta − 主规格` = 「归档了但没合入」；`主规格 − delta` = 「主规格里有一条
///    从没经过归档的 Requirement」。合成的 `重放 == 主规格` 会把两种不同的失败压成一句话，
///    而它们的处置完全不同（前者补合入，后者补记录）。
/// 3. **不判 `MODIFIED` 的先存性**。实测 42 条 `MODIFIED` 里有 **2** 条改的是更早就存在的
///    Requirement（`w02-w03-admission-hardening`、`engine-broker-frame-delivery`），判它会误红
///    两条合法的历史记录。同理不判 `REMOVED` 有没有删到东西。
/// 4. **能力整体缺席只报一条**。逐条报会让「这份主规格根本不存在」被稀释成 N 个数字，
///    读者看到的是 N，而不是「一个能力整个没合入」。
///
/// 判据是**纯函数 + 可注入域**：域（哪些 delta、哪些主规格、它们的文本）由调用方给，判据只做判定。
/// 真实语料侧由测试从 `.git/index` 派生（W75），因此「未跟踪的归档目录」不在域内。
import { ARCHIVE_ROOT } from './archived-proposal.mjs';

/// 主规格根目录（MUST 带尾随斜杠）。
export const SPECS_ROOT = 'openspec/specs/';

/// delta 的段标记闭世界。实测（178 份 delta）：只出现 `ADDED Requirements`（164 处）与
/// `MODIFIED Requirements`（34 处），`REMOVED` **0** 处；非标记的 `##` 标题只有 `Purpose`（53 处）。
///
/// 保留 `REMOVED` 是因为它是 delta 语义的一部分（见文件头第 1 条）；但它在当前语料上零样本
/// ⇒ 该分支只由合成用例覆盖，MUST NOT 记作「已由真实语料验证」。
export const DELTA_MARKS = ['ADDED', 'MODIFIED', 'REMOVED'];

/// delta 里允许出现的 `##` 标题（闭世界）。任何不在此列的标题都具名失败：
/// 「不认识」与「没有」在判定里是两件事，静默忽略等于把一段 Requirement 藏起来。
export const DELTA_SECTION_TITLES = [...DELTA_MARKS.map(mark => `${mark} Requirements`), 'Purpose'];

/// 主规格里唯一会承载 Requirement 的段。实测 43 份主规格全部是这个形状。
export const MAIN_REQUIREMENTS_TITLE = 'Requirements';

const REQUIREMENT_LINE = /^###[ \t]+Requirement:[ \t]*(.+?)[ \t]*$/;

/// 一份 delta 的路径。`capability` 是 `specs/` 下的一级目录名。
export function deltaPathOf(change, capability) {
  return `${ARCHIVE_ROOT}${change}/specs/${capability}/spec.md`;
}

/// 一份主规格的路径。
export function mainSpecPathOf(capability) {
  return `${SPECS_ROOT}${capability}/spec.md`;
}

/// 从**已跟踪路径**里取 delta（目录层）：`{change, capability, path}`，按 `(change, capability)` 排序。
///
/// 排序就是**重放的顺序**——日期前缀在目录名里，所以字典序即时间序。调用方 MUST NOT 再排一次
/// （两处排序就是两处顺序定义，而顺序是本判据的核心语义）。
export function deltaRefsOf(trackedPaths) {
  const refs = [];
  for (const path of trackedPaths) {
    if (typeof path !== 'string' || !path.startsWith(ARCHIVE_ROOT)) continue;
    const segments = path.slice(ARCHIVE_ROOT.length).split('/');
    // <change>/specs/<capability>/spec.md —— 只认这一种形状；归档里的 proposal.md / tasks.md 不进来。
    if (segments.length !== 4) continue;
    const [change, specsDir, capability, file] = segments;
    if (specsDir !== 'specs' || file !== 'spec.md' || change === '' || capability === '') continue;
    refs.push({ change, capability, path });
  }
  return refs.sort((left, right) =>
    left.change === right.change
      ? left.capability.localeCompare(right.capability)
      : left.change.localeCompare(right.change),
  );
}

/// 从**已跟踪路径**里取主规格：`Map<capability, path>`（按能力名排序插入，便于比对时稳定）。
export function mainSpecRefsOf(trackedPaths) {
  const names = new Set();
  for (const path of trackedPaths) {
    if (typeof path !== 'string' || !path.startsWith(SPECS_ROOT)) continue;
    const segments = path.slice(SPECS_ROOT.length).split('/');
    if (segments.length !== 2 || segments[1] !== 'spec.md' || segments[0] === '') continue;
    names.add(segments[0]);
  }
  return new Map([...names].sort().map(capability => [capability, mainSpecPathOf(capability)]));
}

/// 把一份 delta 切成 `{entries, problems}`。
///
/// `entries`：`[{mark, title, requirements}]`，按文件里的出现顺序；
/// `problems`：`[{kind, detail}]`，`kind` ∈ `unknown-section` / `orphan-requirement`。
///
/// `##` 标题是段标记的**唯一**出处：Requirement 归属上一条 `##`。落在第一个 `##` 之前的
/// Requirement 没有归属，它不会被任何重放动作覆盖 ⇒ 具名失败。
export function deltaSections(text) {
  const lines = String(text).split(/\r?\n/);
  const entries = [];
  const problems = [];
  let current = null;
  /// 文件里**所有** `### Requirement:` 行数（含放不进任何段的那些）。空 delta 的判据要用它：
  /// 「落在不认识的段里」已经另外具名报过了，再说「什么都没声明」就是一句假话。
  let declared = 0;

  for (const line of lines) {
    const heading = /^##[ \t]+(.+?)[ \t]*$/.exec(line);
    if (heading) {
      const title = heading[1];
      const mark = DELTA_MARKS.find(candidate => title === `${candidate} Requirements`);
      if (mark === undefined) {
        // `Purpose` 之类的非动作段是允许的（实测 53 处），只是它不承载任何重放动作：
        // 它下面的 Requirement 会成为 orphan 并被具名报出来，而不是被静默吞掉。
        if (!DELTA_SECTION_TITLES.includes(title)) problems.push({ kind: 'unknown-section', detail: title });
        current = null;
        continue;
      }
      current = { mark, title, requirements: [] };
      entries.push(current);
      continue;
    }
    const requirement = REQUIREMENT_LINE.exec(line);
    if (!requirement) continue;
    declared += 1;
    if (current === null) {
      problems.push({ kind: 'orphan-requirement', detail: requirement[1] });
      continue;
    }
    current.requirements.push(requirement[1]);
  }

  return { entries, problems, declared };
}

/// 把一份主规格切成 `{requirements, problems}`。
///
/// 只有 `## Requirements` 段里的 `### Requirement:` 算数。段外的 Requirement 具名失败——
/// 它读起来像生效了，实际没有任何重放动作能覆盖到它。
export function mainRequirements(text) {
  const lines = String(text).split(/\r?\n/);
  const requirements = [];
  const problems = [];
  let inside = false;
  let sawSection = false;

  for (const line of lines) {
    const heading = /^##[ \t]+(.+?)[ \t]*$/.exec(line);
    if (heading) {
      inside = heading[1] === MAIN_REQUIREMENTS_TITLE;
      if (inside) sawSection = true;
      continue;
    }
    const requirement = REQUIREMENT_LINE.exec(line);
    if (!requirement) continue;
    if (!inside) {
      problems.push({ kind: 'orphan-requirement', detail: requirement[1] });
      continue;
    }
    requirements.push(requirement[1]);
  }

  if (!sawSection && problems.length === 0 && requirements.length === 0) {
    problems.push({ kind: 'no-requirements-section', detail: MAIN_REQUIREMENTS_TITLE });
  }
  return { requirements, problems };
}

/// 判定入口。
///
/// `deltas`：`[{change, capability, text}]`，**按重放顺序**给（`deltaRefsOf` 的结果就是那个顺序）；
/// `specs`：`Map<capability, text>`。`text` 为 `null`/`undefined` 表示文件不在域内（缺文件）。
///
/// 返回 `{errors, changesChecked, deltasChecked, capabilitiesCompared, requirementsReplayed}`。
/// 计数一律由判据自己记账，调用方 MUST NOT 另行统计（「谁构造入参」只能是这一处）。
export function specMergeErrors({ deltas, specs } = {}) {
  const empty = what => ({
    errors: [`${what} was provided, so "every archived requirement is merged into the main spec" would hold over an empty set`],
    changesChecked: 0,
    deltasChecked: 0,
    capabilitiesCompared: 0,
    requirementsReplayed: 0,
  });
  if (!Array.isArray(deltas)) return empty('no list of archived spec deltas');
  if (!deltas.length) return empty('an empty list of archived spec deltas');
  if (!specs || typeof specs.size !== 'number') return empty('no map of main specs');
  if (!specs.size) return empty('an empty map of main specs');

  const errors = [];
  const replayed = new Map(); // capability -> Set<requirement>
  const provenance = new Map(); // capability -> Map<requirement, {change, mark}>
  const removedBy = new Map(); // capability -> Map<requirement, change>
  const deltasPerCapability = new Map(); // capability -> 见过的 delta 文件数
  const broken = new Set(); // 有 delta 目录但读不到 spec.md 的能力（无法重放，已具名报过）
  const changes = new Set();

  for (const delta of deltas) {
    const { change, capability } = delta;
    changes.add(change);
    deltasPerCapability.set(capability, (deltasPerCapability.get(capability) ?? 0) + 1);
    const label = `${change} | ${capability}`;

    const text = delta.text;
    if (text === null || text === undefined) {
      // 记进 `broken`：这份能力**无法重放**，已经在这里具名报过一条。若再让反向检查说
      // 「没有任何 delta 写过它」，同一条缺口就会被报两遍，而且第二遍是一句误导。
      broken.add(capability);
      errors.push(
        `${label}: the archived delta file is missing — an archive directory that lists a capability but carries no spec.md cannot be replayed, and an unreplayable record reads exactly like a merged one`,
      );
      continue;
    }

    const sections = deltaSections(text);
    for (const problem of sections.problems) {
      if (problem.kind === 'unknown-section') {
        errors.push(
          `${label}: "## ${problem.detail}" is not one of the section marks this judge knows (${DELTA_SECTION_TITLES.join(' / ')}), so every requirement under it would be invisible to the replay`,
        );
        continue;
      }
      errors.push(
        `${label}: "${problem.detail}" sits outside any ADDED/MODIFIED/REMOVED section — the replay has no action to apply to it, so it would look archived and never take effect`,
      );
    }

    if (sections.declared === 0) {
      errors.push(
        `${label}: the archived delta declares no requirement at all — a delta that says nothing cannot be told apart from one that was never merged`,
      );
    }

    if (!replayed.has(capability)) {
      replayed.set(capability, new Set());
      provenance.set(capability, new Map());
      removedBy.set(capability, new Map());
    }
    const names = replayed.get(capability);
    const origin = provenance.get(capability);
    const removals = removedBy.get(capability);

    for (const entry of sections.entries) {
      for (const name of entry.requirements) {
        if (entry.mark === 'REMOVED') {
          names.delete(name);
          origin.delete(name);
          // 删掉的 Requirement 也要留出处：否则反向检查会把它当成「从没经过归档」，
          // 而它其实有归档出处——只是那份出处的结论是「删掉」。两种情况的处置不同。
          removals.set(name, change);
          continue;
        }
        names.add(name);
        origin.set(name, { change, mark: entry.mark });
      }
    }
  }

  // 两个方向的并集：主规格有而重放没有的能力、重放有而主规格没有的能力。
  const capabilities = [...new Set([...replayed.keys(), ...specs.keys()])].sort();
  let compared = 0;

  for (const capability of capabilities) {
    // 有 delta 目录却读不到 spec.md ⇒ 这份能力**不知道**重放出来是什么。已在文件层具名报过，
    // 这里 MUST NOT 再比：拿残缺的重放集去比会造出「主规格里有一条没出处」这类假结论
    // （把「不知道」写成「没有」，正是 W134 那个缺陷的形状）。
    if (broken.has(capability)) continue;

    const expected = replayed.get(capability);
    if (expected === undefined) {
      errors.push(
        `${capability}: a main spec exists at ${mainSpecPathOf(capability)}, but no archived delta ever wrote it — the two records are not derived from each other, so the main spec states requirements nothing can be reviewed against`,
      );
      continue;
    }

    const text = specs.get(capability);
    if (text === null || text === undefined) {
      errors.push(
        `${capability}: ${expected.size} requirement(s) were assembled from ${deltasPerCapability.get(capability)} archived delta(s), but there is no main spec at ${mainSpecPathOf(capability)} — the archive holds the decision and the repository does not apply it`,
      );
      continue;
    }

    compared += 1;
    const main = mainRequirements(text);
    for (const problem of main.problems) {
      if (problem.kind === 'no-requirements-section') {
        errors.push(
          `${capability}: the main spec has no "## ${MAIN_REQUIREMENTS_TITLE}" section — without it there is no place where a merged requirement could live, so the replay has nothing to compare against`,
        );
        continue;
      }
      errors.push(
        `${capability}: "${problem.detail}" sits outside "## ${MAIN_REQUIREMENTS_TITLE}" in the main spec — it reads as in force, but the replay cannot see it, so the next reader cannot tell whether it was ever archived`,
      );
    }

    const actual = new Set(main.requirements);
    const origin = provenance.get(capability);
    const removals = removedBy.get(capability);
    // 具名两方向。先在 `expected − actual` 上走：它是「归档了但没合入」——本判据存在的理由。
    for (const name of [...expected].sort()) {
      if (actual.has(name)) continue;
      const written = origin.get(name);
      errors.push(
        `${capability}: the archived delta ${written.change} ${written.mark === 'MODIFIED' ? 'modifies' : 'adds'} "${name}", but the main spec does not carry it — archiving a change is not the same as applying it, and nothing between the two steps is being watched`,
      );
    }
    for (const name of [...actual].sort()) {
      if (expected.has(name)) continue;
      // `REMOVED` 来的要单独措辞：它**有**归档出处，只是那份出处说「删掉」。用「从没经过归档」
      // 报它，读者会以为文档写错了，而实际错的是「删了却没删」。
      const removed = removals.get(name);
      if (removed !== undefined) {
        errors.push(
          `${capability}: the archived delta ${removed} removed "${name}", but the main spec still carries it — a removal that was archived and never applied is the same gap as an addition that was never applied, in the other direction`,
        );
        continue;
      }
      errors.push(
        `${capability}: the main spec carries "${name}", but no archived delta ever introduced it — a requirement with no archived provenance was written straight into the main spec, so no record says why it exists`,
      );
    }
  }

  let replayedTotal = 0;
  for (const names of replayed.values()) replayedTotal += names.size;

  return {
    errors,
    changesChecked: changes.size,
    deltasChecked: deltas.length,
    capabilitiesCompared: compared,
    requirementsReplayed: replayedTotal,
  };
}
