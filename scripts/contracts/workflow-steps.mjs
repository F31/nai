import YAML from 'yaml';
import {reasonPaymentErrors} from './cause-payment.mjs';

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
/// （`pnpm spec:validate`）与「工具链前提超出本机门控」（`schema --check` 要 protoc）
/// **风险不同、成因不同**，运行时探测区分不出这两者。⇒ 分类是人的主张，逐条在册；
/// 判据负责验「主张有没有付代价」——W108 起，这后半句是一条**真的**判据（`CI_ONLY_REASON_PAYMENTS`）。

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
/// 四个取值按**实测**区分——「只有 CI 跑」有四种不同的成因，混成一个会让理由不可读：
/// - `compiled-artifact`：需要编译产物（`descriptor.test.mjs` 要 protoc + 生成物）；
/// - `platform-toolchain`：需要特定平台工具链（`--suite contracts` 要 Linux C++）；
/// - `not-in-profile`：本机**能**跑，但没接进 `check` profile（`spec:validate`／`openspec --archived`）；
/// - `ci-produced-artifact`：本机**能**跑，但它判定的**产物只由 CI 的前序步产生**
///   （W130：门控日志由写方 `tee` 落进 `.nai-diagnostics/`，由 `xtask/src/secrets.rs` 的
///   `ARTIFACTS` 登记表指出扫描域）。本机跑它是**空洞的**——扫描域为空，报「通过」等于什么都没说。
///   它与 `not-in-profile` 的区别就是**实测出来的**：后者删掉该步，那条判定**本机本来也没有**；
///   前者删掉该步，判定的**对象**根本不会出现在任何地方。
///
/// ⚠️ 上面每一点都 MUST 在 `CI_ONLY_REASON_PAYMENTS` 里有**凭据**。原有的第四个取值
/// `host-subprocess` 已**逐出**（W108 / ADR 0086）：它是**宿主/机器事实**，而宿主瞬态 MUST NOT
/// 成为判定输入（铁律二）⇒ **它不可能有凭据**——它是这条规则里一个**永久无法兑付的空格**。
/// 逐出的理由**不引用任何发生过的瞬态**：即使本机从未跑通那条命令，结论相同。
export const CI_ONLY_REASONS = [
  'compiled-artifact',
  'platform-toolchain',
  'not-in-profile',
  'ci-produced-artifact',
];

/// **成因兑付规格**（W108）：每条 `ci-only` 成因 MUST 付代价——给出一条**可从仓库自证**的凭据。
///
/// 形状：`{evidence: [{file, marker, why}, …]}`，每格**至少一条**。凭据的 `marker` MUST 在该
/// `file` 的内容里**逐字存在**，`file` MUST 已被 git 跟踪，且 MUST NOT 落在 `scripts/contracts/`
/// 之下（被审注册表自己写的凭据对表恒真）。`why` MUST 非空。
///
/// ⚠️ **为什么 `platform-toolchain` 的凭据指向执行体而不是 workflow 自身**：后者是**被审宿主**里的
/// 另一个步，用它做凭据会让两个在册条目互为证据；指向执行体（`xtask/src/schema.rs` 里那条
/// `cmake -S tests/contracts/cpp`）才是真正的外部兑付。
///
/// ⚠️ **为什么 `host-subprocess` 不在这张表里**：它**不可能**有凭据——它的成因是**宿主/机器事实**，
/// 而宿主瞬态 MUST NOT 成为判定输入（铁律二）⇒ 它是这条规则里一个**永久无法兑付的空格**，
/// 因此从成因闭集**逐出**（W108 / ADR 0086）。逐出的理由**不引用任何发生过的瞬态**。
///
/// ⚠️ **同一执行体在两层的成因可以不同**（如实披露，不改变）：`test --suite contracts` 在**命令层**
/// 归 `requires-compiled-artifact`（依赖 protoc），在 **step 层**归 `platform-toolchain`（CI 上要
/// `Install Linux C++ protobuf runtime` 那一步）。两层回答的是「谁的范围」（命令面 / workflow 步面）。
export const CI_ONLY_REASON_PAYMENTS = {
  'compiled-artifact': {
    evidence: [
      {
        file: 'tests/contracts/descriptor.test.mjs',
        marker: 'needs protoc, so these belong to the contracts suite',
        why: '那个测试直接解码生成物（`target/contracts/descriptor.bin`）⇒ 它要 protoc 与编译产物，MUST NOT 进本机门控（同一段注释还写明「静态的那一半留在 schema.test.mjs」）',
      },
    ],
  },
  'platform-toolchain': {
    evidence: [
      {
        file: 'xtask/src/schema.rs',
        marker: '"tests/contracts/cpp"',
        why: '该套件要 `cmake` 构建 `tests/contracts/cpp` 里的 C++ 往返夹具 ⇒ 需要 Linux C++ 工具链；凭据指向**执行体**，不是 workflow 里那个装 protobuf 运行时的步（后者是被审宿主的另一条目）',
      },
    ],
  },
  'not-in-profile': {
    evidence: [
      {
        file: 'package.json',
        marker: '"spec:validate": "openspec validate --all --strict --no-interactive"',
        why: '该脚本**真实存在、能跑**——「不在 profile 里」说的是接入方式，不是「跑不了」',
      },
      {
        file: 'package.json',
        marker: '"@fission-ai/openspec"',
        why: '第二条实例（`pnpm exec openspec validate --archived`）用的 CLI 装在依赖里 ⇒ 它同样是一条能跑的判定',
      },
    ],
  },
  'ci-produced-artifact': {
    evidence: [
      {
        file: 'scripts/ci-diagnose.mjs',
        marker: 'Usage: node scripts/ci-diagnose.mjs <log-path>',
        why: '门控日志在这个仓库里的**唯一消费者**把日志当**入参**（`<log-path>`）、自己不生产它 ⇒ 产物由别处产生，本机构建不是它的来源。凭据刻意指向**消费者**而不是 workflow 里那条 `tee`：后者是被审宿主里的另一个在册条目，用它做凭据会让两个在册条目互为证据（W108 同一条理由）',
      },
      {
        file: 'xtask/src/secrets.rs',
        marker: 'producer: ".github/workflows/foundation.yml"',
        why: '扫描器自己的**产物登记表**点明该产物的生产者是 CI workflow；而那条 `tee` 行由同一个文件里的 `every_registered_artifact_names_the_repository_text_that_produces_it` 逐字核验 ⇒ 这条凭据不是一句空声明，是一条被另一条判据守着的声明',
      },
    ],
  },
};

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
    // W108：原为 `host-subprocess`。该成因是**宿主事实**（能不能派生子进程），宿主瞬态 MUST NOT
    // 成为判定输入（铁律二），因此它**不可能有凭据**、已从闭集逐出。此处按**今日实测的**事实
    // 重归类：这条命令要 `protoc` 3.21.12（`xtask/src/schema.rs:207-217` 硬校验），与
    // `Validate schemas and API contract` 同因。⚠️ 这个理由**不随宿主变化**。
    reason: 'compiled-artifact',
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
    // W130：诊断产物的凭据扫描。本机**能**跑这条命令（`secrets::check(root)` 就在
    // `["check", "--secrets"]` 那条 arm 里），但它判定的**产物**只由前序步的 `tee` 产生
    // ⇒ 本机跑它是空洞的：扫描域为空，报「通过」等于什么都没说。
    identity: 'Scan the diagnostic artifacts for credential material (W130)',
    kind: 'ci-only',
    reason: 'ci-produced-artifact',
    target: [{shape: 'xtask', value: 'check --secrets'}],
  },
  {
    host: '.github/workflows/foundation.yml',
    // W134：这里原有 7 条 `test --suite …` 步（storage / kernel / hosts / session / scheduler /
    // provider / knowledge）。它们的包集全部包含在 `check --profile full` 里
    // （`engineering_checks` 的 `cargo test --workspace --exclude <harness>`）⇒ 每个平台腿上
    // 都在跑第二遍同一批测试。删掉它们**不损失判定**，覆盖仍由门控步提供；
    // 而这个关系现在由 `tests/contracts/step-overlap.test.mjs` 判着（两两覆盖包含 ⇒ 红）。
    // ⚠️ 这条注释是**记录**，不是判据：真正的守卫在 `scripts/contracts/step-overlap.mjs`。
    //
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
/// 少了它，「本机也跑」的凭据会退化成恒真）；`reasonInput` 是 `{sources: Map<path, text>,
/// tracked: Set<string>}`（`ci-only` 的**成因凭据**所指文件的内容与跟踪状态，显式注入——
/// 少了它，「这条成因凭什么成立」同样会退化成恒真）。可选带 `payments`：**兑付表本身**的注入位，
/// 缺省用 `CI_ONLY_REASON_PAYMENTS`——没有它，「表少一格 / 多一格 / 凭据指错」这几种缺陷
/// **无法被测试构造**（守卫都要注入验证）。
export function workflowStepErrors(
  host,
  text,
  registered = WORKFLOW_STEPS,
  gateSources = new Map(),
  reasonInput,
) {
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

  // ---------- 成因兑付（W108）：每条 `ci-only` 成因 MUST 付代价——一条可从仓库自证的凭据，
  // 或从闭集逐出。成因格 MUST ≥1 实例（没有实例的规则对空集恒真）。
  // ⚠️ 「格非空洞」按**整张注册表**算，不按本 host 的 `entries` 算：成因闭集是**全局**的，
  // 实例也应全局非空洞。将来若有多个 workflow 文件，同一句话会在每个 host 上各报一次——
  // 宁可重复，不可漏报（按 host 算会让「另一个文件里有实例」变成假阳性）。
  errors.push(
    ...reasonPaymentErrors({
      label: host,
      where: 'step',
      closedSet: CI_ONLY_REASONS,
      payments: (reasonInput ?? {}).payments ?? CI_ONLY_REASON_PAYMENTS,
      reasons: registered.map(entry => entry.reason).filter(reason => typeof reason === 'string'),
      sources: (reasonInput ?? {}).sources,
      tracked: (reasonInput ?? {}).tracked,
    }),
  );

  return errors;
}
