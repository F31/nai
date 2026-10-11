#!/usr/bin/env node
// 前端 workspace 的工程门控驱动。
//
// 由 `cargo xtask check --profile full` 调用，也可以 `node scripts/frontend-gate.mjs` 单独运行。
//
// 为什么是 Node 脚本，而不是让 xtask 直接调 `pnpm`：
//   * 本仓库的 `node_modules` 由 Linux 容器安装，`node_modules/.bin/*` 是一千字节左右的
//     POSIX shell 垫片，Windows 上不可执行；`pnpm` 本身也不保证在 PATH 上（本机只有
//     corepack）。把「pnpm 在不在」混进门控结论，会让**主机环境事实**看起来像前端缺陷。
//   * 「前端该怎么检查」天然是 JS 生态的问题：读 package.json、解析 npm 包入口、展开 glob。
//
// 「跑什么」取自各包 `package.json` 的 `scripts` —— 单一真源。本脚本只做一件事：把
// scripts 里的可执行名解析成**真实入口**，不定义第二套可能与 pnpm 漂移的检查命令。
// 不认识的可执行名与门控不实现的 shell 语法一律具名拒绝，绝不落到 PATH 上猜测执行。

import { spawnSync } from 'node:child_process';
import { readFileSync, readdirSync } from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

/**
 * 门控步骤：(包目录, script 名)。
 * 顺序有意为之 —— 先类型、再构建、后测试；任一步失败即停，后续步骤不再执行。
 */
const STEPS = [
  ['packages/contracts-ts', 'typecheck'],
  ['apps/desktop', 'typecheck'],
  ['packages/contracts-ts', 'build'],
  ['apps/desktop', 'build'],
  ['apps/desktop', 'test'],
  ['.', 'test:ui'],
];

/** 已知可执行。scripts 里出现别的名字一律具名拒绝（不落到 PATH 上猜）。 */
const KNOWN_EXECUTABLES = new Set(['node', 'tsc', 'vite']);

/** 门控不实现的 shell 语法：出现即拒绝，而不是近似执行它看不懂的命令。 */
const SHELL_SYNTAX = ['|', '>', '<', ';', '`', '$('];

class GateError extends Error {}

function fail(code, message) {
  throw new GateError(`${code}: ${message}`);
}

function readJson(file) {
  try {
    return JSON.parse(readFileSync(file, 'utf8'));
  } catch (error) {
    fail('FRONTEND_DEPS_MISSING', `无法读取 ${path.relative(REPO_ROOT, file)}：${error.message}`);
  }
}

/** 在某包的解析上下文里定位一个 npm 包（或包内路径）。 */
function resolveFrom(pkgDir, specifier) {
  const require = createRequire(path.join(pkgDir, 'package.json'));
  try {
    return require.resolve(specifier);
  } catch (error) {
    fail(
      'FRONTEND_DEPS_MISSING',
      `在 ${path.relative(REPO_ROOT, pkgDir)} 中无法解析 \`${specifier}\`（${error.code}）；` +
        `前端门控只检查已安装的依赖，不代跑 install`,
    );
  }
}

/**
 * 把 scripts 里的可执行名解析成真实调用：程序 + 前置参数。
 *
 * 所有 JS 工具都经**当前 node** 运行其 JS 入口，而不是把可执行名当裸命令交给 PATH ——
 * npm 的 `.bin` 垫片是无扩展名的 POSIX 脚本（本仓库的由 Linux 容器生成），Windows 上
 * 既不能直接执行也不是可识别命令。`typescript` 与 `vite` 的入口都是 `.js`，故一律走 node。
 */
function resolveInvocation(name, pkgDir) {
  if (name === 'node') {
    return { program: process.execPath, prefix: [] };
  }
  if (name === 'tsc') {
    return { program: process.execPath, prefix: [resolveFrom(pkgDir, 'typescript/lib/tsc.js')] };
  }
  if (name === 'vite') {
    // vite 的 exports 不暴露 bin/vite.js，故经包根拼路径，而不做 require.resolve。
    const pkgRoot = path.dirname(resolveFrom(pkgDir, 'vite/package.json'));
    const meta = readJson(path.join(pkgRoot, 'package.json'));
    const relative = typeof meta.bin === 'string' ? meta.bin : meta.bin?.vite;
    if (!relative) {
      fail('FRONTEND_DEPS_MISSING', `vite 包（${pkgRoot}）未声明 bin.vite`);
    }
    return { program: process.execPath, prefix: [path.join(pkgRoot, relative)] };
  }
  fail(
    'FRONTEND_SCRIPT_UNSUPPORTED',
    `门控不认识可执行 \`${name}\`（已知：${[...KNOWN_EXECUTABLES].join(', ')}）`,
  );
}

function escapeRegExp(text) {
  return text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

/**
 * 展开 `tests/ui/*.test.mjs` 这类参数。
 *
 * 门控不经过 shell，故 glob 由本脚本自己展开；**未匹配到任何文件即失败** ——
 * 「0 个测试文件被跑到」绝不能被读成「测试通过」。
 */
function expandGlobs(cwd, args) {
  const out = [];
  for (const arg of args) {
    if (!arg.includes('*')) {
      out.push(arg);
      continue;
    }
    const dir = path.dirname(arg);
    const pattern = path.basename(arg);
    const regexp = new RegExp(
      `^${pattern.split('*').map(escapeRegExp).join('[^/\\\\]*')}$`,
    );
    let entries;
    try {
      entries = readdirSync(path.join(cwd, dir));
    } catch (error) {
      fail('FRONTEND_SCRIPT_UNSUPPORTED', `glob \`${arg}\` 的目录不存在：${error.message}`);
    }
    const hits = entries.filter((entry) => regexp.test(entry)).sort();
    if (hits.length === 0) {
      fail('FRONTEND_SCRIPT_UNSUPPORTED', `glob \`${arg}\` 未匹配到任何文件`);
    }
    out.push(...hits.map((hit) => path.join(dir, hit)));
  }
  return out;
}

/** 取一个 step 在该包 scripts 里的原文，拆成子命令序列并校验。 */
function planStep(dir, scriptName) {
  const pkgDir = path.join(REPO_ROOT, dir);
  const meta = readJson(path.join(pkgDir, 'package.json'));
  const raw = meta.scripts?.[scriptName];
  if (typeof raw !== 'string' || raw.trim() === '') {
    fail(
      'FRONTEND_SCRIPT_UNSUPPORTED',
      `${meta.name} 的 package.json 里没有 scripts["${scriptName}"]`,
    );
  }
  const subcommands = raw
    .split('&&')
    .map((part) => part.trim())
    .filter(Boolean);
  return subcommands.map((sub) => {
    const tokens = sub.split(/\s+/);
    for (const token of tokens) {
      const offending = SHELL_SYNTAX.find((syntax) => token.includes(syntax));
      if (offending) {
        fail(
          'FRONTEND_SCRIPT_UNSUPPORTED',
          `${meta.name} 的 scripts["${scriptName}"] 含门控不实现的 shell 语法 \`${offending}\`（token \`${token}\`）`,
        );
      }
    }
    if (!KNOWN_EXECUTABLES.has(tokens[0])) {
      resolveInvocation(tokens[0], pkgDir); // 复用同一处具名拒绝
    }
    return tokens;
  });
}

function runStep(dir, scriptName, tokens) {
  const pkgDir = path.join(REPO_ROOT, dir);
  const { program, prefix } = resolveInvocation(tokens[0], pkgDir);
  const args = [...prefix, ...expandGlobs(pkgDir, tokens.slice(1))];
  const status = spawnSync(program, args, { cwd: pkgDir, stdio: 'inherit' });
  if (status.error) {
    fail(
      'FRONTEND_TOOLCHAIN_MISSING',
      `无法运行 \`${path.basename(program)}\`：${status.error.message}`,
    );
  }
  if (status.status !== 0) {
    fail(
      'FRONTEND_STEP_FAILED',
      `${dir === '.' ? '<root>' : dir} 的 scripts["${scriptName}"]（\`${tokens.join(' ')}\`）` +
        `以退出状态 ${status.status ?? `signal ${status.signal}`} 结束`,
    );
  }
}

function main() {
  for (const [dir, scriptName] of STEPS) {
    const label = `${dir === '.' ? '<root>' : dir} → ${scriptName}`;
    console.log(`\n[frontend] ${label}`);
    try {
      for (const tokens of planStep(dir, scriptName)) {
        runStep(dir, scriptName, tokens);
      }
    } catch (error) {
      if (error instanceof GateError) {
        console.error(`[frontend] ${error.message}`);
        console.error(`[frontend] 失败于 ${label}；后续步骤不再执行`);
        process.exit(1);
      }
      throw error;
    }
  }
  console.log('\n[frontend] 全部步骤通过（工程基础，非渲染/人工 UI 验收）');
}

main();
