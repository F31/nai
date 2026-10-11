#!/usr/bin/env node
// 平台能力矩阵校验。
//
// 三件事互相约束：
//   1. 矩阵内部自洽（平台唯一、每项能力在每个平台上都有状态、状态与理由码配合正确）；
//   2. 理由码取自 nai-plugin-sdk 的 UnavailableReason —— **从源码派生**，不在这里另立一份常量表，
//      否则改一侧忘另一侧时校验会变成假的保证；
//   3. 源码里的平台条件编译文件 MUST 是登记路径的子集 —— 平台假设从此只增不减会被门控挡住。
//
// 纯静态：不编译、不联网、不需要 protoc。故它进 engineering_checks。

import { readdirSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';

export const RELEASE_LINES = ['slice_0', 'r1', 'r2', 'r3', 'experimental'];
export const STATES = ['supported', 'explicit-unavailable', 'out-of-scope'];
export const BOUND_TO = ['windows', 'linux'];

export const MATRIX_PATH = 'schemas/platform-matrix/platforms.json';
export const SCHEMA_PATH = 'schemas/platform-matrix/platforms.schema.json';
export const CONNECTOR_PATH = 'crates/nai-plugin-sdk/src/connector.rs';

/** macOS 的发布线归属：用户拍板覆盖 R2（Standard）与 R3（Pro），写成数据断言。 */
export const MACOS_REQUIRED_RELEASE_LINES = ['r2', 'r3'];

const SKIP_DIRS = new Set(['node_modules', 'target', '.git', 'dist', '.workbuddy', 'gen']);
const SOURCE_EXTENSIONS = new Set(['.rs', '.toml']);

/**
 * 平台条件编译的识别形态。**必须与设计文档中的口径一致**：
 * 属性形式 `#[cfg(...)]`、宏形式 `cfg!(...)`、清单形式 `[target.'cfg(...)'.x]`。
 */
const CFG_PATTERNS = [
  /#\[cfg\([^\]]*\b(?:windows|unix|target_os)\b[^\]]*\)\]/,
  /cfg!\([^)]*\b(?:windows|unix|target_os)\b[^)]*\)/,
  /\[target\.'cfg\([^\]]*\b(?:windows|unix|target_os)\b[^\]]*\)'\./,
];

/** 从文本的 startIndex 起，截取以第一个 `{` 开头的**配平**块（含两端大括号）。 */
function sliceBracedBlock(text, startIndex) {
  const open = text.indexOf('{', startIndex);
  if (open === -1) return null;
  let depth = 0;
  for (let i = open; i < text.length; i += 1) {
    if (text[i] === '{') depth += 1;
    else if (text[i] === '}') {
      depth -= 1;
      if (depth === 0) return text.slice(open, i + 1);
    }
  }
  return null;
}

/**
 * 从 `UnavailableReason::label()` 的 match 臂派生合法理由码集合。
 *
 * 派生前先断言该函数存在且解析结果非空——形状变化时必须**具名失败**，不能静默返回空集合
 * （空集合会让「任何理由码都不合法」或「任何理由码都合法」这类错误以假绿的形式溜过去）。
 *
 * @returns {{reasons: string[], error: string|null}}
 */
export function deriveUnavailableReasons(root, connectorPath = CONNECTOR_PATH) {
  const file = path.join(root, connectorPath);
  let text;
  try {
    text = readFileSync(file, 'utf8');
  } catch (error) {
    return { reasons: [], error: `cannot read ${connectorPath}: ${error.message}` };
  }
  const marker = 'pub fn label(self)';
  const at = text.indexOf(marker);
  if (at === -1) {
    return { reasons: [], error: `${connectorPath}: "${marker}" not found; reason derivation has no basis` };
  }
  const block = sliceBracedBlock(text, at);
  if (block === null) {
    return { reasons: [], error: `${connectorPath}: unbalanced braces after "${marker}"` };
  }
  const reasons = [...new Set(
    [...block.matchAll(/UnavailableReason::(\w+)\s*=>\s*"([a-z_]+)"/g)].map((match) => match[2]),
  )];
  if (reasons.length === 0) {
    return { reasons: [], error: `${connectorPath}: no reason codes parsed from label(); refusing to pass vacuously` };
  }
  return { reasons, error: null };
}

/**
 * 扫描全仓含平台条件编译的文件。
 *
 * @returns {Map<string, number>} 相对路径 → 命中行数
 */
export function collectPlatformBranchFiles(root, { skipDirs = SKIP_DIRS } = {}) {
  const found = new Map();
  const walk = (dir) => {
    let entries;
    try {
      entries = readdirSync(dir, { withFileTypes: true });
    } catch {
      return;
    }
    for (const entry of entries) {
      if (entry.isDirectory()) {
        if (skipDirs.has(entry.name)) continue;
        walk(path.join(dir, entry.name));
      } else if (entry.isFile()) {
        if (!SOURCE_EXTENSIONS.has(path.extname(entry.name))) continue;
        const full = path.join(dir, entry.name);
        let text;
        try {
          text = readFileSync(full, 'utf8');
        } catch {
          continue;
        }
        const lines = text.split('\n');
        let hits = 0;
        for (const line of lines) {
          if (CFG_PATTERNS.some((pattern) => pattern.test(line))) hits += 1;
        }
        if (hits > 0) found.set(path.relative(root, full).replaceAll('\\', '/'), hits);
      }
    }
  };
  walk(root);
  return found;
}

/**
 * 校验矩阵。
 *
 * @param {string} root 仓库根
 * @param {object} matrix 解析后的 platforms.json
 * @param {Map<string, number>|null} branchFiles 覆盖扫描结果（测试注入用）
 * @returns {string[]} 错误列表，空即通过
 */
export function validateMatrix(root, matrix, branchFiles = null) {
  const errors = [];

  // ---- 派生理由码 ---------------------------------------------------------
  const { reasons, error: deriveError } = deriveUnavailableReasons(root);
  if (deriveError) errors.push(deriveError);
  const legalReasons = new Set(reasons);

  // ---- 平台 ---------------------------------------------------------------
  const platforms = Array.isArray(matrix?.platforms) ? matrix.platforms : [];
  if (platforms.length === 0) {
    errors.push('matrix declares no platform');
    return errors;
  }
  const platformIds = [];
  for (const platform of platforms) {
    const id = platform?.id;
    if (typeof id !== 'string' || id.length === 0) {
      errors.push('a platform has no id');
      continue;
    }
    if (platformIds.includes(id)) errors.push(`platform ${id} is declared more than once`);
    platformIds.push(id);

    const triples = platform.target_triples;
    if (!Array.isArray(triples) || triples.length === 0) {
      errors.push(`platform ${id} declares no target triple`);
    } else {
      for (const triple of triples) {
        if (typeof triple !== 'string' || !/^[a-z0-9_]+-[a-z0-9_]+-[a-z0-9_]+(-[a-z0-9_]+)?$/.test(triple)) {
          errors.push(`platform ${id}: target triple "${triple}" is not <arch>-<vendor>-<os>[-<abi>]`);
        }
      }
    }
    const lines = platform.release_lines;
    if (!Array.isArray(lines)) {
      errors.push(`platform ${id} declares no release_lines array`);
    } else {
      for (const line of lines) {
        if (!RELEASE_LINES.includes(line)) {
          errors.push(`platform ${id}: release line "${line}" is not one of ${RELEASE_LINES.join('/')}`);
        }
      }
    }
  }
  const platformSet = new Set(platformIds);

  // ---- 能力 × 平台完备性 --------------------------------------------------
  const capabilities = Array.isArray(matrix?.capabilities) ? matrix.capabilities : [];
  if (capabilities.length === 0) errors.push('matrix declares no capability');
  const registeredFiles = new Set();
  const seenCapabilities = new Set();

  for (const capability of capabilities) {
    const id = capability?.id;
    if (typeof id !== 'string' || id.length === 0) {
      errors.push('a capability has no id');
      continue;
    }
    if (seenCapabilities.has(id)) errors.push(`capability ${id} is declared more than once`);
    seenCapabilities.add(id);

    // 每个平台都必须给出状态，且不得多出未声明的平台。
    const states = capability.states;
    if (states === null || typeof states !== 'object' || Array.isArray(states)) {
      errors.push(`capability ${id}: states must be an object`);
      continue;
    }
    const stated = Object.keys(states);
    const missing = platformIds.filter((platform) => !stated.includes(platform));
    const extra = stated.filter((platform) => !platformSet.has(platform));
    if (missing.length > 0) {
      errors.push(`capability ${id} omits platform(s): ${missing.join(', ')}`);
    }
    if (extra.length > 0) {
      errors.push(`capability ${id} states undeclared platform(s): ${extra.join(', ')}`);
    }

    for (const [platform, entry] of Object.entries(states)) {
      const state = entry?.state;
      if (!STATES.includes(state)) {
        errors.push(`capability ${id}/${platform}: state "${state}" is not one of ${STATES.join('/')}`);
        continue;
      }
      if (state === 'explicit-unavailable') {
        if (typeof entry.reason !== 'string' || entry.reason.length === 0) {
          errors.push(`capability ${id}/${platform}: explicit-unavailable carries no reason`);
        } else if (!legalReasons.has(entry.reason)) {
          errors.push(
            `capability ${id}/${platform}: reason "${entry.reason}" is not a UnavailableReason variant`
            + ` (derived: ${reasons.join(', ')})`,
          );
        }
      } else if (entry.reason !== undefined) {
        errors.push(`capability ${id}/${platform}: state "${state}" must not carry a reason`);
      }
    }

    // 平台绑定：绑定某原生平台的能力，在其它平台上 MUST NOT 是 supported。
    const boundTo = capability.bound_to;
    if (boundTo !== undefined) {
      if (!BOUND_TO.includes(boundTo)) {
        errors.push(`capability ${id}: bound_to "${boundTo}" is not one of ${BOUND_TO.join('/')}`);
      } else {
        for (const [platform, entry] of Object.entries(states)) {
          if (platform !== boundTo && entry?.state === 'supported') {
            errors.push(
              `capability ${id} is bound to ${boundTo} but claims supported on ${platform}`,
            );
          }
        }
      }
    }

    // 证据：source 必须有登记路径且路径存在；baseline 不得登记路径。
    const files = capability.source_files;
    if (!Array.isArray(files)) {
      errors.push(`capability ${id}: source_files must be an array`);
      continue;
    }
    if (capability.basis === 'source' && files.length === 0) {
      errors.push(`capability ${id}: basis "source" requires at least one source file`);
    }
    if (capability.basis === 'baseline' && files.length > 0) {
      errors.push(`capability ${id}: basis "baseline" must not register source files`);
    }
    if (!['source', 'baseline'].includes(capability.basis)) {
      errors.push(`capability ${id}: basis "${capability.basis}" is not one of source/baseline`);
    }
    for (const file of files) {
      registeredFiles.add(file);
      const full = path.join(root, file);
      let ok = false;
      try {
        ok = statSync(full).isFile();
      } catch {
        ok = false;
      }
      if (!ok) errors.push(`capability ${id}: registered source file "${file}" does not exist`);
    }
  }

  // ---- 源码平台分支 ⊆ 登记路径（核心门控）---------------------------------
  const branches = branchFiles ?? collectPlatformBranchFiles(root);
  for (const file of branches.keys()) {
    if (!registeredFiles.has(file)) {
      errors.push(
        `platform branch in "${file}" is not registered by any capability`
        + ' — register it or the platform assumption grows unnoticed',
      );
    }
  }

  // ---- macOS 覆盖 Standard 与 Pro -----------------------------------------
  const macos = platforms.find((platform) => platform?.id === 'macos');
  if (macos === undefined) {
    errors.push('macos platform is absent; the Standard/Pro coverage assertion has no subject');
  } else {
    const lines = Array.isArray(macos.release_lines) ? macos.release_lines : [];
    for (const required of MACOS_REQUIRED_RELEASE_LINES) {
      if (!lines.includes(required)) {
        errors.push(`macos release_lines must include "${required}"`);
      }
    }
  }

  return errors;
}

/** 读取并解析矩阵。 */
export function readMatrix(root, matrixPath = MATRIX_PATH) {
  const text = readFileSync(path.join(root, matrixPath), 'utf8');
  return JSON.parse(text);
}
