/// 被测面的**集合**守卫：本机门控跑哪些 Rust 包、每个 `test --suite` 跑哪些包、CI 调用哪些
/// xtask 命令——三者之间的接线。
///
/// 为什么需要它：「本机门控到底测了什么」当前完全由**实参字面量**决定：
///
/// ```rust
/// run(root, &["test", "--workspace", "--locked", "--exclude", HARNESS])?;  // ← 覆盖 18 个包
/// run(root, &["test", "--locked", "--package", HARNESS, "--lib"])?;        // ← 覆盖  1 个包
/// ["test", "--suite", "storage"] => run(root, &["test", …, "--package", "nai-security"]),
/// ```
///
/// 而 W95 那 8 条 suite step 的 `localEvidence` **全部填了同一个 marker**
/// （`["test", "--workspace", "--locked", "--exclude", HARNESS]`）——一条 `--workspace` 的证据
/// 回答的是「**全 workspace 被跑了**」，**没有一条**回答「**这个 suite 自己的包集被跑了」。
/// 用超集冒充专属——这是同一个陷阱的**第六次**出现（见 design 决定 1）：
///
/// | 轮次 | 缺口 | 处置 |
/// | --- | --- | --- |
/// | W75 | 「来源存在」不验 ⇒ 假契约 | 注入事实（`repositoryFiles`） |
/// | W86 | 同上，另一层 | 注入事实（`readTrackedText`） |
/// | W94 | 「前置是否已达成」查不到 | 注入两张**事实表**，判据自己算 |
/// | W95 | 「本机也跑」没有凭据 | 注入 `gateSources`，marker 须逐字存在 |
/// | W96 | marker 可填 arm 自己的文本 ⇒ 自我满足 | **门控闭包区域**，由解析得出判定域 |
/// | **W97** | 一条超集 marker 一次签 8 个名 | **解析出包集合，做包含关系计算** |
///
/// 与本轮实测（`members` **19**／包 **19**／suite **9**／门控 test 命令 **2**）：把门控的
/// `--workspace --exclude HARNESS` 换成 `-p nai-storage` ⇒ **9 个 suite 全部失去覆盖、
/// 17 个成员不再被测**，而改之前的所有判据**全绿、零信号**。
///
/// ⚠️ **分类是静态声明，MUST NOT 由运行时探测得出**（铁律二）。本模块不跑 `cargo metadata`、
/// 不读磁盘——所有事实由**调用方显式注入**，判据只负责把注入的事实算成结论。
/// 遍历工作区是 W75 的 F2 根因（磁盘存在 ≠ 被跟踪）。

import {dispatchSignatures, gateRegions} from './xtask-commands.mjs';

/// suite 的**形态**闭集。域外取值 ⇒ 具名失败（W90：一个未知形态是一个标签，不是一个分类）。
/// - `inline`：包集写在 `execute()` 那条 arm 的**体**里（`run(root, &["test", …])`）；
/// - `delegated`：arm 把工作交给另一个函数（`schema::contracts`），包集在**被调函数的体**里。
export const SUITE_DRIVERS = ['inline', 'delegated'];

/// 一条 `cargo test` 命令的**覆盖形态**闭集。
/// - `workspace`：以整个 workspace 为起点（可经 `--exclude` 挖洞）；
/// - `packages`：显式给出的包列表。
export const COVERAGE_KINDS = ['workspace', 'packages'];

/// 一个 suite 命令的前缀（dispatch 签名形式）。
const SUITE_PREFIX = 'test --suite ';

/// **在册 suite 表**：一条「某 suite 的包集从哪里算出来」的独立声明。每条 `{suite, driver, …}`：
/// - `driver: 'inline'` ⇒ 从 `execute()` 那条 arm 的体解析；
/// - `driver: 'delegated'` ⇒ MUST 带 `file` 与 `fn`，从那个函数的体解析。
///
/// **逐字写死**：这份表是「有哪些 suite」的**独立声明**，不是从 `execute()` 派生的——
/// 派生出的「现有集合 == 清单」是恒真式，一个字都没说。`V1` 做两侧双向对账。
///
/// **包集本身不写在表里**（W93：同一件事 MUST NOT 两处定义）——它永远从源码算出来。
export const TEST_SUITES = [
  {
    suite: 'contracts',
    driver: 'delegated',
    file: 'xtask/src/schema.rs',
    fn: 'contracts',
  },
  {suite: 'knowledge', driver: 'inline'},
  {suite: 'provider', driver: 'inline'},
  {suite: 'scheduler', driver: 'inline'},
  {suite: 'session', driver: 'inline'},
  {suite: 'kernel', driver: 'inline'},
  {suite: 'hosts', driver: 'inline'},
  {suite: 'storage', driver: 'inline'},
  {suite: 'upgrade', driver: 'inline'},
];

/// 从 workspace 根 Cargo.toml 取 `members`。行首锚 `^` 让它不会误取 `default-members`。
/// 取不到 ⇒ 抛（调用方转成具名错误）。
export function membersFromToml(rootToml) {
  if (typeof rootToml !== 'string' || rootToml === '') {
    throw new Error('the workspace manifest was not provided');
  }
  const match = rootToml.match(/^[ \t]*members[ \t]*=[ \t]*\[([^\]]*)\]/m);
  if (!match) throw new Error('the workspace manifest declares no `members`');
  const members = [...match[1].matchAll(/"([^"]+)"/g)].map(item => item[1]);
  if (members.length === 0) throw new Error('`members` resolved to an empty list');
  return members;
}

/// 从每个成员的 Cargo.toml 取它的**包名**。缺一份 ⇒ 抛并点名那个成员
/// （「缺输入」MUST NOT 退化成「这个成员没有包名」）。
export function packageNames(memberTomls) {
  if (!(memberTomls instanceof Map)) {
    throw new Error('the member manifests were not provided as a map');
  }
  const out = new Map();
  for (const [path, text] of memberTomls) {
    if (typeof text !== 'string' || text === '') {
      throw new Error(`the member manifest ${path} was not provided`);
    }
    const match = text.match(/^[ \t]*name[ \t]*=[ \t]*"([^"]+)"/m);
    if (!match) throw new Error(`the member manifest ${path} declares no package name`);
    out.set(path, match[1]);
  }
  return out;
}

/// 剥掉 Rust 的注释（`// …` 与 `/* … */`），**不碰字符串字面量内部**（`"https://x"` 里的 `//`
/// 不是注释）。
///
/// 为什么必须有这一层：写注释时在句子里举一个 `[…]` 的形状是完全正常的写法，而本模块靠读
/// `[…]` 生活；不剥离注释就会把**散文**当成**实参**去解析——这是披着别的外衣的同一个陷阱
/// （README 里出现 `--help` 并不是 dispatch 的一项）。剥离失败比解析失败更危险，所以这里
/// 显式跟踪字符串状态，而不是用一条 `/^\s*\/\//gm` 的正则（后者会吃掉 `"http://…"` 的尾巴）。
export function stripRustComments(text) {
  let out = '';
  let i = 0;
  let inString = false;
  while (i < text.length) {
    const char = text[i];
    if (inString) {
      if (char === '\\') {
        out += char + (text[i + 1] ?? '');
        i += 2;
        continue;
      }
      if (char === '"') inString = false;
      out += char;
      i += 1;
      continue;
    }
    if (char === '"') {
      inString = true;
      out += char;
      i += 1;
      continue;
    }
    if (char === '/' && text[i + 1] === '/') {
      while (i < text.length && text[i] !== '\n') i += 1;
      continue;
    }
    if (char === '/' && text[i + 1] === '*') {
      i += 2;
      while (i < text.length && !(text[i] === '*' && text[i + 1] === '/')) i += 1;
      i += 2;
      continue;
    }
    out += char;
    i += 1;
  }
  return out;
}

/// 找出一段文本里全部**顶层** `[...]` 的体。嵌套括号计数，不匹配内含数组的寄存器/不定式写法。
export function bracketBodies(text) {
  const out = [];
  let i = text.indexOf('[');
  while (i >= 0) {
    let depth = 0;
    let j = i;
    for (; j < text.length; j += 1) {
      if (text[j] === '[') depth += 1;
      else if (text[j] === ']') {
        depth -= 1;
        if (depth === 0) break;
      }
    }
    if (depth !== 0) break;
    out.push(text.slice(i + 1, j));
    i = text.indexOf('[', j + 1);
  }
  return out;
}

/// 按**顶层**逗号切成一个数组的各个项（字符串内部的逗号不分）。
function splitTopLevelItems(inner) {
  const parts = [];
  let depth = 0;
  let current = '';
  for (const ch of inner) {
    if (ch === '[' || ch === '(' || ch === '{') depth += 1;
    else if (ch === ']' || ch === ')' || ch === '}') depth -= 1;
    if (ch === ',' && depth === 0) {
      parts.push(current);
      current = '';
    } else {
      current += ch;
    }
  }
  if (current.trim() !== '') parts.push(current);
  return parts.map(part => part.trim()).filter(part => part !== '');
}

/// 把数组的各项归一成 `{kind, value}`。三种 kind：
/// - `string`：`"…"` 字面量；
/// - `identifier`：裸标识符（`HARNESS`），须由 `identifiers` 解析；
/// - `unknown`：别的写法（表达式、宏调用等）——**保守报错**，不许静默跳过。
export function topLevelItems(inner) {
  return splitTopLevelItems(inner).map(raw => {
    const str = raw.match(/^"(.*)"$/s);
    if (str) return {kind: 'string', value: str[1], raw};
    if (/^[A-Za-z_][A-Za-z0-9_]*$/.test(raw)) return {kind: 'identifier', value: raw, raw};
    return {kind: 'unknown', value: raw, raw};
  });
}

/// 把一段源码里的每个顶层数组解析成一条 `cargo test` 覆盖（非 `test` 的数组 ⇒ 跳过）。
/// 返回 `{coverages, unresolvable}`——后者是**点了名**的不可哈希项，绝不静默丢弃。
export function coveragesOf(text, identifiers) {
  const coverages = [];
  const unresolvable = [];
  for (const inner of bracketBodies(stripRustComments(text))) {
    const items = topLevelItems(inner);
    if (items.length === 0 || items[0].kind !== 'string') continue;
    if (items[0].value !== 'test') continue;
    const values = [];
    let stuck = null;
    for (const item of items) {
      if (item.kind === 'string') {
        values.push(item.value);
      } else if (item.kind === 'identifier') {
        if (identifiers.has(item.value)) values.push(identifiers.get(item.value));
        else {
          stuck = item.value;
          break;
        }
      } else {
        stuck = item.raw;
        break;
      }
    }
    if (stuck !== null) {
      unresolvable.push({command: '[test, …]', token: stuck});
      continue;
    }
    coverages.push(testCoverage(values));
  }
  return {coverages, unresolvable};
}

/// 把一条 `["test", …]` 实参数组解析成**覆盖形态**。首项不是 `test` ⇒ 返回 `null`。
/// `--locked`／`--lib`／`--tests` 等旗标不影响集合（它们约束的是**目标类型**，不是**包**）。
export function testCoverage(values) {
  if (values[0] !== 'test') return null;
  let workspace = false;
  const packages = [];
  const excludes = [];
  for (let i = 1; i < values.length; i += 1) {
    const value = values[i];
    if (value === '--workspace') workspace = true;
    else if (value === '--exclude') {
      i += 1;
      excludes.push(values[i]);
    } else if (value === '-p' || value === '--package') {
      i += 1;
      packages.push(values[i]);
    }
  }
  if (workspace) return {kind: 'workspace', packages: [], excludes, raw: values.join(' ')};
  return {kind: 'packages', packages, excludes: [], raw: values.join(' ')};
}

/// 把一个覆盖形态**结算**成一个具体的包名集合（`workspace` 形态才需要全集与排除项）。
export function resolveCoverage(coverage, packages) {
  const all = new Set(packages);
  if (coverage.kind === 'packages') return new Set(coverage.packages);
  for (const exclude of coverage.excludes) all.delete(exclude);
  return all;
}

/// 从 `execute()` 里一条 `["test", "--suite", X] => …` 臂取出它的**体**。
/// 体可能是 `run(root, &[…])`（圆括号）也可能是 `{ … }`（花括号）。取不到 ⇒ 抛。
export function suiteArmBody(harnessText, suite) {
  const anchor = new RegExp(
    `\\[\\s*"test"\\s*,\\s*"--suite"\\s*,\\s*"${suite}"\\s*\\]\\s*=>`,
  );
  const match = anchor.exec(harnessText);
  if (!match) throw new Error(`no dispatch arm for the suite "${suite}"`);
  const after = harnessText.slice(match.index + match[0].length);
  const openBrace = after.search(/[({]/);
  if (openBrace < 0) throw new Error(`the arm of the suite "${suite}" has no body`);
  const open = match.index + match[0].length + openBrace;
  const delimiter = harnessText[open] === '(' ? ')' : '}';
  let depth = 0;
  for (let i = open; i < harnessText.length; i += 1) {
    if (harnessText[i] === harnessText[open]) depth += 1;
    else if (harnessText[i] === delimiter) {
      depth -= 1;
      if (depth === 0) return harnessText.slice(open + 1, i);
    }
  }
  throw new Error(`the arm of the suite "${suite}" carries an unbalanced body`);
}

/// 从一段源码里定位一个函数的体。找不到 ⇒ 抛。
export function functionBody(text, name) {
  const found = new RegExp(`(?:pub )?fn ${name}\\s*\\(`).exec(text);
  if (!found) throw new Error(`no \`fn ${name}(\` in the provided source`);
  const open = text.indexOf('{', found.index);
  if (open < 0) throw new Error(`\`fn ${name}\` has no body`);
  let depth = 0;
  for (let i = open; i < text.length; i += 1) {
    if (text[i] === '{') depth += 1;
    else if (text[i] === '}') {
      depth -= 1;
      if (depth === 0) return text.slice(open + 1, i);
    }
  }
  throw new Error(`\`fn ${name}\` carries an unbalanced body`);
}

/// 从 CI 执行面的文本里抽出**全部** `cargo xtask <…>` 命令，归一成命令签名。
/// shell 语法成分（`2>&1`、`| tee …`、`&&`、`;`、行续符）必须被剥掉，否则一条合法命令
/// 会以**不存在的签名**落到 dispatch 台账上对账，制造假阳性。
export function ciXtaskCommands(workflowText) {
  const out = [];
  for (const match of workflowText.matchAll(/cargo\s+xtask\s+([^\n]+)/g)) {
    let command = match[1]
      .replace(/\s*2>&1[\s\S]*$/, '')
      .replace(/\s*\|\s*tee[\s\S]*$/, '')
      .replace(/\s*&&[\s\S]*$/, '')
      .replace(/;\s*[\s\S]*$/, '')
      .replace(/\s+/g, ' ')
      .trim();
    if (command === '') continue;
    out.push(command);
  }
  return out;
}

/// 登记表自身的一致性。与 `siteShapeErrors()`／`registryShapeErrors()` 同一条纪律：
/// 挡的是**未来某次合法编辑**造成的**静默失去覆盖**，而不是当前的错。
export function suiteRegistryShapeErrors(registered = TEST_SUITES) {
  const errors = [];
  const seen = new Set();
  for (const entry of registered) {
    const name = entry?.suite;
    if (typeof name !== 'string' || name === '') {
      errors.push(`a suite entry carries no suite name: ${JSON.stringify(entry)}`);
      continue;
    }
    if (seen.has(name)) errors.push(`the suite "${name}" is declared twice — one is redundant`);
    seen.add(name);
    if (!SUITE_DRIVERS.includes(entry.driver)) {
      errors.push(`the suite "${name}" names driver "${entry.driver}", which is not a suite driver — a driver outside the closed set is a label, not a classification`);
      continue;
    }
    if (entry.driver === 'delegated') {
      if (typeof entry.file !== 'string' || entry.file === '') {
        errors.push(`the suite "${name}" delegates but names no file, so its packages cannot be computed`);
      }
      if (typeof entry.fn !== 'string' || entry.fn === '') {
        errors.push(`the suite "${name}" delegates but names no function, so its packages cannot be computed`);
      }
    } else {
      // 双向：`inline` MUST NOT 同时声称 delegation——否则「包集从哪里算」没有单一回答（W88 同族）。
      if (entry.file !== undefined || entry.fn !== undefined) {
        errors.push(`the suite "${name}" is inline yet also names a delegation target — the packages cannot be computed from two places`);
      }
    }
  }
  return errors;
}

/// 主判据。入参**显式注入**，缺任一项即报错——一条会因入参缺失而恒真的断言比没有断言更坏。
///
/// - `rootToml`：workspace 根 `Cargo.toml` 全文（`members` 的真源）；
/// - `memberTomls`：`Map<path, text>`，每个成员自己的 `Cargo.toml`（包名的真源）；
/// - `harnessText`：`xtask/src/lib.rs` 全文（dispatch 与门控闭包的真源）；
/// - `gateSources`：`Map<path, text>`，**至少**包含被 delegated 的 suite 所指的文件；
/// - `workflowText`：CI 执行面（`.github/workflows/foundation.yml`）全文；
/// - `suiteRegistry`：默认为 `TEST_SUITES`；
/// - `identifiers`：`Map<string, string>`，源码里的常量标识符 → 它的值。默认只派生 `HARNESS`。
export function coverageErrors(inputs) {
  const errors = [];
  const {
    rootToml,
    memberTomls,
    harnessText,
    gateSources,
    workflowText,
    suiteRegistry = TEST_SUITES,
    identifiers = new Map(),
  } = inputs ?? {};

  if (typeof harnessText !== 'string' || harnessText === '') {
    return ['the harness source was not provided, so no suite can be judged'];
  }
  if (typeof workflowText !== 'string' || workflowText === '') {
    return ['the CI workflow was not provided, so its commands cannot be judged'];
  }
  if (!(gateSources instanceof Map)) {
    return ['the gate sources were not provided as a map, so delegated suites cannot be judged'];
  }
  if (!(memberTomls instanceof Map)) {
    return ['the member manifests were not provided as a map, so no package name can be judged'];
  }

  errors.push(...suiteRegistryShapeErrors(suiteRegistry));

  // ---------- 事实：members、包名、标识符 ----------
  let members = null;
  try {
    members = membersFromToml(rootToml);
  } catch (error) {
    errors.push(`the workspace members could not be read: ${error.message}`);
  }
  let namesByPath = null;
  try {
    namesByPath = packageNames(memberTomls);
  } catch (error) {
    errors.push(`the package names could not be read: ${error.message}`);
  }

  const known = new Map(identifiers);
  if (namesByPath !== null) {
    // `HARNESS` 是源码里的 const（= `env!("CARGO_PKG_NAME")`）⇒ 从 xtask 自己的包名派生，
    // **不写死**（W93：同一件事 MUST NOT 两处定义）。
    if (namesByPath.has('xtask') && !known.has('HARNESS')) {
      known.set('HARNESS', namesByPath.get('xtask'));
    }
  }
  let packages = null;
  if (namesByPath !== null && members !== null) {
    packages = new Set(members.map(member => namesByPath.get(member)).filter(Boolean));
    const missing = members.filter(member => !namesByPath.has(member));
    for (const member of missing) {
      errors.push(`the workspace member ${member} has no manifest among the provided ones — its package name is unknown, so nothing about it can be judged`);
    }
  }

  // ---------- V1：suite 表 ↔ dispatch 双向闭世界 ----------
  let signatures = null;
  try {
    signatures = new Set(dispatchSignatures(harnessText));
  } catch (error) {
    errors.push(`the command dispatch could not be parsed, so no suite can be judged: ${error.message}`);
  }
  const dispatched = signatures === null
    ? null
    : new Set(
      [...signatures]
        .filter(signature => signature.startsWith(SUITE_PREFIX))
        .map(signature => signature.slice(SUITE_PREFIX.length)),
    );

  const declared = new Set(suiteRegistry.map(entry => entry.suite));
  if (dispatched !== null) {
    if (dispatched.size === 0) {
      errors.push('the harness dispatches no suite — an empty suite set makes the closure vacuously true');
    }
    if (declared.size === 0) {
      errors.push('the suite registry is empty — an empty registry makes the closure vacuously true');
    }
    for (const suite of dispatched) {
      if (!declared.has(suite)) {
        errors.push(`the harness dispatches the suite "${suite}", which nobody declares — an undeclared suite is not judged`);
      }
    }
    for (const entry of suiteRegistry) {
      if (!dispatched.has(entry.suite)) {
        errors.push(`the suite "${entry.suite}" is declared but the harness does not dispatch it — a registered suite that vanished is a judgement that silently disappeared`);
      }
    }
  }

  // ---------- V2/V3：逐个 suite 算出它的包集 ----------
  const suitePackages = new Map();
  if (packages !== null) {
    for (const entry of suiteRegistry) {
      if (!SUITE_DRIVERS.includes(entry.driver)) continue;
      let body = null;
      try {
        body = entry.driver === 'inline'
          ? suiteArmBody(harnessText, entry.suite)
          : (() => {
            const source = gateSources.get(entry.file);
            if (typeof source !== 'string' || source === '') {
              throw new Error(`${entry.file} was not provided for judgement`);
            }
            return functionBody(source, entry.fn);
          })();
      } catch (error) {
        errors.push(`the suite "${entry.suite}" could not be read, so the packages it tests are unknown: ${error.message}`);
        continue;
      }
      const {coverages, unresolvable} = coveragesOf(body, known);
      for (const stuck of unresolvable) {
        errors.push(`the suite "${entry.suite}" names ${stuck.token}, which cannot be resolved to a package — an unparsed operand must not be treated as an empty set`);
      }
      // **空态 MUST 被显式断言，MUST NOT 静默跳过**（W92）：一个 driver 被标错形态时
      // （例如 `contracts` 其实是 `schema::contracts(root)`，却被标成 inline），它的体里
      // 压根没有 `["test", …]` ⇒ 包集算成空集 ⇒ V4 对空集**恒真**而没有信号。
      if (coverages.length === 0) {
        errors.push(`the suite "${entry.suite}" (${entry.driver}) carries no \`["test", …]\` command — either its driver is mislabelled or it tests nothing, and neither may pass silently`);
        continue;
      }
      const set = new Set();
      for (const coverage of coverages) {
        for (const value of coverage.packages) {
          set.add(value);
          // V3：`-p` 的每个值 MUST 是一个真实成员的包名（改名的裂缝从这里收口）。
          if (!packages.has(value)) {
            errors.push(`the suite "${entry.suite}" tests the package "${value}", which is not a package of this workspace — a phantom package reference is a name that drifted`);
          }
        }
      }
      if (set.size === 0) {
        // 同上：空包集让 V4 恒真 ⇒ MUST 显式处置，不许滑过去。
        errors.push(`the suite "${entry.suite}" (${entry.driver}) selects no package, so every coverage claim about it is vacuously true`);
        continue;
      }
      suitePackages.set(entry.suite, set);
    }
  }

  // ---------- V4/V5：门控闭包里 test 命令的覆盖 ----------
  const gateByRegion = [];
  try {
    for (const region of gateRegions(harnessText)) {
      gateByRegion.push({name: region.name, ...coveragesOf(region.text, known)});
    }
  } catch (error) {
    errors.push(`the local gate closure could not be located, so nothing about the tested set can be judged: ${error.message}`);
  }

  if (packages !== null) {
    const all = new Set(packages);
    const commandSet = [];
    const union = new Set();
    for (const region of gateByRegion) {
      for (const stuck of region.unresolvable) {
        errors.push(`the gate closure (${region.name}) names ${stuck.token}, which cannot be resolved to a package — an unparsed operand must not be treated as an empty set`);
      }
      for (const coverage of region.coverages) {
        const set = resolveCoverage(coverage, all);
        commandSet.push({coverage, set});
        for (const value of set) union.add(value);
      }
    }
    if (commandSet.length === 0) {
      errors.push('the local gate closure runs no test command — an empty command set makes every coverage claim vacuous');
    }

    // V4：每个 suite 的包集 MUST 被**单一条**门控命令覆盖——不许用若干条的并集拼出来。
    for (const [suite, set] of suitePackages) {
      const covering = commandSet.filter(item => [...set].every(value => item.set.has(value)));
      if (covering.length === 0) {
        errors.push(`the suite "${suite}" tests ${set.size === 0 ? 'no package at all' : `"${[...set].sort().join('", "')}"`}, and no single command in the local gate closure covers all of it — coverage by a union of commands is not coverage`);
      }
    }

    // V5：每个 workspace 成员 MUST 被这些命令的**并集**覆盖。
    for (const member of members) {
      const name = namesByPath.get(member);
      if (name && !union.has(name)) {
        errors.push(`the workspace member ${member} (package "${name}") is tested by no command in the local gate closure — member added but never run is a test surface that quietly shrank`);
      }
    }

    // ---------- 非空洞：两种覆盖形态都必须真的出现，否则某一格的规则对空集恒真 ----------
    const kinds = new Set(commandSet.map(item => item.coverage.kind));
    for (const kind of COVERAGE_KINDS) {
      if (!kinds.has(kind)) {
        errors.push(`no test command in the gate closure is of the "${kind}" kind, so its rule is vacuous`);
      }
    }
  }

  // ---------- V6：CI 的每条 xtask 命令 MUST 在本机命令面在册 ----------
  if (signatures !== null) {
    const commands = ciXtaskCommands(workflowText);
    if (commands.length === 0) {
      errors.push('the CI workflow calls no xtask command — an empty command set makes this check vacuous');
    }
    for (const command of commands) {
      if (!signatures.has(command)) {
        errors.push(`the CI workflow calls "cargo xtask ${command}", which the harness does not dispatch — a CI step calling a command that does not exist`);
      }
    }
  }

  // ---------- 非空洞：两种 suite 形态都必须被用到 ----------
  const drivers = new Set(suiteRegistry.map(entry => entry.driver));
  for (const driver of SUITE_DRIVERS) {
    if (!drivers.has(driver)) {
      errors.push(`no suite is of the "${driver}" driver, so its rule is vacuous`);
    }
  }

  return errors;
}
