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

import { readFileSync } from 'node:fs';

const USAGE = 'Usage: node scripts/ci-diagnose.mjs <log-path> [--max-lines N] [--max-chars N]';

/** 候选行的最大条数：GitHub UI 每个 step 只显示有限条注释，API 可读全部，故这是一个显示与信噪比的折中。 */
const DEFAULT_MAX_LINES = 25;

/** 单条 annotation 的最大字符数，避免被服务端截断后丢掉尾部信息。 */
const DEFAULT_MAX_CHARS = 400;

/** 参与挑选的日志窗口大小：门控失败的关键信息总是出现在日志尾部。 */
const TAIL_WINDOW = 400;

/**
 * 判定「这一行像是失败信息」的信号。
 *
 * 覆盖本仓库门控实际会产出的形态：`FRONTEND_STEP_FAILED: …`（前端门控）、
 * `error[E0308]: …`（rustc/clippy）、`---- xxx stdout ----`、`thread '…' panicked at …`、
 * `failures:`、`test result: FAILED.`、`失败于 …；后续步骤不再执行`，以及 xtask 对未实现任务的
 * 具名拒绝（`unsupported or not implemented task: …`）。
 */
const SIGNAL =
  /error|failed|failures?|panicked|assertion|unsupported|not implemented|unimplemented|失败|未通过|不通过/i;

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
 * 优先挑选「看起来像失败原因」的行，不足再用尾部普通行补齐，并**按原顺序**输出。
 *
 * 只看尾部会漏掉「错误在前、大段 cargo 输出在后」的情况；只看信号行则在纯编译失败、
 * 或失败行恰好不含关键词时全空。两者结合，再用 Set 恢复原顺序。
 */
function pickLines(lines, maxLines) {
  const signals = lines.filter((line) => SIGNAL.test(line));
  if (signals.length >= maxLines) {
    const picked = lines.filter((line) => signals.slice(-maxLines).includes(line));
    return { picked, omitted: Math.max(0, signals.length - maxLines), total: lines.length };
  }
  const rest = lines.filter((line) => !SIGNAL.test(line));
  const need = maxLines - signals.length;
  const chosen = new Set([...signals, ...rest.slice(-need)]);
  return {
    picked: lines.filter((line) => chosen.has(line)),
    omitted: Math.max(0, rest.length - need),
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

  const window = lines.slice(-TAIL_WINDOW);
  const { picked, omitted, total } = pickLines(window, maxLines);
  const title = escapeProperty('gate-failure-summary');
  for (const line of picked) {
    console.log(`::error title=${title}::${escapeData(clamp(line, maxChars))}`);
  }
  if (omitted > 0) {
    console.log(
      `::error title=${title}::${escapeData(`（另有 ${omitted} 行未展示；完整输出见该 job 的日志）`)}`,
    );
  }
  console.log(
    `[ci-diagnose] 共 ${total} 行（窗口取尾部 ${window.length} 行），输出 ${picked.length} 条 annotation`,
  );
  return 0;
}

process.exit(main(process.argv.slice(2)));