import {ciXtaskCommands, stripShellSyntax} from './gate-coverage.mjs';
import {STEP_KINDS, WORKFLOW_STEPS, workflowStepsOf} from './workflow-steps.mjs';

/// 在册判定步的**目标绑定**：这一步跑的到底是哪个目标。
///
/// 为什么需要它：W95 给 workflow 的 step 集补了**步层闭世界**（身份由解析得出、与在册清单
/// 双向一致、`kind` 在闭集里、`local` 要引出真的门控落点）；W97 管住了包集合与「CI 调用哪些
/// 命令」；W100 管住了执行配置面（跑不跑／在哪儿跑／失败算不算数）。但**一条在册判定步跑的是
/// 哪个目标**从来没被读过——在册条目只声明 `{host, identity, kind}`，而 **`identity` 是名字，
/// 不是目标**。
///
/// 实测（W101 探针，改真文件、真跑 9 个 portable 套件、逐字节还原、`sha256` 逐字一致）：
/// - 把 `hosts` 步与 `session` 步的 `--suite` **互换** ⇒ `[0,…]` **零信号**；
/// - 把 `scheduler` 步改跑 `--suite provider`（scheduler 在 CI 消失、provider 跑两遍）⇒ **零信号**；
/// - 把 `knowledge` 步改跑 `--suite storage` ⇒ **零信号**；
/// - 把 provider 判定步**整条**换成 `cargo xtask check --secrets` ⇒ **零信号**。
///
/// ⚠️ **不能把「身份被顺带改掉」当成「目标被改」**：无 `name` 的步（`- run: pnpm spec:validate`）
/// 其身份就是 `run` 首行 ⇒ 改它的 `run` 会先触发 W95 的「未在册 + 已消失」。两层 MUST 分开测。
///
/// ⚠️ **探针的第一轮红大多是夹具锚点被扰动**：`workflow.test.mjs` 的「步消失」用例锚在
/// `- name: …\n        run: cargo xtask test --suite kernel\n` 这一整段上 ⇒ 一改那条命令，
/// `.replace()` 匹配不到，注入变 no-op，用例因**前提被扰动**而红。选探针样本时 MUST 避开
/// 被夹具锚定的步。
///
/// 三条纪律照抄：**「已声明」≠「已执行」**（W75）、**「判据的输入 MUST 等于被断言对象」**、
/// **「同一件事 MUST NOT 两处定义」**（xtask 那一支复用 W97 的 `ciXtaskCommands`）。

/// **目标形状闭集**。域外取值 ⇒ 具名失败（W90：一个未知形状是一个标签，不是一个分类）。
/// 三个取值由**实测**得出，各有活实例：`xtask` 10、`node-test` 1、`pnpm` 2。
///
/// 与 `WORKFLOW_STEPS.JUDGE_COMMAND_SHAPES` 的关系：那张表判「**这条命令像不像一条判定**」，
/// 本表判「**它的目标是什么**」。对象不同，两条纪律各自成立。
export const TARGET_SHAPES = ['xtask', 'node-test', 'pnpm'];

/// `node --test` 的目标：`node <可选旗标> --test <文件…>`。旗标留在值里——否则
/// `--experimental-strip-types --test x` 与 `--test x` 会塌成同一个目标。
const NODE_TEST = /\bnode\s+((?:--[\w-]+(?:=\S+)?\s+)*)--test\s+([^\n]+)/g;

/// `pnpm <脚本>` 的目标。**`pnpm install` 不算目标**——它是环境准备（`Install frontend and
/// OpenSpec dependencies` 是 `infra`）。与 `JUDGE_COMMAND_SHAPES` 的 `/^pnpm\s+(?!install\b)/`
/// 同一条纪律，但对象不同：那条问「像不像判定」，本条问「目标是什么」。
const PNPM = /\bpnpm\s+([^\n]+)/g;

/// 一条规范化目标的比较键。判定单元 MUST 是「**(形状, 取值)**」这个身份，
/// MUST NOT 是取值本身——`--suite kernel` 这样的串在别的形状里也可能出现。
export function targetKey(target) {
  return `${target?.shape}:${target?.value}`;
}

/// 把一段 `run` 拆成**独立的命令段**：`&&`、`||`、`;`、`|`、换行都是段界。
///
/// 为什么**必须**拆：`cargo xtask test --suite kernel && cargo xtask check --secrets` 里
/// **两条**命令都会被 shell 执行，而「一行取一条」的朴素做法只会看到第一条
/// （`stripShellSyntax` 把 `&&…` 整段剪掉）⇒ 第二条判定**静默逃出**目标绑定。
/// 那是**假阴性**方向，危险（W92：假阳性与假阴性不对称 ⇒ 永远选安全方向）。
///
/// W97 的 `ciXtaskCommands` **保留它自己的边界**（它的对象是「一段 CI 文本里被调用的命令」，
/// 本轮不动它一个字符）；`targetsOf` 在**调用它之前**先分段，于是每一段各自对账。
function segmentsOf(run) {
  return run
    .split(/\|\||&&|;|\||\n/)
    .map(segment => segment.trim())
    .filter(segment => segment !== '');
}

/// 把一段 `run` 文本**解析**成目标列表 `[{shape, value}]`。
///
/// 纯函数、只读文本：不执行任何命令、不读磁盘（铁律二：宿主瞬态 MUST NOT 改变结论）。
/// 三个形状**全部**会被解析出来（包括一条 `run` 里被 `&&` 串起来的第二、第三条命令）——
/// 多出来的会被「解析出而没声明」抓住，这是**有意的**：要求人显式归类。
///
/// `xtask` 那一支**调用** `ciXtaskCommands`：剥离 shell 语法的规则只有一处定义。
export function targetsOf(run) {
  if (typeof run !== 'string' || run.trim() === '') return [];
  const out = [];
  for (const segment of segmentsOf(run)) {
    for (const value of ciXtaskCommands(segment)) out.push({shape: 'xtask', value});
    for (const match of segment.matchAll(NODE_TEST)) {
      const value = `${match[1]}--test ${stripShellSyntax(match[2])}`.replace(/\s+/g, ' ').trim();
      if (value !== '--test') out.push({shape: 'node-test', value});
    }
    for (const match of segment.matchAll(PNPM)) {
      const value = stripShellSyntax(match[1]);
      if (value === '' || /^install\b/.test(value)) continue;
      out.push({shape: 'pnpm', value});
    }
  }
  return out;
}

/// 主判据。入参**显式注入**：`workflowText` 少了它，整条判据会退化成恒真。
///
/// 判定分派**由 `kind` 决定**，MUST NOT 写死步名（W88：改前/改后都要真跑）。
/// W95 已经报过的东西（未知 `kind`、已消失的步、重复身份）**不在这里重复报**——
/// 同一件事两处定义会让边界变糊。
export function stepTargetErrors({host, workflowText, registered = WORKFLOW_STEPS} = {}) {
  if (typeof workflowText !== 'string' || workflowText === '') {
    // 缺输入 MUST 具名，MUST NOT 静默返回空错误——那会让判据在被削弱时保持沉默。
    return ['the CI workflow was not provided, so no step target can be judged'];
  }
  if (!Array.isArray(registered) || registered.length === 0) {
    return ['the step registry is empty, so every target rule would be vacuously true'];
  }

  let parsed;
  try {
    parsed = workflowStepsOf(workflowText);
  } catch (error) {
    // 不可解析 ⇒ 具名失败。MUST NOT 静默降级（那会把「读不懂」伪装成「没问题」）。
    return [`${host} could not be parsed, so its step targets cannot be judged: ${error.message}`];
  }

  const byIdentity = new Map();
  for (const step of parsed) {
    if (step.identity !== '') byIdentity.set(step.identity, step);
  }

  const errors = [];
  for (const entry of registered) {
    if (entry?.host !== host) continue;
    if (!STEP_KINDS.includes(entry.kind)) continue;
    const step = byIdentity.get(entry.identity);
    if (step === undefined) continue;

    const produced = targetsOf(step.run);
    const producedKeys = new Set(produced.map(targetKey));
    const judging = entry.kind === 'local' || entry.kind === 'ci-only';

    if (!judging) {
      // 双向：`infra` / `diagnostic` MUST NOT 声明目标 **且** MUST NOT 解析出目标。
      // 单向（只禁声明）是装饰——真正的后门是「把一条判定塞进环境准备」。
      // W95 也有一条同目的检查，但它**只看首行**、只认 5 个正则；本条是**整段解析**，
      // 两者对象不同（那条问「像不像判定」，本条问「解析出的目标集合是不是空的」）。
      if (entry.target !== undefined) {
        errors.push(
          `${host} entry "${entry.identity}" is ${entry.kind} yet declares a target — a target belongs to a step that judges`,
        );
      }
      if (producedKeys.size > 0) {
        errors.push(
          `${host} entry "${entry.identity}" is ${entry.kind} but its command yields the target(s) ${[...producedKeys].sort().join(', ')} — placing a judgement in ${entry.kind} is a back door around the checks`,
        );
      }
      continue;
    }

    // ① 一个**产生判定**的步 MUST 声明它跑的目标。
    if (!Array.isArray(entry.target) || entry.target.length === 0) {
      errors.push(
        `${host} entry "${entry.identity}" claims judging coverage but names no target — "which target this step runs" has to be declared, a name is not a target`,
      );
      continue;
    }
    // ② 声明的形状 MUST 落在闭集里，值 MUST 非空。
    const declaredKeys = new Set();
    let shaped = true;
    for (const target of entry.target) {
      if (!TARGET_SHAPES.includes(target?.shape)) {
        errors.push(
          `${host} entry "${entry.identity}" declares the target shape "${target?.shape}", which is not a target shape — a shape outside the closed set is a label, not a classification`,
        );
        shaped = false;
        continue;
      }
      if (typeof target.value !== 'string' || target.value === '') {
        errors.push(
          `${host} entry "${entry.identity}" declares a ${target.shape} target with no value — a shape without a value names nothing`,
        );
        shaped = false;
        continue;
      }
      declaredKeys.add(targetKey(target));
    }
    if (!shaped) continue;
    // ③ 声明了目标，命令却解析不出任何目标 ⇒ 凭据是空话。
    if (producedKeys.size === 0) {
      errors.push(
        `${host} entry "${entry.identity}" declares the target(s) ${[...declaredKeys].sort().join(', ')}, but its command yields none — the evidence is a claim, not a fact`,
      );
      continue;
    }
    // ④ 双向对账。两个方向都具名——「目标被悄悄换掉」与「声明是空话」是两种不同的缺陷。
    for (const key of [...producedKeys].sort()) {
      if (!declaredKeys.has(key)) {
        errors.push(
          `${host} entry "${entry.identity}" runs "${key}", which it does not declare — a step whose command was swapped is a judgement that silently disappeared`,
        );
      }
    }
    for (const key of [...declaredKeys].sort()) {
      if (!producedKeys.has(key)) {
        errors.push(
          `${host} entry "${entry.identity}" declares "${key}", which its command does not run — the declaration is a claim, not a fact`,
        );
      }
    }
  }
  return errors;
}
