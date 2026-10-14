/// 在册判定步的**执行配置面**（`matrix` × `if:` × `continue-on-error`）的兑付闭世界。
///
/// W95 给 `.github/workflows/foundation.yml` 的 step 集补了步层闭世界（步在不在、`kind` 与
/// `localEvidence` 是否付代价、`diagnostic` 是否由 `if: failure…` 认出）。但每个步的
/// **执行配置**——**跑不跑**（`if:`）、**在哪个平台跑**（`matrix`）、**失败了会不会被吞**
/// （`continue-on-error`）——**从来没有被任何判据读过**：
///
/// ```text
/// workflowStepsOf(text) → [{identity, condition, run}]   ← W95 解析器只产出这三个字段
///                         ↑ condition 只被 diagnostic 分支读一次：condition.includes('failure')
/// matrix / continue-on-error                              ← 解析器根本不产出，判据无从读起
/// ```
///
/// 实测（W100 探针，改**真文件**、真跑 8 个 portable 套件、逐字节还原、`sha256` 逐字一致）：
/// - P-A1 把 Linux-only 步的 `if: runner.os == 'Linux'` 改成 `'FreeBSD'` ⇒ `[0,…]` **零信号**；
/// - P-A2 把矩阵从两个平台缩成 `[windows-latest]` ⇒ `[0,…]` **零信号**——这一改动让两条
///   Linux-only 步（其中一条是三语言契约套件）**永远不会跑**，而 21 个步「都在册、身份都在、
///   `kind` 都在闭集里」——**每一格都还是绿的**；
/// - P-A3/P-A6 给 `local` 步加 `continue-on-error: true` ⇒ `[0,…]` **零信号**
///   （判定能被吞掉，门控可以全绿）；
/// - P-A4 删掉 Linux C++ 安装步的平台条件 ⇒ `[0,…]` **零信号**；
/// - P-A5（**对照组**）删掉诊断步的 `if: failure()` ⇒ `workflow` 套件 **2 条红**——W95 已兑付，
///   说明上面那些「零信号」不是因为判据整体失效。
///
/// 于是补法与母题同形（`repositoryFiles` → `readTrackedText` → W94 两张事实表 → W95 `gateSources`
/// → W96 门控闭包区域 → W97 包集合 → W98 npm 脚本执行面 → W99 站点层级声明 → **本轮：执行配置**）。
/// 四条纪律照抄自前轮：
/// 1. **判定域由解析得出**（W96）：`workflowJobsOf` 给出的 `matrix` / `condition` / `continueOnError`，
///    而不是「字段存不存在」——`if:` 存在但恒假，与「没有 `if:`」在**集合层**上是两回事（P-A2）。
/// 2. **集合级，不是字段级**（W97）：可达性问的是「执行平台集合**非空**吗」，
///    不是「`if:` 有没有写」。
/// 3. **不可解析 ⇒ 失败并分类，不静默降级**（W90）：取值域外的 runner 标签、域外的
///    `runner.os` 取值、形状表外的条件一律**具名**，MUST NOT 当成「没写条件」放过去。
/// 4. **对称方向也要堵**（W96「逃离」教训）：`platform-toolchain` 要求平台限制，
///    其余三因则**禁止**平台限制——只做前者会让「给无关步加个平台条件」成为一条合法路径。
///
/// ⚠️ **静态求值，MUST NOT 执行**（铁律二）：这里只做**闭形状**分类，不实现 GitHub 的
/// `if:` 表达式求值语义。宿主瞬态（有没有 apt、能不能派生子进程）MUST NOT 改变结论。

import {CI_ONLY_REASONS, WORKFLOW_STEPS, workflowJobsOf} from './workflow-steps.mjs';

/// `runner.os` 的**取值闭集**。域外取值 ⇒ 具名失败。
///
/// 为什么需要它（P-A1 的守卫）：`if: runner.os == 'FreeBSD'` 是一段**语法正确、语义永假**的比较。
/// GitHub 的 runner 只会报 `Linux` / `Windows` / `macOS` 三者之一 ⇒ 一个域外取值不是
/// 「条件不成立」，而是**这条步永远不会跑**。
export const RUNNER_OS_VALUES = ['Linux', 'Windows', 'macOS'];

/// runner 标签前缀 ⇒ OS 族的**字典**。字典里没有的标签 ⇒ 具名失败。
///
/// 这是**平台事实的字典**（与 `TEST_EXTENSIONS` 同形），不是一个必须每格有实例的规则表：
/// 矩阵里有什么标签就该认什么，MUST NOT 要求「所有平台都出现在矩阵里」。
export const RUNNER_LABEL_PREFIXES = [
  {prefix: 'ubuntu-', os: 'Linux'},
  {prefix: 'windows-', os: 'Windows'},
  {prefix: 'macos-', os: 'macOS'},
];

/// **`if:` 的形状闭世界**。只认两种形状，其余（`always()`、`success()`、
/// `github.event_name == …`、`!cancelled()` …）一律**具名拒绝**。
///
/// 这是一张**规则表**（与 W99 的 `TIER_PAYMENTS` 同形）：每个形状 MUST 至少有一个在册步在用，
/// 否则那一格的判据对空集恒真。新增一个形状而不在这里归类，会让判据**静默地不认识它**。
/// 拒否方向落在**假阳性**（要求人显式归类），安全。
export const CONDITION_SHAPES = ['platform-os', 'failure'];

/// **平台专属命令形状**：命中它的 `run` ⇒ 该步的执行平台集合 MUST ⊆ `supported` 且非空。
///
/// 与 W95 的 `JUDGE_COMMAND_SHAPES` 同一族（都验**命令形状**，不验语义），但断言的是
/// **平台可得性**：`sudo` 在 Linux 与 macOS 上都有、Windows runner 上没有；
/// `apt-get` 只在 Debian 系 Linux 上。⇒ 支持集 MUST 按**实测的平台事实**写——
/// 把 `sudo` 的支持集写成 `['Linux']` 会在 macOS 矩阵下误报一个**正确**的配置。
///
/// 与 `JUDGE_COMMAND_SHAPES` 一样，这是一张**规则表**：每个形状 MUST 至少有一个实例。
export const PLATFORM_COMMAND_SHAPES = [
  {
    id: 'posix-privilege-escalation',
    supported: ['Linux', 'macOS'],
    pattern: /(?:^|[\s;&|])sudo\s/,
  },
  {
    id: 'debian-package-manager',
    supported: ['Linux'],
    pattern: /(?:^|[\s;&|])(?:apt-get|apt)\s/,
  },
];

/// runner 标签 ⇒ OS 族。字典里没有 ⇒ 具名失败并返回 `null`。
export function osFamilyOf(label, errors = [], where = 'the workflow') {
  const hit = RUNNER_LABEL_PREFIXES.find(entry => String(label).startsWith(entry.prefix));
  if (hit === undefined) {
    errors.push(
      `${where} names runner label ${JSON.stringify(label)}, which is not in the recognized label table ` +
        `(${RUNNER_LABEL_PREFIXES.map(entry => `${entry.prefix}*`).join(', ')}) — a label nobody can classify ` +
        'is a platform nobody checks',
    );
    return null;
  }
  return hit.os;
}

/// 一个 job 的**平台腿**（runner 标签列表）。不可读 ⇒ 具名失败并返回 `null`。
///
/// 闭世界三条：① 有矩阵时 MUST 只有唯一的 `os` 轴，且 `runs-on` MUST 消费它
/// （一个没人消费的矩阵是「声明了但没有任何东西按它走」）；② 矩阵取值 MUST 是非空的标签列表
/// （空矩阵 ⇒ 零个 job ⇒ 连「不跑」都算不上，是**声明本身空洞**）；③ 没有矩阵时
/// `runs-on` MUST 是字面标签（含 `${{ }}` 却无矩阵 ⇒ 平台集不可读）。
export function platformsOf(job, errors = [], where = 'the workflow') {
  const reference = /\$\{\{\s*matrix\.([A-Za-z0-9_-]+)\s*\}\}/.exec(job.runsOn ?? '');
  const matrix = job.matrix;
  if (matrix === undefined || matrix === null) {
    if (reference !== null) {
      errors.push(
        `${where} runs on ${JSON.stringify(job.runsOn)} but declares no matrix — the platform set cannot be read`,
      );
      return null;
    }
    if (typeof job.runsOn !== 'string' || job.runsOn === '') {
      errors.push(`${where} declares neither a matrix nor a literal runs-on — the platform set cannot be read`);
      return null;
    }
    return osFamilyOf(job.runsOn, errors, where) === null ? null : [job.runsOn];
  }

  if (typeof matrix !== 'object' || Array.isArray(matrix)) {
    errors.push(`${where} declares a matrix that is not a mapping — the platform set cannot be read`);
    return null;
  }
  const axes = Object.keys(matrix);
  if (axes.length !== 1 || axes[0] !== 'os') {
    errors.push(
      `${where} declares matrix axes ${JSON.stringify(axes)}, but only the single "os" axis is recognized — ` +
        'an axis nobody reads is a platform set nobody checks',
    );
    return null;
  }
  if (reference === null || reference[1] !== 'os') {
    errors.push(
      `${where} declares matrix.os but its runs-on is ${JSON.stringify(job.runsOn)} — a matrix that runs-on ` +
        'does not consume is a declaration nothing acts on',
    );
    return null;
  }
  const labels = matrix.os;
  if (!Array.isArray(labels) || labels.length === 0
    || labels.some(value => typeof value !== 'string' || value.trim() === '')) {
    errors.push(
      `${where} declares matrix.os which is not a non-empty list of runner labels — an empty platform set ` +
        'makes every reachability assertion vacuous',
    );
    return null;
  }
  const trimmed = labels.map(value => value.trim());
  for (const label of trimmed) {
    if (osFamilyOf(label, errors, where) === null) return null;
  }
  return trimmed;
}

/// `if:` 的形状。`''` ⇒ 无条件（不是一种「形状」）；未归类形状 ⇒ `'unknown'`（由调用方具名）。
export function conditionShapeOf(condition) {
  if (condition === '') return '';
  if (/^runner\.os\s*==\s*'([^']*)'$/.test(condition)) return 'platform-os';
  if (/^failure\(\s*\)$/.test(condition)) return 'failure';
  return 'unknown';
}

/// 一个步的**执行平台腿**（`platformsOf` 的结果里，满足 `if:` 的那一部分）。
///
/// 集合级：`if: runner.os == 'macOS'` 在「矩阵里没有 macOS」时返回**空列表**——这正是
/// 「一个不跑的判定」，而不是「一个写了条件的判定」。不可求值 ⇒ 具名失败并返回 `null`。
export function platformLegsOf(step, platforms, errors = [], where = 'the workflow') {
  const condition = step.condition ?? '';
  const shape = conditionShapeOf(condition);
  if (shape === '') return [...platforms];
  if (shape === 'failure') return [...platforms];
  if (shape === 'platform-os') {
    const value = condition.match(/^runner\.os\s*==\s*'([^']*)'$/)[1];
    if (!RUNNER_OS_VALUES.includes(value)) {
      errors.push(
        `${where} step ${JSON.stringify(step.identity)} compares runner.os to ${JSON.stringify(value)}, which is ` +
          `not a runner OS value (${RUNNER_OS_VALUES.join(', ')}) — the condition can never hold`,
      );
      return null;
    }
    return platforms.filter(label => osFamilyOf(label) === value);
  }
  errors.push(
    `${where} step ${JSON.stringify(step.identity)} carries the condition ${JSON.stringify(condition)}, which is ` +
      `not one of the recognized condition shapes (${CONDITION_SHAPES.join(', ')}) or none — a condition ` +
      'nobody can evaluate is a condition nobody checks',
  );
  return null;
}

/// `continue-on-error` 的真假。只认布尔字面量或「没写」；其余（如 `${{ … }}`）⇒ 具名失败并返回 `null`。
export function continueOnErrorOf(step, errors = [], where = 'the workflow') {
  const raw = step.continueOnError;
  if (raw === undefined || raw === null || raw === false) return false;
  if (raw === true) return true;
  errors.push(
    `${where} step ${JSON.stringify(step.identity)} carries continue-on-error ${JSON.stringify(raw)}, which is ` +
      'not a boolean literal — a failure switch nobody can evaluate is a switch nobody checks',
  );
  return null;
}

/// **主判据**。入参显式注入，**缺任一项即具名报错**（与 `reachabilityErrors` 同纪律——
/// 一条会因入参缺失而恒真的断言比没有断言更坏）。
///
/// `deps`：`{host, workflowText, registered?}`。`host` 是 workflow 文件的仓库路径
/// （在册条目用它归属）；`workflowText` 是该文件正文；`registered` 默认 `WORKFLOW_STEPS`。
export function stepConfigErrors(deps = {}) {
  const {host, workflowText} = deps;
  const registered = deps.registered ?? WORKFLOW_STEPS;
  const errors = [];

  if (typeof host !== 'string' || host === '') {
    errors.push('step configuration requires the workflow host path and it was not provided');
  }
  if (typeof workflowText !== 'string') {
    errors.push('step configuration requires the workflow text and it was not provided');
  }
  if (!Array.isArray(registered) || registered.length === 0) {
    // 空注册表 ⇒ 下面的每一条规则都无对象可判。MUST NOT 让「没有在册步」安静地全绿。
    errors.push(
      'step configuration requires a non-empty registered step set — an empty registry makes every ' +
        'configuration rule vacuously true',
    );
  }
  if (errors.length > 0) return errors;

  let jobs;
  try {
    jobs = workflowJobsOf(workflowText);
  } catch (error) {
    // 不可解析 ⇒ 具名失败。MUST NOT 静默返回空错误（那会把「读不懂」伪装成「没问题」）。
    errors.push(`${host} could not be parsed, so its step configuration cannot be judged: ${error.message}`);
    return errors;
  }

  const entries = registered.filter(entry => entry.host === host);
  const registeredIdentities = new Set(entries.map(entry => entry.identity));
  const usedShapes = new Set();
  const usedCommandShapes = new Set();
  let judged = 0;

  for (const job of jobs) {
    const where = `${host} job ${JSON.stringify(job.id)}`;
    const platforms = platformsOf(job, errors, where);
    const families = platforms === null ? null : new Set(platforms.map(label => osFamilyOf(label)));
    for (const step of job.steps) {
      // 不在册的步由 W95 的闭世界具名，这里不重复判决（**判定域各自的**，见 ADR §边界）。
      if (step.identity === '' || !registeredIdentities.has(step.identity)) continue;
      const entry = entries.find(item => item.identity === step.identity);
      judged += 1;
      // 形状的**实例计数与矩阵无关**：条件与命令都能单独读。先记，再判平台——
      // 否则一个不可读的矩阵会把「形状表非空洞」这条也连带打红（**级联噪声**），
      // 让人分不清「矩阵读不了」与「表格里没有实例」。
      const shape = conditionShapeOf(step.condition ?? '');
      if (shape !== '' && shape !== 'unknown') usedShapes.add(shape);
      const matchedCommands = PLATFORM_COMMAND_SHAPES.filter(command => command.pattern.test(step.run));
      for (const command of matchedCommands) usedCommandShapes.add(command.id);
      if (platforms === null) continue;

      const legs = platformLegsOf(step, platforms, errors, where);
      const swallowed = continueOnErrorOf(step, errors, where);
      if (legs === null || swallowed === null) continue;
      const legFamilies = new Set(legs.map(label => osFamilyOf(label)));

      // R1 可达性：一个**永不跑**的判定是一个**静默消失**的判定。
      if (legs.length === 0) {
        errors.push(
          `${where} step ${JSON.stringify(step.identity)} has an empty execution platform set ` +
            `(condition ${JSON.stringify(step.condition ?? '')} vs platforms ${JSON.stringify(platforms)}) — ` +
            'a judgement that never runs is a judgement that silently disappeared',
        );
      }

      // R2 失败语义与角色一致：判定不可被吞，诊断不可替换结论。
      const judgedKind = entry.kind === 'local' || entry.kind === 'ci-only';
      if (judgedKind && swallowed) {
        errors.push(
          `${where} step ${JSON.stringify(step.identity)} is ${entry.kind} yet can fail without failing the job ` +
            '(continue-on-error) — a judgement that can be swallowed is a decoration, not a judgement',
        );
      }
      if (entry.kind === 'diagnostic' && !swallowed) {
        errors.push(
          `${where} step ${JSON.stringify(step.identity)} is a diagnostic step without continue-on-error — a ` +
            'diagnostic must not replace the verdict it translates',
        );
      }

      // R3 成因与条件一致（**双向**）。
      if (entry.kind === 'ci-only' && legs.length > 0) {
        const restricted = families !== null && legFamilies.size < families.size;
        if (entry.reason === 'platform-toolchain') {
          if (!restricted) {
            errors.push(
              `${where} step ${JSON.stringify(step.identity)} is ci-only because it needs a platform toolchain, ` +
                `but its execution platform set ${JSON.stringify([...legFamilies])} is not a proper subset of the ` +
                `job platforms ${JSON.stringify([...families])} — the cause is not paid for by a platform condition`,
            );
          }
        } else if (CI_ONLY_REASONS.includes(entry.reason) && restricted) {
          errors.push(
            `${where} step ${JSON.stringify(step.identity)} is ci-only because of ${JSON.stringify(entry.reason)}, ` +
              `which says nothing about the platform, yet it is restricted to ${JSON.stringify([...legFamilies])} ` +
              '— the restriction makes it silently skip the other platforms',
          );
        }
      }

      // R5 平台专属命令 MUST 被平台条件兑付。
      for (const command of matchedCommands) {
        if (legs.length === 0) continue;
        const unavailable = [...legFamilies].filter(os => !command.supported.includes(os));
        if (unavailable.length > 0) {
          errors.push(
            `${where} step ${JSON.stringify(step.identity)} runs a ${command.id} command (supported on ` +
              `${JSON.stringify(command.supported)}) but its execution platform set includes ` +
              `${JSON.stringify(unavailable)} — the command is not available there`,
          );
        }
      }
    }
  }

  // **非空洞**：一个都没判到 ⇒ 每一条规则都无对象可判（「没有实例的规则恒真」）。
  if (judged === 0) {
    errors.push(
      `${host} carries no registered step, so no execution configuration was judged — an empty judged set makes ` +
        'every configuration rule vacuously true',
    );
  }
  // **非空洞**：规则表的每一格 MUST 至少有一个实例。
  for (const shape of CONDITION_SHAPES) {
    if (!usedShapes.has(shape)) {
      errors.push(
        `${host} carries no step with a ${JSON.stringify(shape)} condition — a condition shape with no instance ` +
          'makes its rule vacuously true',
      );
    }
  }
  for (const command of PLATFORM_COMMAND_SHAPES) {
    if (!usedCommandShapes.has(command.id)) {
      errors.push(
        `${host} carries no step whose command matches ${JSON.stringify(command.id)} — a platform-command shape ` +
          'with no instance makes its rule vacuously true',
      );
    }
  }

  return errors;
}
