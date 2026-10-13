/// `package.json` 的**脚本执行面**：每个脚本 MUST 有一个具名处置——或引出一处**真调用**，
/// 或落入**成因闭集**并逐格付代价。
///
/// 为什么需要它：仓库里有 3 份 `package.json`、13 个脚本。每个脚本都声称做了某件事，但
/// **没有任何判据**回答「**谁调用它**」。声明面（`scripts` 表）可枚举、可读 JSON；执行面
/// （CI 的 `run:` 行、`frontend-gate.mjs` 驱动表的一项、文档里给人抄的一行）完全不在判据里。
///
/// 实测（W98 三个 Mutation，改真文件 + 真跑现有 6 个 portable 套件 + 备份拷回 + `sha256` 还原）：
/// - 往根 `package.json` 加一个脚本 `"typecheck:fast": "node scripts/nowhere.mjs"`
///   ⇒ **158 passed / 0 failed**——一个新的孤儿，零信号；
/// - 把 `frontend-gate.mjs` 的 `STEPS` 里 `['apps/desktop', 'test']` 改成不存在的 `'tests'`
///   ⇒ **158 passed / 0 failed**——只有真跑前端门控才会以 `FRONTEND_SCRIPT_UNSUPPORTED` 红；
/// - 给脚本加自引用 `"test:loop": "pnpm test:loop"` ⇒ **158 passed / 0 failed**。
///
/// 同一母题的**第七次**出现（`repositoryFiles` → `readTrackedText` → W94 两张事实表 →
/// W95 `gateSources` → W96 门控闭包区域 → W97 包集合 → 本轮脚本执行面）。每轮换一层对象，
/// 而对象自身带的陷阱照旧：
/// 1. **判定单元 MUST 是 `(包目录, 脚本名)` 的配对，MUST NOT 是裸脚本名**（W91：「同名」≠「同一个」）
///    ——`typecheck` 在三个包里同名、`build` 在两个包里同名，按名字看会合成一条。
/// 2. **注释不是引用**（W97 的 `stripRustComments` 换一层）：`workflow-steps.mjs` 的文档注释里
///    写着 `pnpm spec:validate`、`tests/registry.yaml` 的 `notes` 里写着 `pnpm typecheck/build`
///    ——它们是**解释**。实测：原始正则命中 4 行，剥注释后**只有 1 行**是真引用。
/// 3. **文档不是调用**（本轮的核心分离）：`.md` 代码块里的 `pnpm test:control-smoke` 是给**人**抄的，
///    它不执行任何东西。把文档当消费者，两个从不运行的脚本会永久绿灯。
/// 4. **自引用 MUST NOT 充当消费者**：`"test:loop": "pnpm test:loop"` 是一条真引用，但它指向自己
///    ——恒真的自我满足（MEMORY §6）。
///
/// ⚠️ **分类是静态声明，MUST NOT 由运行时探测得出**（铁律二）：`pnpm` 在宿主上不可用，而
/// 「能不能跑」是**宿主事实**——换一台机器会对同一个仓库得出不同分类。分类是人的主张，逐条在册；
/// 判据负责验「主张有没有付代价」。

/// 驱动表宿主：前端门控的「跑什么」由它读各包 `package.json` 决定。
export const DRIVER = 'scripts/frontend-gate.mjs';

/// 根包目录的键。
export const ROOT_DIR = '.';

/// workspace 声明文件——**包目录**的闭世界由它派生。
export const WORKSPACE_FILE = 'pnpm-workspace.yaml';

/// CI 执行面（真调用可能出现的地方之一）。
export const CI_WORKFLOW = '.github/workflows/foundation.yml';

/// **成因闭集**（三格）。域外取值 ⇒ 具名失败（W90：一个未知成因是一个标签，不是一个理由）。
///
/// **三格的判定域两两不同**——这是它们可区分的前提（否则分类退化成不可判定的标签）：
/// - `covered-by-driver`：代价落在**驱动表的解析结果**上（`{dir, script}` 必须真的是一条 STEPS 项）；
/// - `manual-alias`：代价落在**源码**里（逐字给出等价入口的落点，例如 `frontend.rs` 的 `SCRIPT` 常量）；
/// - `manual`：代价落在**文档**里（门控与判据都执行不了的东西，其可用性只能由文档背书）。
///
/// **不**拆「冒烟」与「开发服务器」两格：两者的证据形态都是「文档里的一行」，拆开会让
/// 一次合法的改签（把某个脚本从 `manual` 改成另一格）不被任何判据发现 ⇒ 两格不可区分 ⇒
/// 分类变成标签。合并是**按可判定性**做的，不是按语义做的。
export const ORPHAN_CAUSES = ['covered-by-driver', 'manual-alias', 'manual'];

/// **处置**闭集。
export const DISPOSITIONS = ['invoked', 'orphan'];

/// **消费者形态**闭集。
export const CONSUMER_KINDS = ['driver', 'pnpm'];

/// **在册脚本表**：一条「某个 `(包目录, 脚本名)` 归谁处置」的独立声明。
///
/// **逐字写死**：这份表是「有哪些脚本、各自什么处置」的**独立声明**，不是从 `package.json`
/// 派生的——派生出的「现有集合 == 清单」是恒真式，一个字都没说。
///
/// 每条 `{dir, script, disposition, …}`：
/// - `disposition: 'invoked'` MUST 带 `consumer`（`{kind: 'driver', step: {dir, script}}`
///   或 `{kind: 'pnpm', file, marker}`）；
/// - `disposition: 'orphan'` MUST 带 `cause`（∈ `ORPHAN_CAUSES`）与该格自己的代价字段。
///
/// ⚠️ **`.md` 里的命令是「说明」，不是「调用」**：`docs/control.md` 与 `docs/engine.md` 里那两行
/// 只作为 `manual` 的证据形态出现，MUST NOT 被读成消费者。
export const SCRIPT_DISPOSITIONS = [
  // ---------- invoked（7）----------
  {
    dir: ROOT_DIR,
    script: 'spec:validate',
    disposition: 'invoked',
    consumer: {kind: 'pnpm', file: CI_WORKFLOW, marker: 'pnpm spec:validate'},
  },
  {
    dir: ROOT_DIR,
    script: 'test:ui',
    disposition: 'invoked',
    consumer: {kind: 'driver', step: {dir: ROOT_DIR, script: 'test:ui'}},
  },
  {
    dir: 'packages/contracts-ts',
    script: 'typecheck',
    disposition: 'invoked',
    consumer: {kind: 'driver', step: {dir: 'packages/contracts-ts', script: 'typecheck'}},
  },
  {
    dir: 'packages/contracts-ts',
    script: 'build',
    disposition: 'invoked',
    consumer: {kind: 'driver', step: {dir: 'packages/contracts-ts', script: 'build'}},
  },
  {
    dir: 'apps/desktop',
    script: 'typecheck',
    disposition: 'invoked',
    consumer: {kind: 'driver', step: {dir: 'apps/desktop', script: 'typecheck'}},
  },
  {
    dir: 'apps/desktop',
    script: 'build',
    disposition: 'invoked',
    consumer: {kind: 'driver', step: {dir: 'apps/desktop', script: 'build'}},
  },
  {
    dir: 'apps/desktop',
    script: 'test',
    disposition: 'invoked',
    consumer: {kind: 'driver', step: {dir: 'apps/desktop', script: 'test'}},
  },

  // ---------- orphan（6）----------
  {
    dir: ROOT_DIR,
    script: 'typecheck',
    disposition: 'orphan',
    cause: 'covered-by-driver',
    // 根包的 `typecheck` 体是 `pnpm --recursive typecheck` ⇒ 覆盖**所有**包的 `typecheck`。
    // 「脚本名一致」不是装饰：若这里写成 `build` 项，判据 MUST 红——不让「委托」变成万能通行证。
    coveredBy: [
      {dir: 'packages/contracts-ts', script: 'typecheck'},
      {dir: 'apps/desktop', script: 'typecheck'},
    ],
  },
  {
    dir: ROOT_DIR,
    script: 'build',
    disposition: 'orphan',
    cause: 'covered-by-driver',
    coveredBy: [
      {dir: 'packages/contracts-ts', script: 'build'},
      {dir: 'apps/desktop', script: 'build'},
    ],
  },
  {
    dir: ROOT_DIR,
    script: 'check:frontend',
    disposition: 'orphan',
    cause: 'manual-alias',
    // 等价入口的落点：门控真正发起的是这个脚本路径，不是 `pnpm check:frontend`。
    equivalentTo: {file: 'xtask/src/frontend.rs', marker: 'pub const SCRIPT: &str = "scripts/frontend-gate.mjs";'},
  },
  {
    dir: ROOT_DIR,
    script: 'test:control-smoke',
    disposition: 'orphan',
    cause: 'manual',
    docEvidence: {file: 'docs/control.md', marker: 'pnpm test:control-smoke'},
  },
  {
    dir: ROOT_DIR,
    script: 'test:engine-smoke',
    disposition: 'orphan',
    cause: 'manual',
    docEvidence: {file: 'docs/engine.md', marker: 'pnpm test:engine-smoke'},
  },
  {
    dir: 'apps/desktop',
    script: 'dev',
    disposition: 'orphan',
    cause: 'manual',
    docEvidence: {file: 'docs/engineering.md', marker: 'pnpm --filter @nai/desktop dev'},
  },
];

/// 台账的键。`\u0000` 是分隔符：`(dir, script)` 是配对，不是拼串。
export function dispositionKey(dir, script) {
  return `${dir}\u0000${script}`;
}

/// 去掉 `//` 与 `/* */` 注释，**并跟踪字符串状态**（`"https://x"` 里的 `//` 不是注释）。
/// 不剥注释时，一句注释里举的例子（`pnpm spec:validate`）会被当成真调用——W97 实测过同形的一次。
export function stripSlashComments(text) {
  let out = '';
  let state = 'code';
  for (let i = 0; i < text.length; i += 1) {
    const c = text[i];
    const n = text[i + 1];
    if (state === 'code') {
      if (c === '/' && n === '/') {
        state = 'line';
        i += 1;
        continue;
      }
      if (c === '/' && n === '*') {
        state = 'block';
        i += 1;
        continue;
      }
      if (c === '"' || c === "'" || c === '`') {
        state = c;
        out += c;
        continue;
      }
      out += c;
      continue;
    }
    if (state === 'line') {
      if (c === '\n') {
        state = 'code';
        out += c;
      }
      continue;
    }
    if (state === 'block') {
      if (c === '*' && n === '/') {
        state = 'code';
        i += 1;
      } else if (c === '\n') {
        out += c; // 保行，让行号可对照
      }
      continue;
    }
    // 字符串里
    if (c === '\\') {
      out += c + (n ?? '');
      i += 1;
      continue;
    }
    out += c;
    if (c === state) state = 'code';
  }
  return out;
}

/// 去掉 YAML 的 `#` 注释（引号内的 `#` 不算）。
export function stripYamlComments(text) {
  return text
    .split('\n')
    .map(line => {
      let quote = null;
      for (let i = 0; i < line.length; i += 1) {
        const c = line[i];
        if (quote) {
          if (c === quote) quote = null;
          continue;
        }
        if (c === '"' || c === "'") {
          quote = c;
          continue;
        }
        if (c === '#' && (i === 0 || /\s/.test(line[i - 1]))) return line.slice(0, i);
      }
      return line;
    })
    .join('\n');
}

/// 按扩展名选剥注释方式。`.md` 原样（文档不是代码，不该被剥）。
export function stripComments(text, file = '') {
  if (file.endsWith('.yaml') || file.endsWith('.yml')) return stripYamlComments(text);
  if (file.endsWith('.md')) return text;
  return stripSlashComments(text);
}

/// 解析 `packageFiles`（`Map<包目录, package.json 文本>`）⇒ `Map<dir, {name, scripts}>`。
/// JSON 解析失败 ⇒ 抛（调用方转成具名错误），MUST NOT 当成「没有脚本」。
export function packageScripts(packageFiles) {
  if (!(packageFiles instanceof Map)) throw new Error('package files were not provided as a map');
  const out = new Map();
  for (const [dir, text] of packageFiles) {
    let parsed;
    try {
      parsed = JSON.parse(text);
    } catch (error) {
      throw new Error(`${dir}/package.json could not be parsed, so its scripts cannot be judged: ${error.message}`);
    }
    const scripts = parsed.scripts;
    if (scripts === undefined) {
      out.set(dir, {name: parsed.name, scripts: {}});
      continue;
    }
    if (scripts === null || typeof scripts !== 'object' || Array.isArray(scripts)) {
      throw new Error(`${dir}/package.json carries a non-object "scripts"`);
    }
    out.set(dir, {name: parsed.name, scripts});
  }
  return out;
}

/// 从驱动表源码解析出 `STEPS` ⇒ `[{dir, script}]`。不可解析 ⇒ 抛。
///
/// 判定域由**解析**得出，不是一个全文 `.includes()`：否则「被驱动表覆盖」可以填文件里随便一段文本
/// （W96 的教训：marker 可填自己的文本 ⇒ 自我满足）。
export function driverSteps(driverText) {
  if (typeof driverText !== 'string' || driverText === '') {
    throw new Error('the driver source was not provided');
  }
  const block = driverText.match(/const\s+STEPS\s*=\s*\[([\s\S]*?)\];/);
  if (!block) throw new Error('the driver carries no `const STEPS = [...]`');
  const steps = [];
  const itemRe = /\[\s*'([^']*)'\s*,\s*'([^']*)'\s*\]/g;
  for (const match of block[1].matchAll(itemRe)) {
    steps.push({dir: match[1], script: match[2]});
  }
  if (steps.length === 0) throw new Error('the driver step table is empty');
  return steps;
}

/// 从驱动表源码解析出门控**认得的可执行名**。不可解析 ⇒ 抛。
///
/// 这是 `frontend-gate.mjs` 运行时 `resolveInvocation` 拒绝的**静态镜像**：只有真跑门控才会
/// 撞上的错误，在这里变成一条可复核的判据。
export function driverExecutables(driverText) {
  const match = driverText.match(/KNOWN_EXECUTABLES\s*=\s*new\s+Set\(\s*\[([^\]]*)\]/);
  if (!match) throw new Error('the driver declares no `KNOWN_EXECUTABLES` set');
  const names = [...match[1].matchAll(/'([^']+)'/g)].map(item => item[1]);
  if (names.length === 0) throw new Error('the driver accepts no executable name at all');
  return names;
}

/// 从驱动表源码解析出门控**不实现**的 shell 语法记号。不可解析 ⇒ 抛。
export function driverShellSyntax(driverText) {
  const match = driverText.match(/SHELL_SYNTAX\s*=\s*\[([^\]]*)\]/);
  if (!match) throw new Error('the driver declares no `SHELL_SYNTAX` list');
  const tokens = [...match[1].matchAll(/'([^']*)'/g)].map(item => item[1]);
  if (tokens.length === 0) throw new Error('the driver forbids no shell syntax at all');
  return tokens;
}

/// 把一个 `(dir, script)` 的**脚本体**拆成子命令（`&&` 分隔）。缺脚本 ⇒ 返回 `null`。
function scriptBody(packages, dir, script) {
  if (!(packages instanceof Map)) return null;
  const entry = packages.get(dir);
  if (!entry) return null;
  const body = entry.scripts[script];
  return typeof body === 'string' ? body : null;
}

/// 解析 `pnpm`／`npm`／`yarn` 引用 ⇒ `[{file, line, targets: [{dir, script}], script, raw}]`。
///
/// 归因规则（`--recursive` 与 `--filter` 的语义在此落实）：
/// - `pnpm --recursive X` ⇒ 目标 = **所有**包；
/// - `pnpm --filter P X` ⇒ 目标 = 包名 `P` 对应的目录（包名找不到 ⇒ 归因失败，由调用方具名）；
/// - `pnpm X`（无 flag）⇒ 目标 = **根包**；
/// - `pnpm install` / `exec` / `dlx` … 是**子命令**，不是脚本 ⇒ 跳过。
///
/// **自引用 MUST NOT 充当消费者**：引用落在**被消费包自己的** `package.json` 里时跳过
/// （`"test:loop": "pnpm test:loop"` 是恒真的自我满足）。
export function pnpmReferences(consumerTexts, packages) {
  if (!(consumerTexts instanceof Map)) throw new Error('consumer texts were not provided as a map');
  const nameToDir = new Map();
  for (const [dir, entry] of packages) nameToDir.set(entry.name, dir);

  // **只**跳过真正的子命令。`test`／`start`／`build` **不在此列**：npm 把它们定义成
  // `scripts.<name>` 的别名（`pnpm build` 就是跑 `scripts.build`），把它们当子命令会**漏掉引用**
  // ——假阴性是危险方向（MEMORY §6：假阳性与假阴性不对称 ⇒ 永远选安全方向）。
  const subcommands = new Set([
    'install', 'exec', 'dlx', 'add', 'remove', 'why', 'list', 'config', 'outdated',
    'update', 'publish', 'link', 'store', 'licenses', 'audit', 'patch', 'deploy',
    'import', 'init', 'setup', 'help',
  ]);

  const refs = [];
  const unresolved = [];
  for (const [file, rawText] of consumerTexts) {
    const text = stripComments(rawText, file);
    const ownDir = file === 'package.json' ? ROOT_DIR : file.endsWith('/package.json') ? file.slice(0, -'/package.json'.length) : null;
    text.split('\n').forEach((line, index) => {
      // **非贪婪**：一行里可以有多条命令（`pnpm a && pnpm b`），贪婪的 `[^\n]*` 会把第二条
      // 吞进第一条的 token 里 ⇒ 第二条引用**静默消失**（假阴性）。在 `&&`／`||`／`|`／`;` 处停下。
      for (const match of line.matchAll(/\b(?:pnpm|npm|yarn)\s+([^\n&|;]*)/g)) {
        // 这些引用可能住在 JSON／YAML 里（`"typecheck": "pnpm --recursive typecheck",`），
        // 于是**引号与分隔符会被吞进 token**。剥掉每个 token 的尾随标点——剥的是标点，
        // 不是内容：不剥的话 `typecheck",` 连脚本名都算不上，那条引用会**静默消失**（假阴性）。
        const tokens = match[1]
          .trim()
          .split(/\s+/)
          .filter(Boolean)
          .map(token => token.replace(/["',;)\]}]+$/, ''))
          .filter(Boolean);
        let at = 0;
        let recursive = false;
        const filters = [];
        if (tokens[at] === 'run') at += 1;
        while (at < tokens.length && tokens[at].startsWith('-')) {
          const flag = tokens[at];
          if (flag === '--recursive' || flag === '-r') recursive = true;
          else if (flag === '--filter' || flag === '-F') {
            filters.push(tokens[at + 1]);
            at += 1;
          } else if (flag.startsWith('--filter=')) filters.push(flag.slice('--filter='.length));
          else if (flag.startsWith('-F=')) filters.push(flag.slice(3));
          at += 1;
        }
        const script = tokens[at];
        if (!script) continue;
        // `run` 被显式写出时，「脚本名恰好等于子命令名」是合法的（`pnpm run test`）。
        const explicitRun = tokens[0] === 'run';
        if (!explicitRun && subcommands.has(script)) continue;
        if (!/^[@a-zA-Z][\w:.-]*$/.test(script)) continue;

        let dirs;
        if (recursive) {
          dirs = [...packages.keys()];
        } else if (filters.length > 0) {
          dirs = [];
          for (const filter of filters) {
            const dir = nameToDir.get(filter) ?? (packages.has(filter) ? filter : null);
            if (dir === null) unresolved.push({file, line: index + 1, filter, script});
            else dirs.push(dir);
          }
        } else {
          dirs = [ROOT_DIR];
        }
        if (ownDir !== null) dirs = dirs.filter(dir => dir !== ownDir);
        if (dirs.length === 0) continue;
        refs.push({
          file,
          line: index + 1,
          script,
          dirs,
          raw: match[0].trim().replace(/["',;)\]}]+$/, ''),
        });
      }
    });
  }
  return {refs, unresolved};
}

/// 展开 `pnpm-workspace.yaml` 的 `packages:` glob ⇒ 目录前缀（去掉尾段 `*`）。
export function workspaceGlobs(workspaceText) {
  const match = workspaceText.match(/packages:\s*\n((?:\s*-\s*.*\n?)+)/);
  if (!match) throw new Error(`${WORKSPACE_FILE} declares no packages list`);
  const globs = [...match[1].matchAll(/-\s*'?([^'\n]+?)'?\s*$/gm)].map(item => item[1].trim());
  if (globs.length === 0) throw new Error(`${WORKSPACE_FILE} declares an empty package list`);
  return globs;
}

/// 一个 tracked 的 `package.json` 路径 ⇒ 包目录键。
export function packageDirOf(file) {
  if (file === 'package.json') return ROOT_DIR;
  const suffix = '/package.json';
  return file.endsWith(suffix) ? file.slice(0, -suffix.length) : null;
}

/// 判定域：文档 / 源码文件。`kind` 由扩展名决定。
function evidenceText(sources, file) {
  if (!(sources instanceof Map) || !sources.has(file)) return null;
  return stripComments(sources.get(file), file);
}

/// 主判据。入参**显式注入，缺任一项即报错**（一条会因入参缺失而恒真的断言比没有断言更坏）：
///
/// - `packageFiles`：`Map<包目录, package.json 文本>`；
/// - `consumerTexts`：`Map<路径, 文本>`——**引用扫描面**（CI workflow + `scripts/**/*.mjs`）。
///   `.md` MUST NOT 出现在这里：文档是给读者的指引，不是调用。全部 `package.json` 会在内部
///   自动并入（脚本之间互相调用是真实的），自引用则在解析时被排除；
/// - `driverText`：`scripts/frontend-gate.mjs` 全文；
/// - `evidenceSources`：`Map<路径, 文本>`——`manual` / `manual-alias` 的证据落点（文档与源码）；
/// - `workspaceText`：`pnpm-workspace.yaml` 全文；
/// - `trackedPackageJsons`：已跟踪的 `package.json` 路径数组（包目录的闭世界由它派生）。
export function scriptErrors({
  packageFiles,
  consumerTexts,
  driverText,
  evidenceSources,
  workspaceText,
  trackedPackageJsons,
  registered = SCRIPT_DISPOSITIONS,
} = {}) {
  const errors = [];

  // ---------- S0 输入完整性：缺一项就具名，MUST NOT 静默跳过 ----------
  if (!(packageFiles instanceof Map) || packageFiles.size === 0) {
    return ['the package manifests were not provided, so no script can be judged'];
  }
  if (!(consumerTexts instanceof Map)) {
    return ['the reference scan surface was not provided as a map, so no consumer can be judged'];
  }
  if (typeof driverText !== 'string' || driverText === '') {
    return ['the driver source was not provided, so the step table cannot be judged'];
  }
  if (!(evidenceSources instanceof Map)) {
    return ['the evidence sources were not provided as a map, so no cause can pay for itself'];
  }
  if (typeof workspaceText !== 'string' || workspaceText === '') {
    return [`${WORKSPACE_FILE} was not provided, so the package-directory closure cannot be judged`];
  }
  if (!Array.isArray(trackedPackageJsons)) {
    return ['the tracked package manifests were not provided, so the package-directory closure cannot be judged'];
  }

  // ---------- 解析，各自失败即具名 ----------
  let packages = null;
  try {
    packages = packageScripts(packageFiles);
  } catch (error) {
    errors.push(`${error.message}`);
  }
  let steps = null;
  try {
    steps = driverSteps(driverText);
  } catch (error) {
    errors.push(`${DRIVER}: the driver step table could not be parsed, so no consumer can be judged: ${error.message}`);
  }
  let executables = null;
  try {
    executables = driverExecutables(driverText);
  } catch (error) {
    errors.push(`${DRIVER}: the accepted executable names could not be parsed: ${error.message}`);
  }
  let shellSyntax = null;
  try {
    shellSyntax = driverShellSyntax(driverText);
  } catch (error) {
    errors.push(`${DRIVER}: the forbidden shell syntax could not be parsed: ${error.message}`);
  }
  let workspace = null;
  try {
    workspace = workspaceGlobs(workspaceText);
  } catch (error) {
    errors.push(`${error.message}`);
  }

  // ---------- 台账自身：键必须成对且唯一 ----------
  const declared = new Map();
  for (const entry of registered) {
    const dir = entry?.dir;
    const script = entry?.script;
    if (typeof dir !== 'string' || dir === '' || typeof script !== 'string' || script === '') {
      errors.push(`a disposition entry names no pair: ${JSON.stringify(entry)}`);
      continue;
    }
    const key = dispositionKey(dir, script);
    if (declared.has(key)) {
      errors.push(`the script "${script}" of ${dir} is declared twice — one is redundant`);
    }
    declared.set(key, entry);
  }

  // ---------- 包目录的闭世界：workspace 声明 ↔ 已跟踪的 package.json ----------
  if (workspace !== null && packages !== null) {
    const dirs = new Set(packages.keys());
    for (const file of trackedPackageJsons) {
      const dir = packageDirOf(file);
      if (dir === null) {
        errors.push(`the tracked manifest ${file} is not a package.json at the repository root or one level inside a workspace directory`);
        continue;
      }
      if (dir === ROOT_DIR) continue;
      const matched = workspace.some(glob => new RegExp(`^${glob.replace(/[.+^${}()|[\]\\]/g, '\\$&').replace(/\*/g, '[^/]+')}$`).test(dir));
      if (!matched) {
        errors.push(`the package directory ${dir} is not matched by any ${WORKSPACE_FILE} glob (${workspace.join(', ')}) — a package outside the workspace declaration is not built by anyone`);
      }
      if (!dirs.has(dir)) {
        errors.push(`the package directory ${dir} is declared in the workspace yet its manifest was not provided for judgement — a missing input must not silently shrink the closure`);
      }
    }
    for (const glob of workspace) {
      const pattern = new RegExp(`^${glob.replace(/[.+^${}()|[\]\\]/g, '\\$&').replace(/\*/g, '[^/]+')}$`);
      if (!trackedPackageJsons.some(file => pattern.test(packageDirOf(file) ?? ''))) {
        errors.push(`${WORKSPACE_FILE} declares the glob ${glob}, which matches no tracked package — an empty workspace entry makes its rule vacuous`);
      }
    }
  }

  // ---------- 引用解析 ----------
  //
  // 扫描面 = 调用方注入的 `consumerTexts` **∪ 全部 `package.json`**。后者自动并入，因为
  // **脚本之间互相调用是真实的**（根包的 `"typecheck": "pnpm --recursive typecheck"` 就是一条
  // 跨包引用）。但正因如此，**自引用排除**在这里不是可选项：少了它，`root:typecheck` 会被
  // 自己身体里的那行引用消费 ⇒ 恒真的自我满足（MEMORY §6「守卫 MUST NOT 自我满足」）。
  const scanSurface = new Map(consumerTexts);
  for (const [dir, text] of packageFiles) {
    scanSurface.set(dir === ROOT_DIR ? 'package.json' : `${dir}/package.json`, text);
  }
  let refs = [];
  let unresolvedFilters = [];
  if (packages !== null) {
    try {
      const parsed = pnpmReferences(scanSurface, packages);
      refs = parsed.refs;
      unresolvedFilters = parsed.unresolved;
    } catch (error) {
      errors.push(`the references could not be parsed, so no consumer can be judged: ${error.message}`);
    }
  }
  // ---------- 反向：每条引用 MUST 指向一个存在的脚本 ----------
  for (const ref of refs) {
    for (const dir of ref.dirs) {
      const body = scriptBody(packages, dir, ref.script);
      if (body === null) {
        errors.push(`${ref.file}:${ref.line} runs \`${ref.raw}\`, but ${dir} declares no script "${ref.script}" — a reference to a script that does not exist is a check that never runs`);
      }
    }
  }
  for (const stuck of unresolvedFilters) {
    errors.push(`${stuck.file}:${stuck.line} filters on the package ${stuck.filter}, which no manifest declares — a filter that resolves to nothing must not be read as "no consumer"`);
  }

  // ---------- 正向：闭世界（双向）----------
  if (packages !== null) {
    if (packages.size === 0) {
      errors.push('no package manifest was read — an empty manifest set makes the closure vacuously true');
    }
    const present = new Set();
    for (const [dir, entry] of packages) {
      for (const script of Object.keys(entry.scripts)) present.add(dispositionKey(dir, script));
    }
    if (present.size === 0) {
      errors.push('no package declares any script — an empty script set makes the closure vacuously true');
    }
    for (const key of present) {
      if (!declared.has(key)) {
        const [dir, script] = key.split('\u0000');
        // 新增脚本（或原有脚本）**没有任何处置**——这正是 W98 的洞。
        errors.push(`${dir === ROOT_DIR ? 'the root manifest' : dir} declares the script "${script}", which nobody dispositions — an unregistered script is not judged`);
      }
    }
    for (const key of declared.keys()) {
      if (!present.has(key)) {
        const [dir, script] = key.split('\u0000');
        // 删掉脚本 ⇒ 一条声明**静默消失**。
        errors.push(`the disposition table covers "${script}" of ${dir}, but no manifest declares it — a registered script that vanished is a declaration that silently disappeared`);
      }
    }
  }

  // ---------- 逐条：处置闭集 + 代价 ----------
  const causeUsed = new Set();
  for (const entry of registered) {
    if (typeof entry?.dir !== 'string' || typeof entry?.script !== 'string') continue;
    const where = `${entry.dir === ROOT_DIR ? 'the root manifest' : entry.dir} → "${entry.script}"`;
    if (!DISPOSITIONS.includes(entry.disposition)) {
      errors.push(`${where} names the disposition "${entry.disposition}", which is not a disposition — a disposition outside the closed set is a label, not a classification`);
      continue;
    }
    if (entry.disposition === 'invoked') {
      const consumer = entry.consumer;
      if (!consumer || !CONSUMER_KINDS.includes(consumer.kind)) {
        errors.push(`${where} claims a real consumer but names none — "something runs it" has to pay for itself`);
        continue;
      }
      if (consumer.kind === 'driver') {
        const step = consumer.step;
        if (!step || typeof step.dir !== 'string' || typeof step.script !== 'string') {
          errors.push(`${where} claims the driver runs it but names no step — the step is a {dir, script} pair, not a bare name`);
          continue;
        }
        if (steps !== null && !steps.some(item => item.dir === step.dir && item.script === step.script)) {
          errors.push(`${where} claims the driver runs ${step.dir} → "${step.script}", which is not a step in ${DRIVER} — the evidence is a claim, not a fact`);
        }
      } else {
        const {file, marker} = consumer;
        if (typeof file !== 'string' || file === '' || typeof marker !== 'string' || marker === '') {
          errors.push(`${where} claims a pnpm consumer but names no site — the evidence is a {file, marker} pair`);
          continue;
        }
        if (!scanSurface.has(file)) {
          errors.push(`${where} cites ${file}, which was not provided in the reference scan surface — a missing input must not silently drop the evidence`);
          continue;
        }
        const text = stripComments(scanSurface.get(file), file);
        if (!text.includes(marker)) {
          errors.push(`${where} cites ${file} for "${marker}", which is not there once comments are stripped — a comment is not a reference`);
        }
        const attributed = refs.filter(ref => ref.file === file && ref.script === entry.script && ref.dirs.includes(entry.dir));
        if (attributed.length === 0) {
          errors.push(`${where} cites ${file} for a pnpm invocation of "${entry.script}", but no reference there resolves to that package — the citation does not attribute to this script`);
        }
      }
    } else {
      if (!ORPHAN_CAUSES.includes(entry.cause)) {
        errors.push(`${where} is an orphan with cause "${entry.cause}", which is not in the cause table — an unknown cause is a label, not a reason`);
        continue;
      }
      causeUsed.add(entry.cause);
      if (entry.cause === 'covered-by-driver') {
        const covered = entry.coveredBy;
        if (!Array.isArray(covered) || covered.length === 0) {
          errors.push(`${where} claims the driver covers it, but names no covering step — "the driver already does this" has to name which step`);
          continue;
        }
        // ① 它自己 MUST 是一次递归委托：`pnpm --recursive <X>`。
        const body = scriptBody(packages, entry.dir, entry.script);
        const recursive = typeof body === 'string' ? body.match(/^pnpm\s+--recursive\s+([\w:.-]+)\s*$/) : null;
        if (!recursive) {
          errors.push(`${where} claims the driver covers it, but its body is not a \`pnpm --recursive <script>\` delegation — a script that does something else is not covered by covering another`);
          continue;
        }
        for (const item of covered) {
          if (!item || typeof item.dir !== 'string' || typeof item.script !== 'string') {
            errors.push(`${where} names a covering step that is not a {dir, script} pair`);
            continue;
          }
          // ② 覆盖项 MUST 真的是驱动表的一项（判定域 = 解析出的 STEPS）。
          if (steps !== null && !steps.some(step => step.dir === item.dir && step.script === item.script)) {
            errors.push(`${where} claims the step ${item.dir} → "${item.script}" covers it, but that is not a step in ${DRIVER} — coverage by a step that does not exist is not coverage`);
          }
          // ③ 递归委托覆盖的是**同名**脚本：`typecheck` 不覆盖 `build`。
          if (item.script !== recursive[1]) {
            errors.push(`${where} delegates to \`pnpm --recursive ${recursive[1]}\` yet claims coverage by a "${item.script}" step — a recursive delegation only covers the script of the same name`);
          }
        }
      } else if (entry.cause === 'manual-alias') {
        const evidence = entry.equivalentTo;
        if (!evidence || typeof evidence.file !== 'string' || typeof evidence.marker !== 'string' || evidence.marker === '') {
          errors.push(`${where} claims to be an alias, but names no equivalent entry point — "run the same thing another way" has to name where`);
          continue;
        }
        const text = evidenceText(evidenceSources, evidence.file);
        if (text === null) {
          errors.push(`${where} cites ${evidence.file}, which was not provided for judgement`);
          continue;
        }
        if (!text.includes(evidence.marker)) {
          errors.push(`${where} cites ${evidence.file} for "${evidence.marker}", which is not there — the alias has no equivalent`);
        }
      } else {
        const evidence = entry.docEvidence;
        if (!evidence || typeof evidence.file !== 'string' || typeof evidence.marker !== 'string' || evidence.marker === '') {
          errors.push(`${where} claims to be run by hand, but names no documentation — a script nothing executes is only as real as the document that tells a human to run it`);
          continue;
        }
        if (!evidence.file.endsWith('.md')) {
          errors.push(`${where} cites ${evidence.file} as documentation, which is not a markdown file — the evidence for a manual script is a document, not a program`);
          continue;
        }
        const text = evidenceText(evidenceSources, evidence.file);
        if (text === null) {
          errors.push(`${where} cites ${evidence.file}, which was not provided for judgement`);
          continue;
        }
        if (!text.includes(evidence.marker)) {
          errors.push(`${where} cites ${evidence.file} for "${evidence.marker}", which is not there — a manual script with no document is a script nobody knows how to run`);
        }
      }
    }
  }

  // ---------- 驱动表接线：把门控的**运行时**拒绝**静态化** ----------
  if (steps !== null && packages !== null) {
    for (const step of steps) {
      const body = scriptBody(packages, step.dir, step.script);
      if (body === null) {
        errors.push(`${DRIVER} runs ${step.dir} → "${step.script}", which that package does not declare — the gate would fail at run time; it must fail here`);
        continue;
      }
      const heads = new Set();
      for (const sub of body.split('&&').map(part => part.trim()).filter(Boolean)) {
        const tokens = sub.split(/\s+/);
        heads.add(tokens[0]);
        // 判定域（W91：「每个限定 MUST 用反例验证它在拦什么」）：只有**驱动表覆盖的**这些脚本
        // 才拿门控认得的可执行名去量。根包其余的脚本头是 `pnpm`／`openspec`／`cargo`，
        // 它们本来就不经 `frontend-gate.mjs` ⇒ 拿这把尺子量它们是用错了对象。
        if (shellSyntax !== null) {
          const offending = tokens.flatMap(token => shellSyntax.filter(syntax => token.includes(syntax)));
          if (offending.length > 0) {
            errors.push(`${DRIVER} would run ${step.dir} → "${step.script}" = \`${sub}\`, which uses the shell syntax ${offending.join(', ')} that the gate does not implement — the gate refuses it at run time; it must be refused here`);
          }
        }
      }
      if (executables !== null) {
        const known = [...heads].filter(head => executables.includes(head));
        if (known.length === 0) {
          errors.push(`${DRIVER} would run ${step.dir} → "${step.script}", whose program ${[...heads].join(', ')} is not one of the executable names the gate resolves (${executables.join(', ')}) — the gate refuses it at run time; it must be refused here`);
        }
      }
    }
  }

  // ---------- 成因闭集非空：空闭集是「没有断言」的伪装 ----------
  if (causeUsed.size === 0 && registered.length > 0) {
    errors.push('no disposition exercises an orphan cause, so the cause rules are vacuous');
  }

  return errors;
}
