import YAML from 'yaml';

/// CI 工作流的**文件层 + 步层**闭世界。
///
/// 为什么需要它：`cargo xtask check --profile full`（本机门控）与
/// `.github/workflows/foundation.yml`（CI）是判定的两个执行入口，而**后者多跑**若干判定步
/// ——`schema --check`、8 个 `cargo xtask test --suite …`（含 Linux-only 的 `contracts` 套件）、
/// 含 `descriptor.test.mjs` 的契约步、`pnpm spec:validate`、`openspec validate --archived`。
/// 这个集合**没有任何在册台账**：它只出现在注释里（`tests/contracts/schema.test.mjs` 里那句
/// 「本地门控从不读它」是一句**解释**，不是判据）。
///
/// 实测（W95 两个探针，改前/改后都真跑）：
/// - 往 workflow 插一个 step ⇒ 本机 3 套件（schema 30 / platform 22 / registry 52）**全绿、零信号**；
/// - 删掉 `schema --check` 这个判定步 ⇒ **同样全绿**——一个判定**静默消失**。
///
/// 与 W84（测试文件的可达性）、W86（OpenAPI 文档集）同形，**新的一层对象**：workflow 的 step。
/// 三条纪律照抄：
/// 1. **步的身份由解析得出，MUST NOT 另行声明**（W80）：身份从 YAML 里算，不手写第二份清单；
/// 2. **「已声明」≠「已执行」**（W75）：`local` 声称的门控落点 MUST 在门控源里**逐字存在**；
/// 3. **闭世界 MUST 在每个层级上成立**（W82）：文件层之上还有步层。
///
/// ⚠️ **分类是静态声明，MUST NOT 由运行时探测得出**（铁律二）：`EBUSY` 是**宿主事实**，
/// 换一台机器会对同一个仓库得出不同分类 ⇒ 判据不再可复现。而且「能跑但没接进门控」
/// （`pnpm spec:validate`）与「根本跑不了」（`schema --check`）**风险不同、成因不同**，
/// 运行时探测区分不出这两者。⇒ 分类是人的主张，逐条在册；判据负责验「主张有没有付代价」。

/// workflow 文件住的目录。文件层闭世界按它取「已跟踪的 workflow 文件」。
export const WORKFLOW_DIR = '.github/workflows';

/// **文件层在册**：这些 workflow 文件 MUST 已被跟踪，且目录下已跟踪的 workflow 文件 MUST 全部在册。
/// 逐字写死：由「仓库里现有文件」派生的「现有集合 == 清单」是恒真式。
export const WORKFLOW_FILES = ['.github/workflows/foundation.yml'];

/// **步的类别**。闭集，理由与 `DISPOSITIONS`／`TEST_TIERS` 一致：让「这一步在哪儿跑」可判定。
/// - `infra`：环境/工具链准备，**不产生判定结论**；
/// - `local`：本机门控（`check --profile full`）**同样执行**它的判定面；
/// - `ci-only`：**只有 CI** 执行——删掉它，判定面**静默消失**；
/// - `diagnostic`：仅在失败时跑的诊断通道，**不产生独立判定**。
export const STEP_KINDS = ['infra', 'local', 'ci-only', 'diagnostic'];

/// `ci-only` 的**成因闭集**。域外取值 ⇒ 具名失败（W90：一个未知成因是一个标签，不是一个理由）。
/// 四个取值按**实测**区分——「跑不了」有四种不同的成因，混成一个会让理由不可读：
/// - `host-subprocess`：需要派生子进程，宿主对该步 `EBUSY`（`schema --check`）；
/// - `compiled-artifact`：需要编译产物（`descriptor.test.mjs` 要 protoc + 生成物）；
/// - `platform-toolchain`：需要特定平台工具链（`--suite contracts` 要 Linux C++）；
/// - `not-in-profile`：本机**能**跑，但没接进 `check` profile（`spec:validate`／`openspec --archived`）。
export const CI_ONLY_REASONS = [
  'host-subprocess',
  'compiled-artifact',
  'platform-toolchain',
  'not-in-profile',
];

/// **看起来像判定**的命令形状。`infra` 的 `run` MUST NOT 命中任何一条——否则「把判定步骤标成
/// 环境准备」就成了绕过整套检查的后门（**自我满足**的典型形态）。
///
/// 它验的是**命令形状**，不是语义（如实披露）：把一条环境准备误报成判定 ⇒ **红** ⇒ 要求显式处置。
/// 假阳性方向，安全。
export const JUDGE_COMMAND_SHAPES = [
  /^cargo\s+xtask\s/,
  /^node\s+--test\s/,
  /^node\s+--experimental-strip-types\s/,
  /^pnpm\s+exec\s/,
  /^pnpm\s+(?!install\b)/,
];

/// **步层在册**：一条「某个 step 属于哪一类」的独立声明。每条 `{host, identity, kind, …}`：
/// - `identity`：步的**身份**，由解析得出（有 `name` 用 `name`，否则 `uses` 或 `run` 首行）。逐字；
/// - `kind`：∈ `STEP_KINDS`；
/// - `target`：**这一步跑的到底是哪个目标**（W101）。`local` / `ci-only`（**产生判定**）MUST 带，
///   形状 ∈ `TARGET_SHAPES`，且 MUST 与从该步 `run` **解析**出的目标集合双向相等；
///   `infra` / `diagnostic` MUST NOT 带。**身份是名字，不是目标**——这一项才是目标；
/// - `local` MUST 带 `localEvidence: {file, marker}`——「本机也跑」的**凭据**；
/// - `ci-only` MUST 带 `reason`（∈ `CI_ONLY_REASONS`），且 MUST NOT 带 `localEvidence`。
///
/// **为什么 `target` 住在这里而不是另开一张表**（W101 决定 1）：一个 step 是**一个对象**，
/// 身份／类别／目标／凭据都是它的属性。另开一张 `step → target` 的表就是**同一件事的第二处
/// 定义**——两张表会各自漏掉对方的新增，而闭世界（未在册 / 已消失）已经长在本表上。
export const WORKFLOW_STEPS = [
  {host: '.github/workflows/foundation.yml', identity: 'actions/checkout@v4', kind: 'infra'},
  {host: '.github/workflows/foundation.yml', identity: 'actions/setup-node@v4', kind: 'infra'},
  {host: '.github/workflows/foundation.yml', identity: 'pnpm/action-setup@v4', kind: 'infra'},
  {host: '.github/workflows/foundation.yml', identity: 'Install pinned Rust', kind: 'infra'},
  {
    host: '.github/workflows/foundation.yml',
    identity: 'Install frontend and OpenSpec dependencies',
    kind: 'infra',
  },
  {
    host: '.github/workflows/foundation.yml',
    identity: 'Install protoc (upstream v21.12 is libprotoc 3.21.12)',
    kind: 'infra',
  },
  {
    host: '.github/workflows/foundation.yml',
    identity: 'Install Linux C++ protobuf runtime',
    kind: 'infra',
  },
  {
    host: '.github/workflows/foundation.yml',
    identity: 'Check generated contracts without modifying consumers',
    kind: 'ci-only',
    reason: 'host-subprocess',
    target: [{shape: 'xtask', value: 'schema --check'}],
  },
  {
    host: '.github/workflows/foundation.yml',
    identity: 'Complete engineering foundation (portable steps + frontend workspace)',
    kind: 'local',
    localEvidence: {file: 'xtask/src/lib.rs', marker: '["check", "--profile", "full"]'},
    target: [{shape: 'xtask', value: 'check --profile full'}],
  },
  {
    host: '.github/workflows/foundation.yml',
    identity: 'Publish the gate failure summary as annotations',
    kind: 'diagnostic',
  },
  {
    host: '.github/workflows/foundation.yml',
    identity: 'Storage and credential boundary (Windows native vault only on Windows)',
    kind: 'local',
    localEvidence: {
      file: 'xtask/src/lib.rs',
      marker: '["test", "--workspace", "--locked", "--exclude", HARNESS]',
    },
    target: [{shape: 'xtask', value: 'test --suite storage'}],
  },
  {
    host: '.github/workflows/foundation.yml',
    identity: 'Kernel registration and binding',
    kind: 'local',
    localEvidence: {
      file: 'xtask/src/lib.rs',
      marker: '["test", "--workspace", "--locked", "--exclude", HARNESS]',
    },
    target: [{shape: 'xtask', value: 'test --suite kernel'}],
  },
  {
    host: '.github/workflows/foundation.yml',
    identity: 'Plugin hosts and IPC',
    kind: 'local',
    localEvidence: {
      file: 'xtask/src/lib.rs',
      marker: '["test", "--workspace", "--locked", "--exclude", HARNESS]',
    },
    target: [{shape: 'xtask', value: 'test --suite hosts'}],
  },
  {
    host: '.github/workflows/foundation.yml',
    identity: 'Session leases and independent cancellation',
    kind: 'local',
    localEvidence: {
      file: 'xtask/src/lib.rs',
      marker: '["test", "--workspace", "--locked", "--exclude", HARNESS]',
    },
    target: [{shape: 'xtask', value: 'test --suite session'}],
  },
  {
    host: '.github/workflows/foundation.yml',
    identity: 'Scheduling and shared budget ledger',
    kind: 'local',
    localEvidence: {
      file: 'xtask/src/lib.rs',
      marker: '["test", "--workspace", "--locked", "--exclude", HARNESS]',
    },
    target: [{shape: 'xtask', value: 'test --suite scheduler'}],
  },
  {
    host: '.github/workflows/foundation.yml',
    identity: 'API Provider transport and retry (mock server; no paid calls)',
    kind: 'local',
    localEvidence: {
      file: 'xtask/src/lib.rs',
      marker: '["test", "--workspace", "--locked", "--exclude", HARNESS]',
    },
    target: [{shape: 'xtask', value: 'test --suite provider'}],
  },
  {
    host: '.github/workflows/foundation.yml',
    identity: 'Knowledge algorithms, remote mock transport and local persistence',
    kind: 'local',
    localEvidence: {
      file: 'xtask/src/lib.rs',
      marker: '["test", "--workspace", "--locked", "--exclude", HARNESS]',
    },
    target: [{shape: 'xtask', value: 'test --suite knowledge'}],
  },
  {
    host: '.github/workflows/foundation.yml',
    // 混合步：`schema.test.mjs` 本机跑，`descriptor.test.mjs` 只有 CI 跑（要编译产物）。
    // 只要有**未被本机覆盖的成分**就归 `ci-only`——保守方向，安全（W92：假阳性与假阴性不对称）。
    identity: 'Validate schemas and API contract',
    kind: 'ci-only',
    reason: 'compiled-artifact',
    target: [
      {shape: 'node-test', value: '--test tests/contracts/schema.test.mjs tests/contracts/descriptor.test.mjs'},
    ],
  },
  {
    host: '.github/workflows/foundation.yml',
    identity: 'Full three-language contract suite (Linux C++ toolchain)',
    kind: 'ci-only',
    reason: 'platform-toolchain',
    target: [{shape: 'xtask', value: 'test --suite contracts'}],
  },
  {
    host: '.github/workflows/foundation.yml',
    identity: 'pnpm spec:validate',
    kind: 'ci-only',
    reason: 'not-in-profile',
    target: [{shape: 'pnpm', value: 'spec:validate'}],
  },
  {
    host: '.github/workflows/foundation.yml',
    identity: 'pnpm exec openspec validate --archived --no-interactive',
    kind: 'ci-only',
    reason: 'not-in-profile',
    target: [{shape: 'pnpm', value: 'exec openspec validate --archived --no-interactive'}],
  },
];

/// 一个 step 的**身份**。判定单元 MUST 是身份，MUST NOT 是名字（W91：「同名」≠「同一个」）。
/// 优先 `name`，其次 `uses`，最后 `run` 的首行——三条都没有 ⇒ 返回空串（调用方具名报错）。
export function stepIdentity(step) {
  if (typeof step?.name === 'string' && step.name.trim() !== '') return step.name.trim();
  if (typeof step?.uses === 'string' && step.uses.trim() !== '') return step.uses.trim();
  if (typeof step?.run === 'string' && step.run.trim() !== '') {
    return step.run.trim().split('\n')[0].trim();
  }
  return '';
}

/// 解析一份 workflow 的**全部 job**。产出 `{id, matrix, runsOn, steps}`，其中每个 step 是
/// `{identity, condition, run, continueOnError}`。
///
/// **为什么是 job 视图**（W100）：`matrix` 是 **job 的属性**，不是 step 的；而「这一步会不会跑」
/// 只有在「矩阵的腿 × 这一步的条件」上才可判。W95 的扁平视图丢掉了 job 这一层，
/// 于是执行配置面**根本进不了判定域**。
///
/// 不可解析 ⇒ 抛（由调用方转成一条具名错误，**不静默降级**）。
export function workflowJobsOf(text) {
  const doc = YAML.parse(text);
  const out = [];
  for (const [id, job] of Object.entries(doc?.jobs ?? {})) {
    const steps = [];
    for (const step of job?.steps ?? []) {
      steps.push({
        identity: stepIdentity(step),
        condition: typeof step?.if === 'string' ? step.if.trim() : '',
        run: typeof step?.run === 'string' ? step.run : '',
        // `continue-on-error` 原样带出（**不**在这里解释真假：形状的闭世界属于 W100 的判据面）。
        continueOnError: step?.['continue-on-error'],
      });
    }
    out.push({
      id,
      matrix: job?.strategy?.matrix,
      runsOn: typeof job?.['runs-on'] === 'string' ? job['runs-on'].trim() : '',
      steps,
    });
  }
  return out;
}

/// 解析一份 workflow 的全部 step（**跨 job**）。产出 `{identity, condition, run, matrix, continueOnError}`。
///
/// W95 既有的 `{identity, condition, run}` 语义**一字不动**；新字段是**增补**，
/// 因此 `workflowStepErrors` 的既有用例全部不变。`matrix` 与同 job 的步**共享同一份对象**
/// ——拷贝会让「同一个矩阵」变成若干份可分别篡改的副本，「同一件事 MUST NOT 两处定义」。
///
/// 两个视图来自**同一次** `YAML.parse`：单一解析器，MUST NOT 让第二份解析器去追第一份的漂移。
export function workflowStepsOf(text) {
  const out = [];
  for (const job of workflowJobsOf(text)) {
    for (const step of job.steps) {
      out.push({...step, job: job.id, matrix: job.matrix});
    }
  }
  return out;
}

/// **文件层**闭世界：`declared` 的每份 MUST 已被跟踪，且目录下已跟踪的 workflow 文件 MUST 全部在册。
export function workflowFileErrors(tracked, declared = WORKFLOW_FILES) {
  const errors = [];
  const registry = new Set(declared);
  const onDisk = [...tracked].filter(file => file.startsWith(`${WORKFLOW_DIR}/`));
  for (const file of declared) {
    if (!tracked.has(file)) {
      errors.push(`${file} is declared as a workflow but is not tracked by this repository`);
    }
  }
  for (const file of onDisk) {
    if (!registry.has(file)) {
      errors.push(`${file} is a tracked workflow but nobody declares it — a workflow that is not in the registry is not judged`);
    }
  }
  return errors;
}

/// **步层**判据。`registered` 为在册条目，`gateSources` 是 `Map<path, text>`（门控源，显式注入——
/// 少了它，「本机也跑」的凭据会退化成恒真）。
export function workflowStepErrors(host, text, registered = WORKFLOW_STEPS, gateSources = new Map()) {
  const errors = [];
  let parsed;
  try {
    parsed = workflowStepsOf(text);
  } catch (error) {
    // 不可解析 ⇒ 具名失败。MUST NOT 静默返回空错误（那会把「读不懂」伪装成「没问题」）。
    errors.push(`${host} could not be parsed, so its steps cannot be judged: ${error.message}`);
    return errors;
  }

  const entries = registered.filter(entry => entry.host === host);
  const registeredIdentities = new Set(entries.map(entry => entry.identity));
  const seen = new Map();
  for (const step of parsed) {
    if (step.identity === '') {
      errors.push(`${host} carries a step with no name, uses or run — a step must have an identity`);
      continue;
    }
    if (seen.has(step.identity)) {
      // 身份 MUST 唯一：两个同名步会让「在册」无法指向确定的那个（W91）。
      errors.push(`${host} carries the step identity "${step.identity}" twice — an identity must be unique`);
    }
    seen.set(step.identity, step);
  }
  if (parsed.length === 0) {
    errors.push(`${host} declares no steps — an empty step set makes the closure vacuously true`);
  }

  // 闭世界，两个方向都被点名：
  // ① workflow 里的步不在册 ⇒ 新增/改名的步**没有任何处置**（这正是 W95 的洞）；
  // ② 在册的步不在 workflow 里 ⇒ 一个判定**静默消失**（删步无人发现）。
  for (const step of parsed) {
    if (step.identity !== '' && !registeredIdentities.has(step.identity)) {
      errors.push(`${host} carries the step "${step.identity}", which nobody declares — an unregistered step is not judged`);
    }
  }
  for (const entry of entries) {
    if (!seen.has(entry.identity)) {
      errors.push(`${host} is declared to carry the step "${entry.identity}", but it does not — a registered step that vanished is a judgement that silently disappeared`);
    }
  }

  const gate = new Map(gateSources);
  for (const entry of entries) {
    if (!STEP_KINDS.includes(entry.kind)) {
      errors.push(`${host} entry "${entry.identity}" names kind "${entry.kind}", which is not a step kind — a kind outside the closed set is a label, not a classification`);
      continue;
    }
    const step = seen.get(entry.identity);
    if (entry.kind === 'local') {
      // `local` 必须付代价：引出一处**真的存在**的门控落点（W75「已声明 ≠ 已执行」）。
      const evidence = entry.localEvidence;
      if (!evidence || typeof evidence.file !== 'string' || typeof evidence.marker !== 'string' || evidence.marker === '') {
        errors.push(`${host} entry "${entry.identity}" claims local coverage but names no gate site — "the local gate runs it too" has to pay for itself`);
        continue;
      }
      if (!gate.has(evidence.file)) {
        // 缺输入 MUST 具名，MUST NOT 静默跳过——那会让判据在被削弱时保持沉默。
        errors.push(`${host} entry "${entry.identity}" cites gate source ${evidence.file}, which was not provided for judgement`);
        continue;
      }
      if (!gate.get(evidence.file).includes(evidence.marker)) {
        errors.push(`${host} entry "${entry.identity}" cites ${evidence.file} for "${evidence.marker}", which is not in that file — the evidence is a claim, not a fact`);
      }
    } else if (entry.kind === 'ci-only') {
      // `ci-only` 必须付代价：成因 MUST 落在闭集里。
      if (typeof entry.reason !== 'string' || !CI_ONLY_REASONS.includes(entry.reason)) {
        errors.push(`${host} entry "${entry.identity}" is ci-only with reason "${entry.reason}", which is not in the cause table — an unknown cause is a label, not a reason`);
      }
      // 双向：`ci-only` MUST NOT 同时声称本机覆盖（否则「它是不是本机也跑」没有单一回答）。
      if (entry.localEvidence !== undefined) {
        errors.push(`${host} entry "${entry.identity}" is ci-only yet also carries local evidence — a step cannot be covered locally and only-in-CI at once`);
      }
    } else if (entry.kind === 'diagnostic') {
      // 诊断通道 MUST 由**结构**（`if: failure…`）认出，不由名字认出。
      if (step && !step.condition.includes('failure')) {
        errors.push(`${host} entry "${entry.identity}" is a diagnostic step but its step carries no failure condition — a diagnostic that always runs is not a diagnostic`);
      }
    } else if (entry.kind === 'infra' && step) {
      // 反向：`infra` MUST NOT 是一条判定命令——否则「标成环境准备」就是绕过整套检查的后门。
      const firstLine = step.run.trim().split('\n')[0].trim();
      if (firstLine !== '' && JUDGE_COMMAND_SHAPES.some(shape => shape.test(firstLine))) {
        errors.push(`${host} entry "${entry.identity}" is infra but its command looks like a judgement — infra must not be a back door around the checks`);
      }
    }
  }
  return errors;
}
