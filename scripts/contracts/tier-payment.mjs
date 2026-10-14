/// 站点**图层声明（`tier`）的兑付**闭世界。
///
/// `TEST_REGISTRATION_SITES` 的每个站点都声明了一个 `tier`，意思是「**跑它的最低一层门控**」：
/// `portable` = `engineering_checks`（`portable` 与 `full` 都从这里开始）、
/// `full` = 前端门控驱动表、`ci` = 只在 CI 工作流里。
///
/// 这个模块存在的理由：`tier` 此前**只有形状检查**（`siteShapeErrors` 里的
/// `TEST_TIERS.includes(site.tier)`）——一个站点把 `tier` 从 `portable` 改成 `full`
/// 不会改变覆盖集、不会改变闭世界、门控照样全绿。**声明没有付代价**。
///
/// 实测（W99 探针 P-A）：把 `tests/contracts/npm-scripts.test.mjs` 的注册从 `engineering_checks`
/// **移进** `["check","--profile","full"]` arm，站点仍声明 `tier: 'portable'` ——
/// **192 passed / 0 failed，零信号**。
///
/// 于是补法与母题同形：**声明的兑付平面由宿主解析得出**，MUST NOT 是站点自己那段文本。
/// 三条纪律照抄自前轮：
/// 1. **判定域由解析得出**（W96）：`gateRegions` / `driverSteps` / `workflowStepsOf`，
///    不是一个全文 `.includes()`——marker 可填自己的文本 ⇒ 自我满足。
/// 2. **注释不是引用**（W97 `stripRustComments`，W99 换了一层）：`engineering_checks` 区域的**原文**
///    含 `tests/contracts/descriptor.test.mjs`，但它**只在注释里**（实测 `stripped=false`）。
///    不剥注释，两个 `ci` 站点会被这条注释自我满足。
/// 3. **对称方向也要堵**（W96「逃离」教训）：只做正向会漏掉「声明少报了本机门控真正跑到的东西」
///    ⇒ `full` / `ci` 的 marker MUST NOT 落在 portable 区域里。

import {TEST_TIERS, TEST_REGISTRATION_SITES} from './test-reachability.mjs';
import {gateRegions} from './xtask-commands.mjs';
import {stripRustComments} from './gate-coverage.mjs';
import {driverSteps, packageDirOf, packageScripts} from './npm-scripts.mjs';
import {workflowStepsOf} from './workflow-steps.mjs';

/// 门控宿主：`portable` 面的**唯一**来源。
export const HARNESS_LIB = 'xtask/src/lib.rs';

/// 前端门控驱动表：`full` 面的**唯一**来源。
export const DRIVER = 'scripts/frontend-gate.mjs';

/// CI 工作流：`ci` 面的**唯一**来源。
export const CI_WORKFLOW = '.github/workflows/foundation.yml';

/// `portable` 区域的名字（`gateRegions` 对 `fn engineering_checks(` 的函数体使用的名字）。
export const PORTABLE_REGION = 'engineering_checks';

/// **兑付规则表**：每个 tier 取值 ⇒ 兑付它的那一层**面**。
///
/// 这是一张**闭集**：它的键集 MUST 等于 `TEST_TIERS`。新增一个 tier 取值而不在这里归类，
/// 会让判据**静默地不认识它**（取值域闭世界只写「运行时域外 ⇒ 失败」是不够的，那道
/// 只能挡住**已有的**取值）。`tierErrors` 的第一条断言就是这两张表的键集相等。
export const TIER_PAYMENTS = {
  portable: {surface: 'engineering_checks-region'},
  full: {surface: 'driver-step-script'},
  ci: {surface: 'named-ci-step-run'},
};

/// 从 host 路径取模块名（`xtask/src/registry.rs` ⇒ `registry`）。
function moduleStem(host) {
  return String(host).replace(/^.*\//, '').replace(/\.rs$/, '');
}

/// 剥掉 shell / `run:` 块里的注释：整行注释（`^\s*#`）与尾注（前导空白 + `#` 到行尾）。
///
/// **为什么必须有这一层**：`run:` 里写一行 `# tests/x.test.mjs` 是**散文**，它不执行任何东西；
/// 不剥注释就会把「没有人跑它」换成「有人提到它」。
///
/// ⚠️ 有意**单向保守**：引号内的 ` #` 也会被当成注释起点（当前仓库没有任何命令含它）。
/// 过剥的代价是**假红**（该付的代价看不见），方向安全；反之（不剥）的代价是**假绿**。
export function stripShellComments(text) {
  return String(text)
    .split('\n')
    .map(line => {
      const cut = line.replace(/\s+#.*$/, '');
      return /^\s*#/.test(cut) ? '' : cut;
    })
    .join('\n');
}

/// `portable` 兑付面：`gateRegions(libText)` 的 `engineering_checks` 区域，**再剥注释**。
/// 不可定位 / 不可解析 ⇒ 抛（调用方转成一条具名错误，MUST NOT 静默返回空错误数组）。
export function portableSurface(libText) {
  if (typeof libText !== 'string' || libText === '') {
    throw new Error('the harness source was not provided, so no portable tier can be judged');
  }
  const regions = gateRegions(libText);
  const region = regions.find(candidate => candidate.name === PORTABLE_REGION);
  if (!region) {
    throw new Error(`the harness carries no \`${PORTABLE_REGION}\` region`);
  }
  return {text: stripRustComments(region.text)};
}

/// 一个 `portable` 站点的兑付串。**形状由 host 派生**，不留可选的 `via` 字段
/// （「可选」就是判据的缝——一个可省的字段会让某个站点悄悄走默认分支）。
///
/// - host 是宿主文件本身 ⇒ 兑付串就是 marker（内联注册：`--test <marker>` 就写在区域里）；
/// - 否则 ⇒ 兑付串是「该 host 的模块在区域里被调用」，形状收紧到`<module>::<fn>(root`
///   （`root` 是本仓库这些入口的统一实参约定：`deps::check(root)`、`registry::check(root)`、
///   `schema::node(root, …)`）。
function portablePayment(site, surface) {
  const where = `the ${PORTABLE_REGION} region of ${HARNESS_LIB}`;
  if (site.host === HARNESS_LIB) {
    return {token: site.marker, ok: surface.text.includes(site.marker), where};
  }
  const stem = moduleStem(site.host).replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const call = new RegExp(`\\b${stem}::\\w+\\(root`);
  return {token: `${stem}::<fn>(root`, ok: call.test(surface.text), where};
}

/// **对称方向**：`full` / `ci` 站点的 marker MUST NOT 落在 portable 区域（剥注释后）里。
/// 否则该声明**少报**了本机门控真正跑到的东西 ⇒「逃离 portable 面」成了一条合法路径
/// （`tier` 的语义是「跑它的**最低**一层」，所以「也在 portable 面里」意味着它就该声明 `portable`）。
function reverseErrors(sites, surface) {
  const errors = [];
  for (const site of sites) {
    if (site.tier === 'portable') continue;
    if (surface.text.includes(site.marker)) {
      errors.push(
        `site ${site.id} declares tier ${site.tier} yet its marker ${JSON.stringify(site.marker)} is inside ${PORTABLE_REGION} of ${HARNESS_LIB} once comments are stripped — the declaration understates what the local gate already runs`,
      );
    }
  }
  return errors;
}

/// `full` 的兑付：`packageDirOf(host)` MUST 是驱动表某个 STEP 的 `dir`，
/// 且**该 STEP 的 `script` 在该包 `scripts` 里的值** MUST 含 marker。
///
/// 「该目录出现过任何 STEP」不算兑付：`apps/desktop` 有 3 个 STEP
/// （`typecheck` / `build` / `test`），只有 `test` 的值含 `tests/*.test.mjs`。
function fullPayments(sites, driverText, packages) {
  let steps;
  try {
    steps = driverSteps(driverText);
  } catch (error) {
    return [`${DRIVER} could not be parsed, so no full tier can be judged: ${error.message}`];
  }
  let scripts;
  try {
    scripts = packageScripts(packages);
  } catch (error) {
    return [`the package manifests could not be parsed, so no full tier can be judged: ${error.message}`];
  }

  const errors = [];
  for (const site of sites) {
    const dir = packageDirOf(site.host);
    if (dir === null) {
      errors.push(
        `site ${site.id} declares tier full but its host ${site.host} is not a package manifest — a full-tier site is wired through a package script`,
      );
      continue;
    }
    const dirSteps = steps.filter(step => step.dir === dir);
    if (dirSteps.length === 0) {
      errors.push(
        `site ${site.id} declares tier full, but ${DRIVER} names no STEP for package dir ${JSON.stringify(dir)} — nothing in the frontend gate runs it`,
      );
      continue;
    }
    if (!scripts.has(dir)) {
      errors.push(
        `site ${site.id} declares tier full, but the manifest of package dir ${JSON.stringify(dir)} was not provided, so its scripts cannot be judged`,
      );
      continue;
    }
    const bodies = dirSteps.map(step => scripts.get(dir).scripts?.[step.script]);
    if (!bodies.some(body => typeof body === 'string' && body.includes(site.marker))) {
      errors.push(
        `site ${site.id} declares tier full yet ${JSON.stringify(site.marker)} is not in the script body of any STEP of ${JSON.stringify(dir)} — a driver entry that does not name the marker is an indirection, not a payment`,
      );
    }
  }
  return errors;
}

/// `ci` 的兑付：marker MUST 逐字出现在某个**具名** step 的 `run` 文本（剥整行注释）里。
///
/// 「具名」= `workflowStepsOf` 给出的 `identity !== ''`（W95 的身份定义）：一条没有名字的 `run`
/// 无法被指向，也就不该成为证据。
function ciPayments(sites, workflowText) {
  let steps;
  try {
    steps = workflowStepsOf(workflowText);
  } catch (error) {
    return [`${CI_WORKFLOW} could not be parsed, so no ci tier can be judged: ${error.message}`];
  }
  const errors = [];
  if (steps.length === 0) {
    errors.push(`${CI_WORKFLOW} declares no steps — an empty step set makes the ci tier vacuously true`);
  }
  const named = steps.filter(step => step.identity !== '');
  for (const site of sites) {
    const hits = named.filter(step => stripShellComments(step.run).includes(site.marker));
    if (hits.length === 0) {
      errors.push(
        `site ${site.id} declares tier ci yet ${JSON.stringify(site.marker)} is not named by any step's run in ${CI_WORKFLOW} — a CI-only test that no named step runs is silently unreachable`,
      );
    }
  }
  return errors;
}

/// 主判据。入参**显式注入**；缺输入按其**需要**具名（MUST NOT 静默跳过——
/// 一条会因入参缺失而恒真的断言比没有断言更坏）。
///
/// - `sites`：站点表（默认 `TEST_REGISTRATION_SITES`）；
/// - `libText`：`xtask/src/lib.rs` 全文（只要存在任何站点就需要——`portable` 的兑付面，
///   以及 `full`/`ci` 的反向断言都靠它）；
/// - `driverText`：`scripts/frontend-gate.mjs` 全文（存在 `full` 站点时必需）；
/// - `workflowText`：`.github/workflows/foundation.yml` 全文（存在 `ci` 站点时必需）；
/// - `packages`：`Map<包目录, package.json 文本>`（存在 `full` 站点时必需）。
export function tierErrors({
  sites = TEST_REGISTRATION_SITES,
  libText,
  driverText,
  workflowText,
  packages,
} = {}) {
  const errors = [];

  // ---------- T1 规则闭世界：取值域的两道 ----------
  const declared = [...TEST_TIERS].sort();
  const ruled = Object.keys(TIER_PAYMENTS).sort();
  if (declared.join('|') !== ruled.join('|')) {
    errors.push(
      `TEST_TIERS (${declared.join(', ')}) and TIER_PAYMENTS (${ruled.join(', ')}) disagree — a tier with no payment rule would be silently unrecognised`,
    );
  }

  const list = [...sites];
  for (const site of list) {
    if (!Object.prototype.hasOwnProperty.call(TIER_PAYMENTS, site.tier)) {
      errors.push(
        `site ${site.id} declares tier ${JSON.stringify(site.tier)}, which has no payment rule — a tier outside the rule table is a label, not a declaration`,
      );
    }
  }
  for (const tier of TEST_TIERS) {
    if (!list.some(site => site.tier === tier)) {
      errors.push(
        `no registration site declares tier ${JSON.stringify(tier)} — a payment rule with no site to pay for is vacuously true`,
      );
    }
  }

  const ofTier = tier => list.filter(site => site.tier === tier);

  // ---------- T2 + T5：portable 面的正向与反向 ----------
  if (list.length > 0) {
    let surface = null;
    try {
      surface = portableSurface(libText);
    } catch (error) {
      errors.push(`the portable surface could not be read, so no tier can be judged against it: ${error.message}`);
    }
    if (surface !== null) {
      for (const site of ofTier('portable')) {
        const payment = portablePayment(site, surface);
        if (!payment.ok) {
          errors.push(
            `site ${site.id} declares tier portable yet ${JSON.stringify(payment.token)} is not in ${payment.where} — a tier declaration with no region paying for it is a lie`,
          );
        }
      }
      errors.push(...reverseErrors(list, surface));
    }
  }

  // ---------- T3：full 面（驱动表 STEP 的脚本体） ----------
  const full = ofTier('full');
  if (full.length > 0) {
    if (typeof driverText !== 'string' || driverText === '') {
      errors.push(
        `tier full is declared by ${full.length} site(s) but ${DRIVER} was not provided, so nothing can be judged`,
      );
    } else if (!(packages instanceof Map) || packages.size === 0) {
      errors.push(
        'tier full is declared but the package manifests were not provided as a map, so nothing can be judged',
      );
    } else {
      errors.push(...fullPayments(full, driverText, packages));
    }
  }

  // ---------- T4：ci 面（具名 step 的 run 文本） ----------
  const ci = ofTier('ci');
  if (ci.length > 0) {
    if (typeof workflowText !== 'string' || workflowText === '') {
      errors.push(
        `tier ci is declared by ${ci.length} site(s) but ${CI_WORKFLOW} was not provided, so nothing can be judged`,
      );
    } else {
      errors.push(...ciPayments(ci, workflowText));
    }
  }

  return errors;
}
