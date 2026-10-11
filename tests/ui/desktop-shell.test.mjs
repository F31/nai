// 桌面壳宿主边界的校验（ADR 0046 / `desktop-shell` 能力）。
//
// 这些检查读的是**声明的形状**：它**不能**证明配置被某个 Tauri 版本接受（ADR 0046 R1），
// 也**不能**替代构建（壳尚未构建，`tests/registry.yaml` 的 A12 保持 `pending`）。
// 它证明的是另一件事：**基线里那几条边界没有被静默放宽**——
// 最小权限没有被加宽、CSP 没有被放松、sidecar 没有失去目标三元组、
// 后台生命周期没有被绑到界面上、前端没有偷偷引入 Tauri 的文件系统或 shell 能力。
//
// 每条检查都是一个**纯函数**（输入数据，输出错误列表），因此可以用**注入样本**证明它不是恒真的：
// 「检查器能抓到 shell:allow-execute」这件事本身被断言，否则「配置通过」什么也不说明。
import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const SHELL_DIR = path.join(ROOT, 'apps/desktop/src-tauri');
const SRC_DIR = path.join(ROOT, 'apps/desktop/src');

// ——————————————————————————————————————————————————————————————
// 规格真值（写死在测试里；配置与它**逐一相等**才算通过，见 ADR 0046 D4）
// ——————————————————————————————————————————————————————————————

/** 本轮声明的权限集合。加权限必须**同时**改这里与 capabilities/default.json。 */
const DECLARED_PERMISSIONS = [
  'core:app:allow-version',
  'core:event:allow-listen',
  'core:event:allow-unlisten',
  'core:window:allow-start-dragging',
  'core:window:allow-set-title',
];

/** 具体权限的形状。`core:default` 一类**聚合权限集**在此形状下即被拒（ADR 0046 D4）。 */
const PERMISSION_SHAPE = /^core:[a-z]+:allow-[a-z0-9-]+$/;

/** 命中即离界的权限语义（§1151：前端没有任意 shell、文件系统、SQL 权限）。 */
const FORBIDDEN_PERMISSION_SEMANTICS = /(^|:)(shell|fs|sql|process)(:|$)/;

/** CSP 中禁止的通配源。`data:`（图片内联）与 `ipc:`（受保护 IPC）**不在**此列。 */
const WILDCARD_SOURCES = new Set(['*', 'http:', 'https:', 'ws:', 'wss:']);

/** 受保护 IPC 的源（Tauri 2 在 Windows 上两者都需要）。 */
const PROTECTED_IPC_SOURCES = ['ipc:', 'http://ipc.localhost'];

/** 壳**只**做这四件事（ADR 0046 D2）。 */
const SHELL_RESPONSIBILITIES = [
  'window-and-tray-lifecycle',
  'protected-ipc-host-side',
  'data-root-resolution',
  'sidecar-supervision',
];

/** 不属于壳的业务语义：命中即说明壳把内核职责吸了回去（§17.5、§181）。 */
const BUSINESS_SEMANTICS = /(sessions?|guard|approvals?|knowledge|media|utters?ance|会话|守门|审批|知识|编排)/i;

/** 壳的形态集合：三形态**共用**同一套边界（ADR 0046 D7）。 */
const FORM_FACTORS = ['windows', 'macos', 'mobile-route-b'];

/** 允许的形态差异**只**落在目标与分发上，不得落在边界上。 */
const FORM_FACTOR_DIFFS_ALLOWED = ['target', 'distribution'];

/** 需要覆盖的目标三元组：sidecar 命名规则要能对每一个都成立（§1259）。 */
const TARGET_TRIPLES = [
  'x86_64-pc-windows-msvc',
  'aarch64-apple-darwin',
  'aarch64-linux-android',
  'aarch64-apple-ios',
];

/** 文件系统绝对路径的形态（§900：数据根不得硬编码）。`http://...` 不是文件路径，不在其中。 */
const ABSOLUTE_PATH = /(^|[^:/\w])([A-Za-z]:\\|\/(?:home|Users|usr|etc|var|opt|srv)\/)/;

// ——————————————————————————————————————————————————————————————
// 纯检查函数
// ——————————————————————————————————————————————————————————————

/** 权限面：形状 → 语义 → 集合相等。任一层失败都报出来，不做短路。 */
function checkPermissions(list) {
  const errors = [];
  if (!Array.isArray(list)) return ['permissions 不是数组'];
  for (const id of list) {
    if (typeof id !== 'string') {
      errors.push(`非字符串权限项：${JSON.stringify(id)}`);
      continue;
    }
    if (!PERMISSION_SHAPE.test(id)) {
      errors.push(`权限 ${id} 不是具体权限（须形如 core:<模块>:allow-<动作>；聚合权限集一律拒绝）`);
    } else if (FORBIDDEN_PERMISSION_SEMANTICS.test(id)) {
      errors.push(`权限 ${id} 命中被禁语义（shell / 文件系统 / SQL / 进程控制）`);
    }
  }
  const declared = new Set(DECLARED_PERMISSIONS);
  const actual = new Set(list);
  for (const id of actual) if (!declared.has(id)) errors.push(`超界权限：${id}`);
  for (const id of declared) if (!actual.has(id)) errors.push(`缺少声明权限：${id}`);
  return errors;
}

/** CSP：不得含 unsafe-eval；任一指令不得使用通配源；连接源必须含受保护 IPC。 */
function checkCsp(csp) {
  const errors = [];
  if (typeof csp !== 'string' || csp.trim() === '') return ['CSP 缺失或为空'];
  if (/unsafe-eval/i.test(csp)) errors.push("CSP 含 unsafe-eval（§1151：模型输出与知识片段不得当代码执行）");
  const directives = new Map();
  for (const part of csp.split(';')) {
    const tokens = part.trim().split(/\s+/).filter(Boolean);
    if (tokens.length === 0) continue;
    const [name, ...sources] = tokens;
    directives.set(name, sources);
    for (const source of sources) {
      if (WILDCARD_SOURCES.has(source)) errors.push(`CSP 指令 ${name} 使用通配源 ${source}`);
    }
  }
  if (!directives.has('default-src')) errors.push('CSP 缺少 default-src');
  const connect = directives.get('connect-src');
  if (!connect) {
    errors.push('CSP 缺少 connect-src（受保护 IPC 需要它）');
  } else if (!connect.includes('ipc:')) {
    errors.push('CSP 的 connect-src 未包含受保护 IPC 源 ipc:');
  }
  return errors;
}

/** sidecar：逻辑名必须对应**真实存在**的二进制包，且命名规则对每个目标三元组都成立。 */
function checkSidecars(externalBin, knownPackages) {
  const errors = [];
  if (!Array.isArray(externalBin) || externalBin.length === 0) return ['bundle.externalBin 缺失或为空'];
  for (const entry of externalBin) {
    if (typeof entry !== 'string' || !/^binaries\/nai-[a-z0-9-]+$/.test(entry)) {
      errors.push(`sidecar 逻辑名不合规：${JSON.stringify(entry)}（应为 binaries/<包名>，不带目标三元组）`);
      continue;
    }
    if (/\b(x86_64|aarch64|i686)\b/.test(entry)) {
      errors.push(`sidecar 逻辑名 ${entry} 内含目标三元组——三元组由构建目标附加，不写在声明里`);
      continue;
    }
    const name = entry.slice('binaries/'.length);
    if (!knownPackages.has(name)) {
      errors.push(`sidecar ${name} 不对应任何真实的二进制包（已存在的：${[...knownPackages].sort().join(', ')}）`);
    }
  }
  return errors;
}

/** 命名规则的可执行表达：`<包名>-<目标三元组><扩展名>`。 */
function sidecarFileName(logicalName, targetTriple) {
  const name = logicalName.slice('binaries/'.length);
  const ext = targetTriple.includes('windows') ? '.exe' : '';
  return `${name}-${targetTriple}${ext}`;
}

/** 壳策略：范围、生命周期、数据根、IPC 角色、形态共用。 */
function checkPolicy(policy) {
  const errors = [];
  const responsibilities = policy?.shellResponsibilities;
  if (!Array.isArray(responsibilities)) {
    errors.push('shellResponsibilities 缺失');
  } else {
    for (const item of responsibilities) {
      if (!SHELL_RESPONSIBILITIES.includes(item)) errors.push(`壳承担了范围外的职责：${item}`);
      if (BUSINESS_SEMANTICS.test(item)) errors.push(`壳承担了业务逻辑：${item}`);
    }
    for (const item of SHELL_RESPONSIBILITIES) {
      if (!responsibilities.includes(item)) errors.push(`壳缺少职责：${item}`);
    }
  }
  if (policy?.businessLogicInShell !== 'forbidden') errors.push('businessLogicInShell 必须为 forbidden');
  if (policy?.backendsOutliveUi !== true) errors.push('backendsOutliveUi 必须为 true（§1259）');
  if (policy?.webviewKeepAliveRequiredForBackends !== false) {
    errors.push('webviewKeepAliveRequiredForBackends 必须为 false——后台不得依赖 WebView 保活（§1259）');
  }
  if (policy?.backendSupervisor !== 'independent-supervisor-or-tray') {
    errors.push('backendSupervisor 必须是 independent-supervisor-or-tray（§1259）');
  }
  if (policy?.protectedIpcIsProductionPath !== true) errors.push('protectedIpcIsProductionPath 必须为 true（§1101）');
  if (policy?.loopbackHttpRole !== 'test-adaptation') {
    errors.push('loopbackHttpRole 必须是 test-adaptation（§1101：HTTP 只用于测试适配）');
  }
  const resolver = policy?.dataRootResolvedBy;
  if (!Array.isArray(resolver) || !resolver.includes('os-path-api')) {
    errors.push('dataRootResolvedBy 必须包含 os-path-api（§900）');
  }
  if (policy?.sidecarNamingTemplate !== '{name}-{target_triple}{ext}') {
    errors.push('sidecarNamingTemplate 必须是 {name}-{target_triple}{ext}（§1259）');
  }
  const forms = policy?.formFactors;
  if (!Array.isArray(forms) || forms.join(',') !== FORM_FACTORS.join(',')) {
    errors.push(`formFactors 必须恰为 ${FORM_FACTORS.join(', ')}（三形态共用边界，ADR 0046 D7）`);
  }
  const diffs = policy?.formFactorDiffsAllowed;
  if (!Array.isArray(diffs) || diffs.join(',') !== FORM_FACTOR_DIFFS_ALLOWED.join(',')) {
    errors.push(`formFactorDiffsAllowed 必须恰为 ${FORM_FACTOR_DIFFS_ALLOWED.join(', ')}`);
  }
  return errors;
}

/** 硬编码绝对路径（§900）。排除 JSON 的键值与 URL：匹配的是 `C:\` 与类 Unix 系统目录。 */
function checkNoAbsolutePaths(text, label) {
  const errors = [];
  for (const [index, line] of String(text).split('\n').entries()) {
    if (ABSOLUTE_PATH.test(line)) errors.push(`${label}:${index + 1} 出现硬编码绝对路径：${line.trim()}`);
  }
  return errors;
}

/** 前端不得引入 Tauri 的任何运行时 API（§1151：无任意 shell / 文件系统 / SQL）。 */
function findTauriImports(source) {
  const errors = [];
  const pattern = /@tauri-apps\/[a-z-]+/g;
  for (const [index, line] of String(source).split('\n').entries()) {
    for (const hit of line.match(pattern) ?? []) {
      errors.push(`第 ${index + 1} 行引用了 ${hit}`);
    }
  }
  return errors;
}

/** 窗口标签一致性：capabilities 指向的窗口必须真的在配置里声明过。 */
function checkWindowLabels(capabilityWindows, declaredLabels) {
  const errors = [];
  if (!Array.isArray(capabilityWindows) || capabilityWindows.length === 0) return ['capabilities.windows 缺失或为空'];
  for (const id of capabilityWindows) {
    if (!declaredLabels.includes(id)) errors.push(`capabilities 指向了未声明的窗口标签：${id}`);
  }
  return errors;
}

// ——————————————————————————————————————————————————————————————
// 读真实数据
// ——————————————————————————————————————————————————————————————

const readJson = (file) => JSON.parse(readFileSync(file, 'utf8'));

const conf = readJson(path.join(SHELL_DIR, 'tauri.conf.json'));
const capabilities = readJson(path.join(SHELL_DIR, 'capabilities/default.json'));
const policy = readJson(path.join(SHELL_DIR, 'shell-policy.json'));

/** 仓库里**真实存在**的二进制包名，从各 app 的 Cargo.toml 与 src/main.rs 读出（不是测试里编的常量）。 */
function readBinaryPackageNames() {
  const names = new Set();
  for (const entry of readdirSync(path.join(ROOT, 'apps'), { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const crateDir = path.join(ROOT, 'apps', entry.name);
    let text;
    try { text = readFileSync(path.join(crateDir, 'Cargo.toml'), 'utf8'); } catch { continue; }
    // 二进制包 = 显式 `[[bin]]`，或有 `src/main.rs`（未写 `[[bin]]` 的默认二进制目标）。
    if (!/^\s*\[\[bin\]\]/m.test(text) && !existsSync(path.join(crateDir, 'src/main.rs'))) continue;
    const match = /^\s*name\s*=\s*"([^"]+)"/m.exec(text);
    if (match) names.add(match[1]);
  }
  return names;
}

const knownPackages = readBinaryPackageNames();

const sourceFiles = (() => {
  const files = [];
  const walk = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (/\.(ts|tsx)$/.test(entry.name)) files.push(full);
    }
  };
  walk(SRC_DIR);
  return files;
})();

// ——————————————————————————————————————————————————————————————
// 断言（真实数据）
// ——————————————————————————————————————————————————————————————

test('the granted permission set is exactly the declared one, in concrete form', () => {
  const errors = checkPermissions(capabilities.permissions);
  assert.deepEqual(errors, [], errors.join('\n'));
  // 逐条断言，避免「相等」被一个恰好同形的集合蒙混过去。
  assert.deepEqual([...capabilities.permissions].sort(), [...DECLARED_PERMISSIONS].sort());
});

test('no aggregate permission set and no shell / filesystem / sql / process capability', () => {
  assert.equal(capabilities.permissions.some((id) => /:(default)$/.test(id) || id === 'core:default'), false);
  assert.equal(capabilities.permissions.some((id) => FORBIDDEN_PERMISSION_SEMANTICS.test(id)), false);
  for (const id of capabilities.permissions) assert.match(id, PERMISSION_SHAPE, id);
});

test('the content security policy forbids unsafe evaluation and wildcard sources', () => {
  const errors = checkCsp(conf.app.security.csp);
  assert.deepEqual(errors, [], errors.join('\n'));
  for (const source of PROTECTED_IPC_SOURCES) {
    assert.ok(conf.app.security.csp.includes(source), `CSP 应包含受保护 IPC 源 ${source}`);
  }
});

test('capabilities point at windows the configuration actually declares', () => {
  const labels = conf.app.windows.map((w) => w.label);
  const errors = checkWindowLabels(capabilities.windows, labels);
  assert.deepEqual(errors, [], errors.join('\n'));
});

test('sidecars carry no target triple in the declaration and name real binaries', () => {
  const errors = checkSidecars(conf.bundle.externalBin, knownPackages);
  assert.deepEqual(errors, [], errors.join('\n'));
  assert.ok(knownPackages.size > 0, '未能从 apps/*/Cargo.toml 读出任何二进制包名');
});

test('the sidecar naming rule holds for every target triple we ship to', () => {
  for (const entry of conf.bundle.externalBin) {
    for (const triple of TARGET_TRIPLES) {
      const name = sidecarFileName(entry, triple);
      assert.match(name, /^nai-[a-z0-9-]+-[a-z0-9_]+-[a-z0-9-]+(-[a-z0-9]+)?(\.exe)?$/, name);
      assert.ok(name.startsWith(`${entry.slice('binaries/'.length)}-${triple}`), name);
    }
  }
  assert.equal(sidecarFileName('binaries/nai-engine', 'x86_64-pc-windows-msvc'), 'nai-engine-x86_64-pc-windows-msvc.exe');
  assert.equal(sidecarFileName('binaries/nai-engine', 'aarch64-apple-darwin'), 'nai-engine-aarch64-apple-darwin');
});

test('the shell policy keeps the shell to its four responsibilities and decouples UI lifetime', () => {
  const errors = checkPolicy(policy);
  assert.deepEqual(errors, [], errors.join('\n'));
  assert.equal(policy.businessLogicInShell, 'forbidden');
  assert.equal(policy.backendsOutliveUi, true);
  assert.equal(policy.protectedIpcIsProductionPath, true);
});

test('no hardcoded absolute path in the shell declarations', () => {
  const errors = [
    ...checkNoAbsolutePaths(readFileSync(path.join(SHELL_DIR, 'tauri.conf.json'), 'utf8'), 'tauri.conf.json'),
    ...checkNoAbsolutePaths(readFileSync(path.join(SHELL_DIR, 'capabilities/default.json'), 'utf8'), 'capabilities/default.json'),
    ...checkNoAbsolutePaths(readFileSync(path.join(SHELL_DIR, 'shell-policy.json'), 'utf8'), 'shell-policy.json'),
  ];
  assert.deepEqual(errors, [], errors.join('\n'));
});

test('no hardcoded absolute path in the desktop frontend sources', () => {
  const errors = sourceFiles.flatMap((file) =>
    checkNoAbsolutePaths(readFileSync(file, 'utf8'), path.relative(ROOT, file)));
  assert.deepEqual(errors, [], errors.join('\n'));
  assert.ok(sourceFiles.length > 0, '未扫描到任何前端源码');
});

test('the desktop frontend stays a plain web app and imports no Tauri runtime API', () => {
  const errors = sourceFiles.flatMap((file) =>
    findTauriImports(readFileSync(file, 'utf8')).map((e) => `${path.relative(ROOT, file)} ${e}`));
  assert.deepEqual(errors, [], errors.join('\n'));
});

// ——————————————————————————————————————————————————————————————
// 非空洞守卫：每条检查都必须真的能抓到违反样本。
//
// 没有这一段，「配置通过」与「检查恒真」无法区分——而恒真的检查是盲区，不是守护。
// ——————————————————————————————————————————————————————————————

test('the permission checks are not vacuous', () => {
  const overBroad = checkPermissions([...DECLARED_PERMISSIONS, 'shell:allow-execute']);
  assert.ok(overBroad.some((e) => e.includes('shell:allow-execute')), '注入 shell:allow-execute 必须被抓到');

  const aggregate = checkPermissions([...DECLARED_PERMISSIONS, 'core:default']);
  assert.ok(aggregate.some((e) => e.includes('core:default')), '注入聚合权限集必须被抓到');

  const removed = checkPermissions(DECLARED_PERMISSIONS.slice(1));
  assert.ok(removed.some((e) => e.includes('缺少声明权限')), '少授一项必须被抓到');

  const pluginFs = checkPermissions(['core:app:allow-version', 'fs:allow-read-text-file']);
  assert.ok(pluginFs.some((e) => e.includes('fs:allow-read-text-file')), '注入 fs 权限必须被抓到');
});

test('the CSP checks are not vacuous', () => {
  const evalAllowed = checkCsp("default-src 'self'; script-src 'self' 'unsafe-eval'; connect-src ipc:");
  assert.ok(evalAllowed.some((e) => e.includes('unsafe-eval')), '注入 unsafe-eval 必须被抓到');

  const wildcard = checkCsp("default-src 'self'; connect-src *");
  assert.ok(wildcard.some((e) => e.includes('通配源 *')), '注入通配源必须被抓到');

  const httpWildcard = checkCsp("default-src 'self'; connect-src http:");
  assert.ok(httpWildcard.some((e) => e.includes('通配源 http:')), '注入 http: 通配必须被抓到');

  const noIpc = checkCsp("default-src 'self'; connect-src 'self'");
  assert.ok(noIpc.some((e) => e.includes('ipc:')), '缺受保护 IPC 源必须被抓到');
});

test('the sidecar checks are not vacuous', () => {
  const unknown = checkSidecars(['binaries/nai-nonexistent'], knownPackages);
  assert.ok(unknown.some((e) => e.includes('不对应任何真实的二进制包')), '未登记的 sidecar 必须被抓到');

  const withTriple = checkSidecars(['binaries/nai-engine-x86_64-pc-windows-msvc'], knownPackages);
  assert.ok(withTriple.length > 0, '带目标三元组的逻辑名必须被拒');

  const empty = checkSidecars([], knownPackages);
  assert.ok(empty.length > 0, '空 sidecar 清单必须被拒');
});

test('the shell policy checks are not vacuous', () => {
  const drifted = checkPolicy({ ...policy, backendsOutliveUi: false });
  assert.ok(drifted.length > 0, '把后台存活性绑回界面必须被拒');

  const business = checkPolicy({ ...policy, shellResponsibilities: [...SHELL_RESPONSIBILITIES, 'session-lifecycle'] });
  assert.ok(business.length > 0, '壳承担业务逻辑必须被拒');

  const extraForm = checkPolicy({ ...policy, formFactors: [...FORM_FACTORS, 'linux-desktop'] });
  assert.ok(extraForm.length > 0, '形态集合漂移必须被拒');

  const httpProduction = checkPolicy({ ...policy, loopbackHttpRole: 'production' });
  assert.ok(httpProduction.length > 0, '把 HTTP 当生产路径必须被拒');
});

test('the absolute-path and Tauri-import checks are not vacuous', () => {
  assert.ok(checkNoAbsolutePaths('const root = "C:\\Users\\someone\\nai";', 'probe').length > 0);
  assert.ok(checkNoAbsolutePaths('const root = "/home/someone/nai";', 'probe').length > 0);
  assert.deepEqual(checkNoAbsolutePaths('const root = "http://127.0.0.1:4318";', 'probe'), [], '回环 URL 不是文件路径');

  assert.ok(findTauriImports("import { readTextFile } from '@tauri-apps/plugin-fs';").length > 0);
  assert.ok(findTauriImports("import { invoke } from '@tauri-apps/api/core';").length > 0);
  assert.deepEqual(findTauriImports("import { ControlClient } from './control.ts';"), []);
});
