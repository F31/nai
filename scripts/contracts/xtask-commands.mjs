import {reasonPaymentErrors} from './cause-payment.mjs';

/// `cargo xtask` 的**命令面闭世界**：`execute()` 的每一条命令 MUST 具名在册，
/// 且 `Usage:` 字符串 MUST 与它**对账**。
///
/// 为什么需要它：`xtask/src/lib.rs` 的 `execute()` 是本机门控的**入口面**——一个
/// `match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice()` 的 **24 条臂**。
/// 这个集合**没有任何在册台账**，而它住在同一个文件里、与 `check --profile full` 的
/// 门控闭包相隔几十行。同一个文件里的用法字符串（`Usage: cargo xtask …`）是命令集合的
/// **第二次书写**，两处没有任何机制保证一致。
///
/// 实测（W96 三个探针，改前/改后都真跑）：
/// - 往 `execute()` 插一条臂 `["check", "--audit"] => Ok(())` ⇒ 本机 4 套件
///   （registry 52 / schema 30 / platform 22 / workflow 13）**全绿、零信号**；
/// - 删掉 `["check", "--secrets"] => secrets::check(root)` ⇒ **同样全绿**——一条判定命令**静默消失**；
/// - usage 字符串追加 `| check --audit` ⇒ **同样全绿**——usage 与 dispatch **分家**。
///
/// 与 W84（测试文件的可达性）、W86（OpenAPI 文档集）、W95（CI workflow 的 step）同形，
/// **新的一层对象**：`execute()` 的命令面。四条纪律照抄：
/// 1. **判定单元 MUST 是命令的「身份」（签名），MUST NOT 是裸命令名**（W91：「同名」≠「同一个」）。
///    签名 = 空格连接的顶层字面量序列——`check --golden` 与 `check --golden --dir` 是**两条**命令，
///    只按名字看会合成一条。同理匹配锚是整条签名（`["check","--secrets"]`），不是 `"--secrets"`
///    （后者会在 `secrets::check` 这类**别的对象**上命中）。
/// 2. **「已声明」≠「已执行」**（W75）：`gate` 声称的门控落点 MUST 在**门控闭包**里**逐字存在**。
/// 3. **闭世界 MUST 在每个层级上成立**（W82）：命令面之上还有 **usage 字符串**这一层——
///    它同样必须与 dispatch **双向对账**。
/// 4. **不可解析 ⇒ 具名失败，MUST NOT 静默降级**（W90）。
///
/// ⚠️ **分类是静态声明，MUST NOT 由运行时探测得出**（铁律二）：`EBUSY` 是**宿主事实**，
/// 换一台机器会对同一个仓库得出不同分类 ⇒ 判据不再可复现。分类是人的主张，逐条在册；
/// 判据负责验「主张有没有付代价」。

/// 被审对象的宿主文件。
export const XTASK_HOST = 'xtask/src/lib.rs';

/// **命令的类别**。闭集，理由与 `STEP_KINDS`／`TEST_TIERS` 一致：让「这条命令在哪一层跑」可判定。
/// - `gate`：它的**执行体**在**本机门控闭包**（`check --profile portable|windows-local|full`
///   共用的 `engineering_checks`，以及 `check --profile` 各 arm 体）里被执行；
/// - `standalone`：是一条**判定命令**，但**不在**门控闭包里——只有显式调用才跑；
/// - `artifact`：**生成产物**、不产生判定结论（`sbom`、`schema`）；
/// - `help`：不产生动作的入口（就是「打印 usage」本身）。
export const COMMAND_KINDS = ['gate', 'standalone', 'artifact', 'help'];

/// `standalone` 的**成因闭集**。域外取值 ⇒ 具名失败（W90：一个未知成因是一个标签，不是一个理由）。
/// 三个取值按**实测**区分——「不在门控里」有三种不同的成因，混成一个会让理由不可读：
/// - `requires-compiled-artifact`：需要 protoc／编译产物（`schema --check`、`test --suite contracts`）；
/// - `superseded-by-workspace-run`：门控跑的是**全 workspace** 测试（`run(root, &["test",
///   "--workspace", …])`），覆盖同一批包，但**不走这个入口**（`test --suite ×8`）；
/// - `deliberate-on-demand`：有意按需，**不并入**常规门控（`check --golden`／`check --secrets`。
///   依据 ADR 0043：`cargo xtask sbom` 是生成命令，与 `check --secrets` 同类，暂不并入
///   `engineering_checks`——portable 门控须保持「只需 Rust 工具链」不变）。
///
/// ⚠️ 上面每一点都 MUST 在 `STANDALONE_REASON_PAYMENTS` 里有**凭据**。原有的第四个取值
/// `host-subprocess` 已**逐出**（W108 / ADR 0086）：它是**宿主/机器事实**，而宿主瞬态 MUST NOT
/// 成为判定输入（铁律二）⇒ **它不可能有凭据**——它是这条规则里一个**永久无法兑付的空格**。
/// 逐出的理由**不引用任何发生过的瞬态**：即使本机从未跑通 `schema --check`，结论相同。
export const STANDALONE_REASONS = [
  'requires-compiled-artifact',
  'superseded-by-workspace-run',
  'deliberate-on-demand',
];

/// **成因兑付规格**（W108）：每条 `standalone` 成因 MUST 付代价——给出一条**可从仓库自证**的凭据。
///
/// 形状：`{evidence: [{file, marker, why}, …]}`，每格**至少一条**。凭据的 `marker` MUST 在该
/// `file` 的内容里**逐字存在**，`file` MUST 已被 git 跟踪，且 MUST NOT 落在 `scripts/contracts/`
/// 之下（被审注册表自己写的凭据对表恒真）。`why` MUST 非空——它答「这条凭据回答了哪个问题」，
/// 判据验不了它，但人读它。
///
/// ⚠️ **为什么不强制成对**：强制成对会逼出**凑数的第二条**。为凑成对而补一条「看起来相关」的
/// 文本，正是本项目禁止的**装饰** ⇒ 形状定为「至少一条」，条数由成因自己决定。
///
/// ⚠️ **为什么 `host-subprocess` 不在这张表里**：它**不可能**有凭据——它的成因是**宿主/机器事实**
/// （能不能派生子进程），而宿主瞬态 MUST NOT 成为判定输入（铁律二）⇒ 它是这条规则里一个
/// **永久无法兑付的空格**，因此从成因闭集**逐出**。逐出的理由**不引用任何发生过的瞬态**：
/// 即使本机从未跑通 `schema --check`，逐出结论相同。
export const STANDALONE_REASON_PAYMENTS = {
  'requires-compiled-artifact': {
    evidence: [
      {
        file: 'xtask/src/schema.rs',
        marker: 'generation requires exactly protoc 3.21.12',
        why: '生成器在编译 proto 前**硬校验** protoc 版本 ⇒ 这条命令的工具链前提超出 portable 门控「只需 Rust 工具链」的既有不变式（ADR 0041 / 0043）',
      },
    ],
  },
  'superseded-by-workspace-run': {
    evidence: [
      {
        file: 'xtask/src/lib.rs',
        marker: '["test", "--workspace", "--locked", "--exclude", HARNESS]',
        why: '门控用**一条**全 workspace 测试覆盖同一批包 ⇒ 这些 suite 的判定面不因它们不进门控而消失（把这条调用换成显式包列表，本仓会立刻**静默牺牲包**，W97 已实测）',
      },
    ],
  },
  'deliberate-on-demand': {
    evidence: [
      {
        file: 'xtask/src/golden.rs',
        marker: 'pub fn check(',
        why: '这些命令有**真实执行体**——它们是能跑的判定，不是禁用项；「按需」说的是接入方式，不是「跑不了」',
      },
      {
        file: 'docs/adr/0043-sbom-generation-and-scope.md',
        marker: '暂不并入 `engineering_checks`',
        why: 'ADR 0043 具名裁决「暂不并入 engineering_checks：portable 门控须保持『只需 Rust 工具链』不变」——「有意按需」是一条**人的决定**，凭据就是那条决定的记录',
      },
    ],
  },
};

/// **usage 里被具名豁免的三条**。它们本就不该出现在 `Usage:` 里——因为它们**就是**
/// 「打印 usage」那件事本身。
///
/// 具名而非「其余随便」：少写一条**判定命令**是缺陷，少写 `help` 不是；把两者混进同一个
/// 「其余」里，「少写一条命令」就会混在豁免里溜过去。**触发条件只由「字段存在」决定**（W83）。
export const HELP_SIGNATURES = ['help', '--help', ''];

/// **在册命令表**：一条「某条命令属于哪一类」的独立声明。每条 `{signature, kind, …}`：
/// - `signature`：命令的**签名**，由 `execute()` 的臂**解析**得出（空格连接的顶层字面量序列）；
/// - `kind`：∈ `COMMAND_KINDS`；
/// - `gate` MUST 带 `evidence: {file, marker}`——「本机门控真的跑它」的**凭据**；
/// - `standalone` MUST 带 `reason`（∈ `STANDALONE_REASONS`），且 MUST NOT 带 `evidence`；
///   它的成因 MUST 在 `STANDALONE_REASON_PAYMENTS` 里有凭据（W108）。
///
/// **逐字写死**：这份表是「有哪些命令」的**独立声明**，不是从 `execute()` 派生的——
/// 派生出的「现有集合 == 清单」是恒真式，一个字都没说。
export const XTASK_COMMANDS = [
  {signature: '', kind: 'help'},
  {signature: '--help', kind: 'help'},
  {signature: 'help', kind: 'help'},
  {
    signature: 'check-deps',
    kind: 'gate',
    evidence: {file: XTASK_HOST, marker: 'deps::check(root)?'},
  },
  {
    signature: 'check --registry',
    kind: 'gate',
    evidence: {file: XTASK_HOST, marker: 'registry::check(root)?'},
  },
  {
    signature: 'check --golden',
    kind: 'standalone',
    reason: 'deliberate-on-demand',
  },
  {
    signature: 'check --golden --dir',
    kind: 'standalone',
    reason: 'deliberate-on-demand',
  },
  {
    signature: 'check --secrets',
    kind: 'standalone',
    reason: 'deliberate-on-demand',
  },
  {
    signature: 'check --profile portable',
    kind: 'gate',
    // 这条证据同时出现在 `full` 的 arm 里 ⇒ 判据验「它在门控闭包内逐字存在」，
    // **不**验「它专属于 portable」。假阴性方向的宽松，见 design 决定 3 的如实披露。
    evidence: {file: XTASK_HOST, marker: 'engineering_checks(root)?'},
  },
  {
    signature: 'check --profile windows-local',
    kind: 'gate',
    // 取该 arm **独有**的落点（`full` 的 arm 里没有这一句）。
    evidence: {
      file: XTASK_HOST,
      marker: '"windows-local requires a native Windows environment"',
    },
  },
  {
    signature: 'check --profile full',
    kind: 'gate',
    evidence: {file: XTASK_HOST, marker: 'frontend::check(root)?'},
  },
  {signature: 'sbom', kind: 'artifact'},
  {signature: 'sbom --out', kind: 'artifact'},
  {signature: 'schema', kind: 'artifact'},
  {
    signature: 'schema --check',
    kind: 'standalone',
    // W108：原为 `host-subprocess`。该成因是**宿主事实**（能不能派生子进程），宿主瞬态 MUST NOT
    // 成为判定输入（铁律二），因此它**不可能有凭据**、已从闭集逐出。此处按**今日实测的**事实重归类：
    // `generate()` 在编译 proto 前硬校验 `protoc` 3.21.12（`schema.rs:207-217`）⇒ 这条命令的工具链
    // 前提超出 portable 门控「只需 Rust 工具链」的既有不变式（ADR 0041 / 0043），与 `test --suite
    // contracts` 同因。⚠️ 这个理由**不随宿主变化**：本机恰巧装了 protoc，不代表门控可以依赖它。
    reason: 'requires-compiled-artifact',
  },
  {
    signature: 'test --suite contracts',
    kind: 'standalone',
    reason: 'requires-compiled-artifact',
  },
  {signature: 'test --suite knowledge', kind: 'standalone', reason: 'superseded-by-workspace-run'},
  {signature: 'test --suite provider', kind: 'standalone', reason: 'superseded-by-workspace-run'},
  {signature: 'test --suite scheduler', kind: 'standalone', reason: 'superseded-by-workspace-run'},
  {signature: 'test --suite session', kind: 'standalone', reason: 'superseded-by-workspace-run'},
  {signature: 'test --suite kernel', kind: 'standalone', reason: 'superseded-by-workspace-run'},
  {signature: 'test --suite hosts', kind: 'standalone', reason: 'superseded-by-workspace-run'},
  {signature: 'test --suite storage', kind: 'standalone', reason: 'superseded-by-workspace-run'},
  {signature: 'test --suite upgrade', kind: 'standalone', reason: 'superseded-by-workspace-run'},
];

/// 从 `openIndex` 处的 `{` 起，用**括号计数**取到配对的 `}`，返回中间的文本。
/// 不计字符串／注释里的花括号——本文件里没有那种情形，且**如实披露**：
/// 输入若是含 `{` 的字符串字面量，定位会偏（当前宿主没有，新增时会被下游断言发现）。
function braceBody(source, openIndex) {
  let depth = 0;
  for (let i = openIndex; i < source.length; i += 1) {
    if (source[i] === '{') depth += 1;
    else if (source[i] === '}') {
      depth -= 1;
      if (depth === 0) return source.slice(openIndex + 1, i);
    }
  }
  throw new Error('unbalanced braces');
}

/// 定位 `execute()` 里 `match args …` 的体。找不到 ⇒ 抛（调用方转成具名错误）。
function matchBody(source) {
  const fn = source.indexOf('pub fn execute(');
  if (fn < 0) throw new Error('no `pub fn execute(` in the harness');
  const at = source.indexOf('match args', fn);
  if (at < 0) throw new Error('`execute` carries no `match args`');
  const open = source.indexOf('{', at);
  if (open < 0) throw new Error('`match args` has no body');
  return braceBody(source, open);
}

/// 把一个臂模式按**顶层** `|` 分列（括号内的 `|` 不分列，例如
/// `profile @ ("portable" | "windows-local")`）。
function splitTopLevel(pattern) {
  const parts = [];
  let depth = 0;
  let current = '';
  for (const ch of pattern) {
    if (ch === '(') depth += 1;
    else if (ch === ')') depth -= 1;
    if (ch === '|' && depth === 0) {
      parts.push(current);
      current = '';
    } else {
      current += ch;
    }
  }
  parts.push(current);
  return parts.map(part => part.trim()).filter(part => part !== '');
}

/// 把一个**单个**臂模式展开成命令签名。
/// - `"lit"` ⇒ 字面量，贡献一个 token；
/// - `name @ ("a" | "b")` ⇒ 标识符锚 + 括号内字面量集合 ⇒ 笛卡尔积展开（锚本身被抹掉）；
/// - 裸标识符（`dir`／`path`）⇒ **绑定变量**，不贡献 token（它承载的是值，不是命令的一部分）。
function expandSignaturePattern(pattern) {
  const tokens = [];
  const re = /([A-Za-z_][A-Za-z0-9_]*)\s*@\s*\(([^)]*)\)|"([^"]*)"|([A-Za-z_][A-Za-z0-9_]*)/g;
  let match;
  while ((match = re.exec(pattern))) {
    if (match[1] !== undefined) {
      const values = [...match[2].matchAll(/"([^"]*)"/g)].map(item => item[1]);
      if (values.length === 0) throw new Error(`an anchored pattern binds nothing: ${pattern}`);
      tokens.push({kind: 'choice', values});
    } else if (match[3] !== undefined) {
      tokens.push({kind: 'literal', value: match[3]});
    } else {
      tokens.push({kind: 'binding'});
    }
  }
  let acc = [''];
  for (const token of tokens) {
    if (token.kind === 'binding') continue;
    const values = token.kind === 'literal' ? [token.value] : token.values;
    acc = acc.flatMap(prefix => values.map(value => (prefix === '' ? value : `${prefix} ${value}`)));
  }
  return acc;
}

/// 从 `execute()` 的 match 臂解析出**全部命令签名**。不可解析 ⇒ 抛。
export function dispatchSignatures(text) {
  const body = matchBody(text);
  const armRe =
    /^[ \t]*((?:\[[^\[\]]*\]|_)(?:[ \t]*\|[ \t]*(?:\[[^\[\]]*\]|_))*)[ \t]*=>/gm;
  const out = [];
  for (const match of body.matchAll(armRe)) {
    for (const sub of splitTopLevel(match[1])) {
      if (sub === '_') continue; // 兜底臂不是一条命令。
      for (const signature of expandSignaturePattern(sub)) out.push(signature);
    }
  }
  return out;
}

/// 从 `Usage:` 字符串解析出**全部命令签名**。不可解析 ⇒ 抛。
///
/// 展开规则（与 `execute()` 的写法一一对应）：
/// - `[--dir <path>]` 这类**可选段** ⇒ 有／无两路（`<path>` 是占位符，不贡献 token）；
/// - `portable|windows-local|full` 这类**多值 token** ⇒ 逐值展开；
/// - 顶层分隔符是 ` | `（**带空格**）——`portable|windows-local` 内部无空格，因此不会被切开。
export function usageSignatures(text) {
  const match = text.match(/"Usage: cargo xtask ([^"]*)"/);
  if (!match) throw new Error('no `Usage: cargo xtask …` string in the harness');
  const firstLine = match[1].split('\\n')[0];
  const segments = firstLine.split(' | ');
  const out = [];
  for (const segment of segments) {
    for (const signature of expandUsageSegment(segment)) out.push(signature);
  }
  return out;
}

/// 展开一个 usage 片段：可选段与多值 token 的笛卡尔积（`<占位符>` 被抹掉）。
function expandUsageSegment(segment) {
  const optionals = [];
  const masked = segment.replace(/\[([^\]]*)\]/g, (_, inner) => {
    optionals.push(inner);
    return `\u0001${optionals.length - 1}\u0001`;
  });
  const tokens = masked.split(/\s+/).filter(Boolean);
  const choicesPerToken = tokens.map(token => {
    const placeholder = token.match(/^\u0001(\d+)\u0001$/);
    if (placeholder) {
      const inner = optionals[Number(placeholder[1])].split(/\s+/).filter(Boolean);
      let acc = [''];
      for (const part of inner) {
        acc = acc.flatMap(prefix =>
          tokenChoices(part).map(value => (value === '' ? prefix : prefix === '' ? value : `${prefix} ${value}`)),
        );
      }
      return ['', ...acc.filter(Boolean)];
    }
    return tokenChoices(token);
  });
  let out = [''];
  for (const choices of choicesPerToken) {
    out = out.flatMap(prefix =>
      choices.map(value => (value === '' ? prefix : prefix === '' ? value : `${prefix} ${value}`)),
    );
  }
  return out.filter(signature => signature !== '');
}

/// 一个 token 的取值：`<占位符>` ⇒ 空（抹掉）；含 `|` ⇒ 逐值；否则原样。
function tokenChoices(token) {
  if (/^<.*>$/.test(token)) return [''];
  if (token.includes('|')) return token.split('|').filter(Boolean);
  return [token];
}

/// **门控区域**：`check --profile portable|windows-local|full` 真的会执行的代码区域。
/// 由解析得出，共三块：
/// 1. `engineering_checks` 的函数体（`portable` 与 `full` 都从这里开始）；
/// 2. `["check", "--profile", …] => { … }` 各 arm 的函数体。
///
/// **为什么必须有这一层**：只要求「marker 在文件里逐字存在」，marker 就可以填 **arm 自己**的
/// 文本（`["check", "--secrets"]`），从而把「本机门控真的跑它」偷换成「这条命令自己存在」
/// ——**自我满足**的典型形态。区域把判定域收紧到**门控闭包**内。不可定位 ⇒ 抛。
export function gateRegions(text) {
  const regions = [];
  const fn = text.indexOf('fn engineering_checks(');
  if (fn < 0) throw new Error('no `fn engineering_checks(` in the harness');
  const fnOpen = text.indexOf('{', fn);
  if (fnOpen < 0) throw new Error('`engineering_checks` has no body');
  regions.push({name: 'engineering_checks', text: braceBody(text, fnOpen)});

  const armRe = /\[\s*"check"\s*,\s*"--profile"[^\]]*\]\s*=>\s*\{/g;
  let match;
  while ((match = armRe.exec(text))) {
    const open = match.index + match[0].length - 1;
    regions.push({name: `arm@${match.index}`, text: braceBody(text, open)});
  }
  if (regions.length < 2) {
    throw new Error('no `check --profile` arm body in the harness — the gate closure would be half-read');
  }
  return regions;
}

/// 登记表自身的一致性。与 `siteShapeErrors()` 同一条纪律：挡的是**未来某次合法编辑**
/// 造成的**静默失去覆盖**，而不是当前的错。
export function registryShapeErrors(registered = XTASK_COMMANDS) {
  const errors = [];
  const seen = new Set();
  for (const entry of registered) {
    if (typeof entry?.signature !== 'string') {
      errors.push(`a command entry carries a non-string signature ${JSON.stringify(entry?.signature)}`);
      continue;
    }
    if (seen.has(entry.signature)) {
      errors.push(`the command signature ${JSON.stringify(entry.signature)} is declared twice — one is redundant`);
    }
    seen.add(entry.signature);
  }
  return errors;
}

/// 主判据。入参显式注入，**缺任一项即报错**（一条会因入参缺失而恒真的断言比没有断言更坏）。
///
/// `text`：`xtask/src/lib.rs` 全文；
/// `registered`：在册命令表（默认 `XTASK_COMMANDS`）；
/// `gateSources`：`Map<path, text>`——`gate` 的证据所指文件必须在这里，**显式注入**
/// （少了它，「本机门控真的跑它」的凭据会退化成恒真）；
/// `reasonInput`：`{sources: Map<path, text>, tracked: Set<string>}`——`standalone` 的**成因凭据**
/// 所指文件的内容与跟踪状态，**显式注入**（少了它，「这条成因凭什么成立」会退化成恒真）。
/// 可选带 `payments`：**兑付表本身**的注入位，缺省用 `STANDALONE_REASON_PAYMENTS`——
/// 没有它，「表少一格 / 多一格 / 凭据指错」这几种缺陷**无法被测试构造**（守卫都要注入验证）。
export function commandErrors(
  text,
  registered = XTASK_COMMANDS,
  gateSources = new Map(),
  reasonInput,
) {
  const errors = [];
  if (typeof text !== 'string' || text === '') {
    return ['the harness source was not provided, so no command can be judged'];
  }
  if (!(gateSources instanceof Map)) {
    return ['the gate sources were not provided as a map, so gate coverage cannot be judged'];
  }
  const causeInput = reasonInput ?? {};

  errors.push(...registryShapeErrors(registered));

  // ---------- 解析三件事，各自失败即具名，绝不静默降级 ----------
  let dispatch = null;
  try {
    dispatch = dispatchSignatures(text);
  } catch (error) {
    errors.push(`${XTASK_HOST}: the command dispatch could not be parsed, so no command can be judged: ${error.message}`);
  }
  let usage = null;
  try {
    usage = usageSignatures(text);
  } catch (error) {
    errors.push(`${XTASK_HOST}: the usage string could not be parsed, so it cannot be reconciled with the dispatch: ${error.message}`);
  }
  let regions = null;
  try {
    regions = gateRegions(text);
  } catch (error) {
    errors.push(`${XTASK_HOST}: the local gate closure could not be located, so gate coverage cannot be judged: ${error.message}`);
  }

  // ---------- 闭世界（命令面）：两个方向都被点名 ----------
  if (dispatch !== null) {
    if (dispatch.length === 0) {
      errors.push(`${XTASK_HOST} dispatches no command — an empty dispatch makes the closure vacuously true`);
    }
    if (registered.length === 0) {
      errors.push('the command registry is empty — an empty registry makes the closure vacuously true');
    }
    const declared = new Set(registered.map(entry => entry.signature));
    const present = new Set(dispatch);
    for (const signature of present) {
      if (!declared.has(signature)) {
        // 新增／改名的命令**没有任何处置**——这正是 W96 的洞。
        errors.push(`${XTASK_HOST} dispatches the command "${signature}", which nobody declares — an unregistered command is not judged`);
      }
    }
    for (const entry of registered) {
      if (!present.has(entry.signature)) {
        // 删掉一条臂 ⇒ 一条命令**静默消失**。
        errors.push(`${XTASK_HOST} is declared to dispatch "${entry.signature}", but it does not — a registered command that vanished is a judgement that silently disappeared`);
      }
    }
  }

  // ---------- usage 与 dispatch 的**双向对账** ----------
  if (usage !== null && dispatch !== null) {
    if (usage.length === 0) {
      errors.push(`${XTASK_HOST}: the usage string lists no command — an empty usage makes the reconciliation vacuously true`);
    }
    const dispatched = new Set(dispatch);
    const advertised = new Set(usage);
    for (const signature of advertised) {
      if (!dispatched.has(signature)) {
        errors.push(`${XTASK_HOST}: the usage string advertises "${signature}", which the dispatch does not implement — usage must not promise what does not exist`);
      }
    }
    for (const signature of dispatched) {
      if (!advertised.has(signature) && !HELP_SIGNATURES.includes(signature)) {
        // 少写一条**判定命令**是缺陷；少写 `help` 不是（三条已具名豁免）。
        errors.push(`${XTASK_HOST}: the command "${signature}" is dispatched but never appears in the usage string — a command nobody can discover`);
      }
    }
  }

  // ---------- 逐条：类别闭集 + 双向字段 + 代价 ----------
  for (const entry of registered) {
    if (!COMMAND_KINDS.includes(entry.kind)) {
      errors.push(`the command "${entry.signature}" names kind "${entry.kind}", which is not a command kind — a kind outside the closed set is a label, not a classification`);
      continue;
    }
    const evidence = entry.evidence;
    if (entry.kind === 'gate') {
      // `gate` 必须付代价：引出一处**真的落在门控闭包内**的落点（W75「已声明 ≠ 已执行」）。
      if (
        !evidence ||
        typeof evidence.file !== 'string' ||
        evidence.file === '' ||
        typeof evidence.marker !== 'string' ||
        evidence.marker === ''
      ) {
        errors.push(`the command "${entry.signature}" claims gate coverage but names no gate site — "the local gate runs it too" has to pay for itself`);
        continue;
      }
      if (!gateSources.has(evidence.file)) {
        // 缺输入 MUST 具名，MUST NOT 静默跳过——那会让判据在被削弱时保持沉默。
        errors.push(`the command "${entry.signature}" cites gate source ${evidence.file}, which was not provided for judgement`);
        continue;
      }
      if (regions !== null && !regions.some(region => region.text.includes(evidence.marker))) {
        errors.push(`the command "${entry.signature}" cites ${evidence.file} for "${evidence.marker}", which is not inside the local gate closure (${regions.map(region => region.name).join(', ')}) — the evidence is a claim, not a fact`);
      }
    } else if (entry.kind === 'standalone') {
      // `standalone` 必须付代价：成因 MUST 落在闭集里。
      if (typeof entry.reason !== 'string' || !STANDALONE_REASONS.includes(entry.reason)) {
        errors.push(`the command "${entry.signature}" is standalone with reason "${entry.reason}", which is not in the cause table — an unknown cause is a label, not a reason`);
      }
      // 双向：`standalone` MUST NOT 同时声称门控覆盖（否则「它是不是在门控里跑」没有单一回答）。
      if (evidence !== undefined) {
        errors.push(`the command "${entry.signature}" is standalone yet also carries gate evidence — a command cannot be inside the gate closure and outside it at once`);
      }
    } else {
      // `artifact` / `help`：两个维度都不适用——**不合并维度**。
      if (evidence !== undefined) {
        errors.push(`the command "${entry.signature}" is ${entry.kind} yet carries gate evidence — a command that produces no judgement cannot claim gate coverage`);
      }
      if (entry.reason !== undefined) {
        errors.push(`the command "${entry.signature}" is ${entry.kind} yet carries a standalone cause — the two dimensions must not be merged`);
      }
    }
  }

  // ---------- 成因兑付（W108）：每条成因 MUST 付代价——一条可从仓库自证的凭据，
  // 或从闭集逐出。成因格 MUST ≥1 实例（没有实例的规则对空集恒真）。 ----------
  errors.push(
    ...reasonPaymentErrors({
      label: XTASK_HOST,
      where: 'command',
      closedSet: STANDALONE_REASONS,
      payments: causeInput.payments ?? STANDALONE_REASON_PAYMENTS,
      reasons: registered.map(entry => entry.reason).filter(reason => typeof reason === 'string'),
      sources: causeInput.sources,
      tracked: causeInput.tracked,
    }),
  );

  // ---------- 类别都非空：某一类被合法清空 ⇒ 它的规则对空集恒真 ⇒ 显式处置 ----------
  const kinds = new Set(registered.map(entry => entry.kind));
  for (const kind of COMMAND_KINDS) {
    if (!kinds.has(kind)) {
      errors.push(`the registry exercises no ${kind} command, so its rules are vacuous`);
    }
  }

  return errors;
}
