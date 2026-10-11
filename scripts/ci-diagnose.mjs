#!/usr/bin/env node
// 把门控日志里的失败摘要转成 GitHub Actions 的 check-run annotation。
//
// 为什么需要它：CI 的**执行日志**只能通过 `GET /repos/{owner}/{repo}/actions/jobs/{id}/logs`
// 读取，而该端点要求仓库 **admin 权限**；无权限的读者只能看到 `Process completed with exit
// code 1.`，**看不到是哪个测试、哪条规则失败**。workflow command（`::error::`）会被 GitHub 解析成
// check-run 的 annotation，而 `GET /repos/{owner}/{repo}/check-runs/{id}/annotations`
// **匿名可读** ⇒ 这条通道让 CI 失败在**没有凭据**时也能被诊断。
//
// 用法：`node scripts/ci-diagnose.mjs <log-path> [--max-lines N] [--max-chars N]`
// 由 `.github/workflows/foundation.yml` 在门控步骤失败后调用。
//
// 契约（改动本脚本时必须保持）：
//   * 日志**缺失或为空** ⇒ 不输出任何 annotation、退出 0。诊断不得凭空制造噪音，也不得让一个
//     「本来就没有失败输出」的 job 凭空多出错误。
//   * 本脚本**永远以 0 退出**（参数错误除外，因为那是脚本自身的缺陷、应当被发现）。诊断失败
//     MUST NOT 掩盖原始失败——workflow 侧另有 `continue-on-error` 作为第二道保险。
//   * **逐行**输出而非整块输出：单条 annotation 过长会被截断，逐行能让关键行各自完整可见。
//   * 剥离 ANSI 转义序列：cargo 的颜色码写进 annotation 只会变成乱码。
//   * `%`、`\r`、`\n` 必须按 workflow command 的规则转义，否则会被截断或错误解析；**property 值**
//     另外还要转义 `:` 与 `,`，否则会被当成新的属性分隔。消息里的 `::` 无需处理——GitHub 只在
//     命令之后的**第一个** `::` 处切分属性与消息，这与 `@actions/core` 的转义规则一致。
//   * **有信号就只输出信号**：一条都没有时才退化到「尾部窗口」。用无关行去填满配额会把真正的错误
//     淹没——这是本脚本第二版的真实缺陷，由 2026-10-11 的**第一次真实** ubuntu 间歇失败暴露。

import { readFileSync } from 'node:fs';

const USAGE = 'Usage: node scripts/ci-diagnose.mjs <log-path> [--max-lines N] [--max-chars N]';

/** 候选行的最大条数：GitHub UI 每个 step 只显示有限条注释，API 可读全部，故这是一个显示与信噪比的折中。 */
const DEFAULT_MAX_LINES = 25;

/** 单条 annotation 的最大字符数，避免被服务端截断后丢掉尾部信息。 */
const DEFAULT_MAX_CHARS = 400;

/**
 * 每个信号行**之后**附带的行数：失败详情（断言消息、失败名列表）就排在信号行之后。
 * 信号行**之前**的内容不按固定偏移取——那里最有价值的是测试目标头，另由 `TARGET_HEADER` 精确取。
 */
const CONTEXT_LINES = 3;

/** 测试目标头（`     Running tests/engine.rs (target/debug/deps/engine-…)`，含 lib 单测与 Doc-tests）。 */
const TARGET_HEADER = /^\s+Running /;

/**
 * **强信号**：这一行几乎必然指向失败原因。
 *
 * 覆盖本仓库门控实际会产出的形态：`error[E0308]: …`（rustc/clippy，**带方括号的错误码**）、
 * `error: could not compile …` / `error: test failed …`（cargo）、`thread '…' panicked at …`、
 * `test result: FAILED.` / `test … FAILED`、`FRONTEND_STEP_FAILED: …`（前端门控）、
 * xtask 对未实现任务的具名拒绝（`unsupported or not implemented task: …`）。
 *
 * 两个都必须**精确**，否则会把噪音当成信号：
 *   * `FAILED` 必须**区分大小写**：cargo 成功时输出的是小写的 `test result: ok. 6 passed; 0 failed;`，
 *     用 `/failed/i` 会让**每一个成功**的测试目标都变成「失败信号」。**这是第一版的真实缺陷**
 *     （2026-10-11 的真实 ubuntu 失败里，12 条注释有 8 条是 `0 failed`，真正的错误一条也没出现）。
 *   * `error` 不能是**裸子串**：`Checking thiserror v2.0.21`、`Compiling thiserror-impl v2.0.21`
 *     全都含 `error`，于是每次编译都会贡献若干条「强信号」并挤占配额。**这是第二版的真实缺陷**
 *     （同一次真实失败暴露：11 条注释里 `thiserror` 的编译行占了 2 条）。故要求 `error` 后面紧跟
 *     `[`（错误码）或 `:`（措辞）——`thiserror` 两者都不满足。
 */
const STRONG = /\berror(\[|:)|FAILED|panicked|unsupported|not implemented|unimplemented|assertion|失败|未通过|不通过/;

/**
 * **弱信号**：`failures:` 是 libtest 汇总段的标题，其后紧跟失败测试名与各自的 stdout 详情。
 * 它排在强信号之后（可能出现在叙述性文本里），但仍是「值得连上下文一起带出」的锚点。
 */
const WEAK = /\bfailures?:/;

/**
 * **永不入选**的噪音——两类「看起来像信号、其实不是」的行：
 *
 *   1. cargo 对每个**成功**的测试目标都会印一行 `test result: ok. N passed; 0 failed; …`，既无信息量
 *      又会在补齐阶段挤满配额。
 *   2. libtest 的**通过/忽略**逐测试行 `test <name> ... ok|ignored`。这一类比第 1 类更隐蔽：本仓库有
 *      大量测试**名字里就含**信号词（`…_unimplemented_routes_…`、`…unsupported…`），关键字匹配会
 *      把一个**通过**的测试判成强信号，再把它两侧的上下文一起拖进来。**这是第三版的真实缺陷**，
 *      由 2026-10-11 的真实 ubuntu 失败在本地复演时暴露：一条 `… cors_queries_unimplemented_routes_…
 *      ... ok` 把紧随其后的两行 `thiserror` 编译输出带进了摘要。
 *
 * 排除它比收窄关键词更根本：**判定依据是「这行报告了失败吗」，而不是「这行含不含某个词」**。
 */
const NOISE = /^test result: ok\.|^test .+ \.\.\. (ok|ignored)\b/;

/** libtest 的**逐测试行**（`test <name> ... ok|FAILED|ignored`）与**汇总行**（`test result: …`）。 */
const PER_TEST = /^test .+ \.\.\. (ok|FAILED|ignored)\b/;
const TEST_SUMMARY = /^test result:/;

/**
 * **截断迹象**：看到了逐测试行，却**没有**汇总行。
 *
 * libtest 必然在跑完全部测试后打印 `test result: …`；只有它**没来得及打印**（进程被信号终止、
 * 输出缓冲丢失、采集被提前切断）时才会「有逐测试行、无汇总行」。2026-10-11 的第一次真实 ubuntu
 * 间歇失败正是这个形态：一个跑满 2 分 50 秒的门控，采集到的日志只有 11 行，含 `test … FAILED`
 * 却没有 `test result:`——**若不点破，读者会把一份不完整的日志当成全部证据**。
 */
function looksTruncated(lines) {
  return lines.some((line) => PER_TEST.test(line)) && !lines.some((line) => TEST_SUMMARY.test(line));
}

// CSI（ESC [ … 最终字节）与 OSC（ESC ] … BEL）两类转义序列。
const ANSI = /\u001B\[[0-9;?]*[A-Za-z]|\u001B\][^\u0007]*(?:\u0007|\u001B\\)/g;

function parseArgs(argv) {
  let logPath = null;
  let maxLines = DEFAULT_MAX_LINES;
  let maxChars = DEFAULT_MAX_CHARS;
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--max-lines') {
      maxLines = Number(argv[(i += 1)]);
    } else if (arg === '--max-chars') {
      maxChars = Number(argv[(i += 1)]);
    } else if (arg.startsWith('--')) {
      return { error: `未知参数 \`${arg}\`` };
    } else if (logPath === null) {
      logPath = arg;
    } else {
      return { error: `多余的位置参数 \`${arg}\`` };
    }
  }
  if (logPath === null) {
    return { error: '缺少日志路径' };
  }
  if (!Number.isInteger(maxLines) || maxLines <= 0) {
    return { error: '--max-lines 必须是正整数' };
  }
  if (!Number.isInteger(maxChars) || maxChars <= 0) {
    return { error: '--max-chars 必须是正整数' };
  }
  return { logPath, maxLines, maxChars };
}

function escapeData(value) {
  return String(value)
    .replace(/%/g, '%25')
    .replace(/\r/g, '%0D')
    .replace(/\n/g, '%0A');
}

/** property 值还要转义 `:` 与 `,`，否则会被当作新的属性分隔。 */
function escapeProperty(value) {
  return escapeData(value).replace(/:/g, '%3A').replace(/,/g, '%2C');
}

function clamp(text, limit) {
  return text.length <= limit ? text : `${text.slice(0, limit - 1)}…`;
}

/**
 * 选行：**有信号就只输出信号**，一条都没有时才退化到尾部窗口。
 *
 * 顺序是「强信号 → 其后置上下文与所属目标头 → 弱信号」，全部**按索引**记账：
 *   * 强信号在全量日志里找，而不是只看尾部——cargo 会先打印成千上万行 `Compiling` / `Checking`，
 *     真实错误可能出现在数千行之前，只看尾部会漏掉根因。
 *   * 后置上下文（`CONTEXT_LINES` 行）把断言消息、失败测试名列表一并带出——只有「哪一行像错误」
 *     是不够的，读者还需要知道**它说了什么**。
 *   * 目标头（`Running tests/xxx.rs …`）单独向前查找：测试名本身不足以说明失败落在哪个二进制。
 *   * 按**索引**而非行内容记账：cargo 的输出里大量存在重复行，按值去重会让「同一内容的所有行」
 *     一起被选中（第一版曾因此把上限冲成 803 条）；索引天然去重且保持原顺序。
 *   * **绝不**用任意行填满配额。当信号稀少时，输出少而准远好过输出满而糊——第二版正是用无关行
 *     把配额填满，才让 1 条真信号淹没在 10 条噪音里。
 */
function pickLines(lines, maxLines) {
  const strong = [];
  const weak = [];
  const rest = [];
  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i];
    if (NOISE.test(line)) {
      continue;
    }
    if (STRONG.test(line)) {
      strong.push(i);
    } else if (WEAK.test(line)) {
      weak.push(i);
    } else {
      rest.push(i);
    }
  }

  const chosen = new Set();
  const add = (index) => {
    if (index < 0 || index >= lines.length || chosen.size >= maxLines) {
      return;
    }
    if (NOISE.test(lines[index])) {
      return;
    }
    chosen.add(index);
  };

  for (const index of strong) {
    add(index);
  }
  for (const index of [...strong, ...weak]) {
    for (let offset = 1; offset <= CONTEXT_LINES; offset += 1) {
      add(index + offset);
    }
  }
  // 「失败发生在哪个测试目标里」只有目标头说得清——测试名本身不足以定位到二进制。
  // 向前找该信号行所属的最近目标头，**只带这一行**，不把整段拖进来。
  for (const index of strong) {
    for (let above = index - 1; above >= 0; above -= 1) {
      if (TARGET_HEADER.test(lines[above])) {
        add(above);
        break;
      }
    }
  }
  for (const index of weak) {
    add(index);
  }
  // 一条信号都没有（既没有强信号也没有弱信号）时才退化到尾部窗口：失败原因通常在最末。
  if (chosen.size === 0) {
    for (const index of rest.slice(Math.max(0, rest.length - maxLines))) {
      chosen.add(index);
    }
  }

  const ordered = [...chosen].sort((a, b) => a - b);
  return {
    picked: ordered.map((index) => lines[index]),
    omitted: lines.length - ordered.length,
    total: lines.length,
  };
}

function readLog(logPath) {
  try {
    return readFileSync(logPath, 'utf8');
  } catch (error) {
    if (error.code === 'ENOENT') {
      console.log(`[ci-diagnose] 日志不存在（${logPath}）；不输出 annotation`);
    } else {
      console.log(`[ci-diagnose] 读取日志失败（${error.message}）；不输出 annotation`);
    }
    return null;
  }
}

function main(argv) {
  const options = parseArgs(argv);
  if (options.error) {
    console.error(`[ci-diagnose] ${options.error}\n${USAGE}`);
    return 2;
  }
  const { logPath, maxLines, maxChars } = options;

  const raw = readLog(logPath);
  if (raw === null) {
    return 0;
  }
  const lines = raw
    .replace(ANSI, '')
    .replace(/\r\n?/g, '\n')
    .split('\n')
    .map((line) => line.replace(/\s+$/, ''))
    .filter((line) => line.length > 0);
  if (lines.length === 0) {
    console.log('[ci-diagnose] 日志为空；不输出 annotation');
    return 0;
  }

  const title = escapeProperty('gate-failure-summary');
  const truncated = looksTruncated(lines);
  if (truncated) {
    // 先把「这份日志本身不完整」说清楚，再列行——否则读者会误以为摘要就是全部证据。
    console.log(
      `::warning title=${title}::${escapeData(
        `日志在测试汇总行（test result:）之前结束（共 ${lines.length} 行）——采集到的输出可能被截断，失败详情未必完整`,
      )}`,
    );
  }

  const { picked, omitted, total } = pickLines(lines, maxLines);
  for (const line of picked) {
    console.log(`::error title=${title}::${escapeData(clamp(line, maxChars))}`);
  }
  if (omitted > 0) {
    console.log(
      `::error title=${title}::${escapeData(`（另有 ${omitted} 行未展示；完整输出见该 job 的日志）`)}`,
    );
  }
  console.log(
    `[ci-diagnose] 共 ${total} 行，输出 ${picked.length} 条 annotation` +
      `（其余 ${omitted} 行未展示${truncated ? '；**检测到截断迹象**' : ''}）`,
  );
  return 0;
}

process.exit(main(process.argv.slice(2)));
