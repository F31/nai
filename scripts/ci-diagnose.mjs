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

/**
 * **强信号**：这一行几乎必然指向失败原因。
 *
 * 覆盖本仓库门控实际会产出的形态：`FRONTEND_STEP_FAILED: …`（前端门控）、`error[E0308]: …`（rustc/clippy）、
 * `error: process didn't exit successfully: …`（cargo）、`thread '…' panicked at …`、`failures:`、
 * `test result: FAILED.`、xtask 对未实现任务的具名拒绝（`unsupported or not implemented task: …`）。
 *
 * 注意 `FAILED` 必须**区分大小写**地单独成条：cargo 成功时输出的是小写的 `test result: ok. 6 passed;
 * 0 failed;`，若用 `/failed/i` 匹配，每一个**成功**的测试目标都会变成「失败信号」——真实 CI 失败时
 * 满屏都是这种噪音，会把真正的错误挤出配额。**这是本脚本第一版的真实缺陷，由 2026-10-11 的一次真实
 * ubuntu 失败暴露**（12 条注释里 8 条是 `0 failed`，真正的错误行一条也没出现）。
 */
const STRONG =
  /error|FAILED|panicked|unsupported|not implemented|unimplemented|assertion|失败|未通过|不通过/;

/** 弱信号：`failures:` 之类的小写措辞，可能出现在正常叙述里，故排在强信号之后。 */
const WEAK = /\bfailures?:/;

/**
 * **永不入选**的噪音：cargo 对每个**成功**的测试目标都会印一行 `test result: ok. N passed; 0 failed; …`。
 * 它既不含信息量，又会在补齐阶段挤满配额——2026-10-11 的真实 ubuntu 失败里，12 条注释有 8 条是它，
 * 而真正的错误一行都没出现。识别它比逐个排除更稳：整行以 `test result: ok.` 开头即丢弃。
 */
const NOISE = /^test result: ok\./;

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
 * **强信号在全量日志里找**，而不是只看尾部：cargo 会先打印成千上万行 `Compiling` / `Checking`，
 * 真实错误可能出现在数千行之前，只看尾部会漏掉根因。
 * 弱信号与「尾部普通行」只作为补齐手段，并按原顺序输出（`Set` 选集合 + 按 `lines` 过滤）。
 */
function pickLines(lines, maxLines) {
  const strong = [];
  const weak = [];
  const rest = [];
  for (let i = 0; i < lines.length; i += 1) {
    if (STRONG.test(lines[i])) {
      strong.push(i);
    } else if (WEAK.test(lines[i])) {
      weak.push(i);
    } else if (!NOISE.test(lines[i])) {
      rest.push(i);
    }
  }
  // 按**索引**选取，不按行内容：cargo 的输出里大量存在重复行（每个成功测试目标都印一遍
  // `test result: ok. …`），按值去重会让「同一内容的所有行」一起被选中——第一版正是这样把
  // 803 条注释塞满上限、把真正的错误挤掉的。索引天然去重且保持原顺序。
  const chosen = new Set(strong.slice(0, maxLines));
  for (const index of weak.slice(0, maxLines - chosen.size)) {
    chosen.add(index);
  }
  const budget = maxLines - chosen.size;
  for (const index of rest.slice(Math.max(0, rest.length - budget))) {
    chosen.add(index);
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

  const { picked, omitted, total } = pickLines(lines, maxLines);
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
    `[ci-diagnose] 共 ${total} 行，输出 ${picked.length} 条 annotation（其余 ${omitted} 行未展示）`,
  );
  return 0;
}

process.exit(main(process.argv.slice(2)));