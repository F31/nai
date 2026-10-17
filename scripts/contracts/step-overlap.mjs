/// CI 执行面上**重复执行**的闭世界：同一批测试 MUST NOT 被跑两遍。
///
/// 这个模块存在的理由（W134）：`foundation.yml` 的 gate 步跑
/// `cargo xtask check --profile full`，而 `engineering_checks` 里已经有
/// `cargo test --workspace --locked --exclude <harness>`——**整个 workspace 的全部测试**。
/// 紧随其后的 7 条 `cargo xtask test --suite …` 步跑的包集是那次全量运行的**子集**
/// （`storage`/`kernel`/`hosts`/`session`/`scheduler`/`provider`/`knowledge`），
/// 于是同一批测试在**每一个平台腿上各被执行了第二遍**。
///
/// 实测（本机暖缓存、无编译，只计执行）：7 条步合计 **517 秒**
/// （storage 125 / kernel 108 / scheduler 89 / session 77 / hosts 43 / knowledge 40 / provider 35），
/// 而它们跑的东西 `check --profile full` 刚刚跑完。CI 的 windows runner 只会更慢。
///
/// 为什么既有四层判据都看不见它：
/// - W95 步层闭世界：管「这一步在不在册」；
/// - W100 执行配置：管「这一步会不会跑、能不能被吞」；
/// - W101 目标绑定：管「这一步跑的是不是它自称的那个目标」；
/// - W97 覆盖闭包：管「门控与 suite 的包集合有没有漏」。
/// 四条都正确，四条都**不问**「这两个目标覆盖的是不是同一批测试」——**重复是它们共同的盲区**。
///
/// 与母题同形（W95 → W100 → W101 → **本轮：覆盖关系之间**），四条纪律照抄：
/// 1. **判定域由解析得出**：包集从 `xtask/src/lib.rs` 的 dispatch 臂与门控区域算出来，
///    MUST NOT 在表里手写第二份（W93「同一件事 MUST NOT 两处定义」）；
/// 2. **不可解析 ⇒ 具名失败并分类**，MUST NOT 静默当成「没有覆盖」——空集对包含关系恒真；
/// 3. **每一条覆盖 MUST 被归入闭集里的一格**：未归类的目标是一条**没有判据**的判定；
/// 4. **判据 MUST 由注入证明会红**：基线里一条 `cargo-suite` 步都没有时，包含关系判断
///    对空集恒真 ⇒ 该逻辑只靠单元注入保持可验（见 `tests/contracts/step-overlap.test.mjs`）。

import {WORKFLOW_STEPS, workflowStepsOf} from './workflow-steps.mjs';
import {coveragesOf, resolveCoverage, suiteArmBody} from './gate-coverage.mjs';
import {gateRegions} from './xtask-commands.mjs';

/// 一个在册判定步的 `target` 在**覆盖关系**上的类别。闭集：域外取值 ⇒ 具名失败
/// （W90：一个未知类别是一个标签，不是一个分类）。
///
/// - `gate`：本机全量门控（`check --profile full`），覆盖 = `engineering_checks` 里
///   全部 `["test", …]` 的并集；
/// - `suite`：`test --suite X`。它的**包集**由 dispatch 臂的体解析出来：体是一条
///   `["test", …]` ⇒ 有包集（参与包含关系）；体交给别的函数（`contracts` 走
///   `schema::contracts`，跑 C++ 夹具）⇒ 没有 cargo 包集，**不参与**包含关系。
///   这两者的区分是**读源码得出的**，不是猜的：`suiteCoverageOf` 返回 `null` 就是后者；
/// - `non-test-target`：`schema --check`、`check --secrets`、node 测试、pnpm 脚本。
export const COVERAGE_CLASSES = ['gate', 'suite', 'non-test-target'];

/// 判定步（产生结论的那些 kind）。`infra` / `diagnostic` 不产生判定，不进本判据。
const JUDGED_KINDS = ['local', 'ci-only'];

/// 把一个 `target` 归入覆盖类别。未归类 ⇒ 返回 `null`（由调用方具名）。
///
/// ⚠️ 归类只读 `shape` 与 `value` 的**形状**，不读步的名字：名字是身份，不是类别（W91）。
export function coverageClassOf(target) {
  if (!target || typeof target !== 'object') return null;
  const {shape, value} = target;
  if (typeof value !== 'string' || value === '') return null;
  if (shape === 'xtask') {
    if (/^check --profile (portable|full|windows-local)$/.test(value)) return 'gate';
    if (/^test --suite [A-Za-z0-9_-]+$/.test(value)) return 'suite';
    return 'non-test-target';
  }
  if (shape === 'node-test' || shape === 'pnpm') return 'non-test-target';
  return null;
}

/// 从门控区域取**门控步的覆盖包集**：区域里每条 `["test", …]` 结算后取并集。
///
/// 为什么是并集而不是「第一条」：`engineering_checks` 有两条 test 命令
/// （`--workspace --exclude <harness>` 与 `--package <harness> --lib`），
/// 两条都是门控真正执行的测试 ⇒ 门控覆盖是它们的并集。取第一条会**少报**门控覆盖，
/// 于是一条被门控覆盖的步会被判成「不重复」——假阴性，方向不安全。
export function gateCoverageOf(harnessText, packages, identifiers) {
  if (typeof harnessText !== 'string' || harnessText === '') {
    throw new Error('the harness source was not provided, so the gate coverage cannot be read');
  }
  const regions = gateRegions(harnessText);
  const region = regions.find(candidate => candidate.name === 'engineering_checks');
  if (!region) throw new Error('the harness carries no `engineering_checks` region');
  const {coverages, unresolvable} = coveragesOf(region.text, identifiers);
  if (unresolvable.length > 0) {
    // 不可解析的实参 MUST 具名上抛：把它当成「没有覆盖」会让包含关系对空集恒真。
    const token = unresolvable.map(item => item.token).join(', ');
    throw new Error(`the gate region carries a test command whose operands cannot be resolved: ${token}`);
  }
  if (coverages.length === 0) {
    throw new Error('the gate region runs no cargo test command, so its coverage is empty');
  }
  const union = new Set();
  for (const coverage of coverages) {
    for (const name of resolveCoverage(coverage, packages)) union.add(name);
  }
  return union;
}

/// 一个 `suite` 步的覆盖包集。体里**恰好一条** `["test", …]` ⇒ 结算它；
/// **零条** ⇒ `null`（这条套件跑的不是 cargo 的包测试，例如 `contracts` 走 C++ 夹具）；
/// **多条或实参不可解析** ⇒ 抛（「两条命令」与「一条命令的实参读不懂」都不该被猜成一条集合）。
///
/// `null` 与「空集」是两件事：空集会让包含关系对空集恒真（假红），
/// `null` 是**类别事实**——它不参与包含关系，而不是「什么都没覆盖」。
export function suiteCoverageOf(harnessText, suite, packages, identifiers) {
  const body = suiteArmBody(harnessText, suite);
  const {coverages, unresolvable} = coveragesOf(body, identifiers);
  if (unresolvable.length > 0) {
    const token = unresolvable.map(item => item.token).join(', ');
    throw new Error(`the suite "${suite}" carries a test command whose operands cannot be resolved: ${token}`);
  }
  if (coverages.length === 0) return null;
  if (coverages.length !== 1) {
    throw new Error(`the suite "${suite}" resolves to ${coverages.length} cargo test commands, not one`);
  }
  return resolveCoverage(coverages[0], packages);
}

/// 集合的**包含判定**。相等也算重复——同一批测试跑两遍，方向无所谓。
function covers(outer, inner) {
  if (inner.size === 0) return false;
  for (const name of inner) {
    if (!outer.has(name)) return false;
  }
  return true;
}

function sorted(values) {
  return [...values].sort();
}

/// **主判据**。判「同一 workflow 里，有没有两个判定步覆盖的是同一批测试」。
///
/// 入参**显式注入**，缺任一项即具名报错——一条会因入参缺失而恒真的断言比没有断言更坏。
///
/// - `host`：workflow 的仓库路径（在册条目按它归属）；
/// - `workflowText`：该文件正文；
/// - `harnessText`：`xtask/src/lib.rs` 全文（覆盖的唯一真源）；
/// - `packages`：`Set<string>`，workspace 的包名集合（`workspace` 形态结算用）；
/// - `identifiers`：`Map<string, string>`，常量标识符 → 值（默认只需 `HARNESS`）；
/// - `registered`：在册步表，默认 `WORKFLOW_STEPS`。
export function stepCoverageErrors({
  host,
  workflowText,
  harnessText,
  packages,
  identifiers = new Map(),
  registered = WORKFLOW_STEPS,
} = {}) {
  const errors = [];

  if (typeof host !== 'string' || host === '') {
    errors.push('the step coverage judgement requires the workflow host path and it was not provided');
  }
  if (typeof workflowText !== 'string' || workflowText === '') {
    errors.push('the step coverage judgement requires the workflow text and it was not provided');
  }
  if (typeof harnessText !== 'string' || harnessText === '') {
    errors.push('the step coverage judgement requires the harness source and it was not provided');
  }
  if (!(packages instanceof Set) || packages.size === 0) {
    errors.push(
      'the step coverage judgement requires the workspace package set — an empty set makes every ' +
        'containment check vacuous',
    );
  }
  if (!Array.isArray(registered) || registered.length === 0) {
    errors.push('the step coverage judgement requires a non-empty registered step set — an empty registry is vacuous');
  }
  if (errors.length > 0) return errors;

  let steps;
  try {
    steps = workflowStepsOf(workflowText);
  } catch (error) {
    // 不可解析 ⇒ 具名失败。MUST NOT 静默返回空错误（读不懂被伪装成没问题）。
    errors.push(`${host} could not be parsed, so its step coverage cannot be judged: ${error.message}`);
    return errors;
  }

  const entries = registered.filter(entry => entry.host === host && JUDGED_KINDS.includes(entry.kind));
  const present = new Set(steps.map(step => step.identity));
  const judged = [];

  for (const entry of entries) {
    // 在册但不在文件里的步由 W95 的闭世界具名，这里不重复判决（判定域各自的）。
    if (!present.has(entry.identity)) continue;
    const targets = Array.isArray(entry.target) ? entry.target : [];
    if (targets.length === 0) continue;
    for (const target of targets) {
      const kind = coverageClassOf(target);
      if (kind === null) {
        errors.push(
          `${host} step ${JSON.stringify(entry.identity)} carries the target ${JSON.stringify(target)}, which ` +
            'falls into no coverage class — an unclassified target is a judgement with no coverage rule',
        );
        continue;
      }
      if (kind === 'non-test-target') continue;
      let coverage;
      try {
        coverage = kind === 'gate'
          ? gateCoverageOf(harnessText, packages, identifiers)
          : suiteCoverageOf(harnessText, target.value.replace(/^test --suite /, ''), packages, identifiers);
      } catch (error) {
        errors.push(`${host} step ${JSON.stringify(entry.identity)} has an unreadable coverage: ${error.message}`);
        continue;
      }
      // `suite` 类里「没有 cargo 包集」的那些（`contracts`）以 `coverage: null` 记入
      // ⇒ 它们**被数进了判过的集合**，但不参与包含关系。跳过它们不算静默：
      // 类别本身已经由解析结果确定，且下一段的两两比较显式跳过 `null`。
      judged.push({identity: entry.identity, kind, coverage});
    }
  }

  // **两两包含**：`b ⊆ a`（含相等）⇒ 同一批测试被执行两次。
  // 只比 `a` 在前的方向，避免相等时同一对报两遍。
  for (let i = 0; i < judged.length; i += 1) {
    for (let j = i + 1; j < judged.length; j += 1) {
      const a = judged[i];
      const b = judged[j];
      if (a.coverage === null || b.coverage === null) continue;
      const ab = covers(a.coverage, b.coverage);
      const ba = covers(b.coverage, a.coverage);
      if (!ab && !ba) continue;
      const outer = ab ? a : b;
      const inner = ab ? b : a;
      errors.push(
        `${host} runs the same tests twice: step ${JSON.stringify(inner.identity)} covers ` +
          `${JSON.stringify(sorted(inner.coverage))}, which is already covered by step ` +
          `${JSON.stringify(outer.identity)} (${JSON.stringify(sorted(outer.coverage))}) — a second ` +
          'execution of the same packages buys no judgement, only wall-clock',
      );
    }
  }

  // **非空洞**：一条都没判到 ⇒ 下面的包含关系无对象可判。
  if (judged.length === 0) {
    errors.push(
      `${host} carries no judged step with a coverage, so no containment was judged — an empty judged set ` +
        'makes the containment rule vacuously true',
    );
  }

  return errors;
}
