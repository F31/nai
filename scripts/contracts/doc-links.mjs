/// **文档内部指针的可达性**（W151 立项，ADR 0125；W152 扩域并补代码段豁免，ADR 0126；
/// W153 把引用式链接纳入域、把锚点改为拒绝，ADR 0127；W154 把「未定义引用式使用」的计数
/// 改成有人认领的名单，ADR 0128）。
///
/// 问题形状：文档之间的相对链接是「记录的记录」——报告指向 ADR、ADR 指向基线、报告指向归档变更。
/// 它们一旦断开，读者落在空处，而**没有任何门控会说一句话**：`--strict` / `--archived` / registry /
/// 各条契约判据都不解析 markdown 链接。W151 实测 `docs/` 的 793 条相对链接里有 36 处缺陷；
/// W152 把域扩到**全仓**（991 份已跟踪 `*.md` / 963 条链接 / 925 条相对），又量出 3 处。
///
/// 判据只做一件事：**每条相对指针的落点，都必须是索引里真的存在的东西**。它不判内容对不对
/// （链接指的那一节是否真在讲这件事，是语义，机械判不了），只判「能不能走到」。
///
/// 域与口径：
/// - 域是**已跟踪**路径（`.git/index` 派生，与 W75 同源）。未跟踪的文件不在域内——与
///   `pending-registration` 同一条边界，两者不互相替代。
/// - 文档域 = 仓库内全部已跟踪 `*.md`（`documentPathsOf` 是这一定义的唯一落点）。W151 时域是
///   `docs/`，W152 扩到全仓：`openspec/` 的变更与规格文档、根目录的 README/AGENTS 同样是记录。
/// - **代码段里的链接形状不是指针**：行内代码与围栏块是被引用的文本（规格里举的坏例子就活在这里），
///   判它们等于要求作者不能举例。遮蔽在判定前完成（`maskCode`），行号保持不变。
/// - **目录算存在**：一条链接指向目录是可点的，只要该前缀下有已跟踪文件。把目录判成断链会让判据
///   去要求本来就对的链接改写法，那是判据错，不是记录错。
/// - **通配不是指针**：`*.md` 会说「我指的是哪一份」这一件事没说，且它永远解析得到东西 ⇒
///   直接失败，MUST NOT 放行。
/// - **外部链接不判**：http(s)/mailto/协议相对一律跳过——它们的可达性不归本仓库管。
/// - **域 = 「markdown 认为是链接的东西」，不是「看起来像链接的东西」**（W153 定下的判据边界）。
///   - 引用式**定义**行 `[label]: url` 在域内：URL 走与内联链接**同一套**可达性判定。W153 勘定前
///     全仓定义行 0 条 ⇒ 零样本不等于没问题，它意味着这种写法一旦出现判据完全看不见。
///   - 引用式**使用** `[text][label]` 在两种情形下分开：label **有**本文件定义 ⇒ 它是链接，
///     落点由定义那一侧判（别处的同名定义不算，CommonMark 的定义按文档生效）；label **没有**定义
///     ⇒ CommonMark 把它降级成**字面文本**，读者看到的就是原文写下的字 ⇒ **它不是指针**，
///     判它「指向哪里」等于判排版。
///   - **「不是指针」≠「不需要有人认领」**（W154 定下的后半条）。W153 只证了前半条，于是在这条
///     支路上留下两个洞：计数只有**一个数字**没有**名单**（拿不到「是哪几条」，核对就无从谈起），
///     计数**没有上限**（用例只钉「非负整数」，多写一处笔误，6 变 7 依然全绿）。⇒ 每一处未定义的
///     引用式使用 MUST 在 `tests/undefined-references.yaml` 里被**具名认领**，否则失败。
///     **认领不是豁免**：两种状态各要付一条**能被机械否证**的声称，且方向相反——`citation` 声称
///     「该标签在本文件的资料清单里有条目」（找不到 ⇒ 声称倒），`missing_bibliography` 声称
///     「本文件一条清单都没有」（一旦有 ⇒ 声称倒）。⇒ 随手登记一条**糊不过去**，记录一改还会
///     **自行过期**：台账过期同样是红的。
/// - **锚点（`#fragment`）在域内且判「拒绝」**：标题怎么变成碎片由渲染器决定
///   （GitHub / GitLab / 编辑器预览 / 静态站生成器各一套 slug 规则，中文标题更是各写各的）⇒
///   判据**无法**判断那个位置存不存在 ⇒ 拒绝。带碎片的指针：文件部分照判，碎片本身单独失败。
///   MUST NOT 静默跳过——跳过的意思是「这条我看不了，但我把它算进绿里了」。
///   注意它与上一条的区别：`[x](#a)` **是**链接（inline link），只是落点不可判 ⇒ 拒绝；
///   `[x][y]`（y 无定义）**不是**链接 ⇒ 不在域内。两者结论不同，理由是同一个：按「它是不是链接」分。
/// - **shortcut `[label]` 不在域内**：它与普通方括号（数组下标、清单勾选框、`[待办]`）在字形上
///   不可区分，自造正则去猜只会产出成片的假指针。这是一条**声明的域边界**，不是「已覆盖」。
/// - **转义方括号（`\[x\]`）未处理**：W153 勘定实测 `\[` 全仓 56 处，但没有一处落在定义行或
///   引用式使用的**位置**上（全是 `evidence\[\]` 这类写法），所以今天不产生误判。定义行以
///   `^ {0,3}\[` 锚定、被转义的写不了行首；而 `\[x\]\[y\]` 会被当成**一条引用式使用**
///   ⇒ 多计一次 `undefinedReferences`。方向是「多看一步」（计数偏大），不是「放行」。
///
/// 判据形状是**纯函数 + 可注入**：域（哪些文档、它们的文本、索引里有哪些路径）由调用方给，
/// 判据只做判定。真实语料侧由测试从 `.git/index` 派生。

/// 文档后缀。判据只读这些文件——`.json` 证据文件不是记录。
export const DOC_SUFFIX = '.md';

/// 「未定义引用式使用」的认领状态。**封闭枚举**，两种状态各付一条**可被机械否证**的声称：
///
/// - `citation`：作者在引注某一来源，而来源条目写在同一份文档的资料清单里（`- [S3] …`）。
///   声称 = 「该标签在本文件有清单条目」⇒ 判据去找，找不到就是声称倒了。
/// - `missing_bibliography`：**本文件一条资料清单都没有**，所以那条引注**无出处可核**。
///   这不是「没问题」，是「问题在仓外」：关闭它需要真实来源，编不出来。声称 = 「本文件清单为空」
///   ⇒ 判据去数，一旦有人补了清单，这条登记就过期。
///
/// 两种声称方向相反，是刻意的：**同一张台账里，「有出处」与「没出处」不能靠作者自己说**，
/// 否则它就从台账退化成豁免清单（MEMORY §7：台账不是豁免，要具名、相称、自清理）。
export const UNDEFINED_REFERENCE_STATES = ['citation', 'missing_bibliography'];

/// 认领状态 → 必填字段。`citation` 的声称就是「同文件有清单条目」这个事实本身，不需要额外字段；
/// `missing_bibliography` 必须写「取到什么算闭合」，否则「等外部」可以永远既不算完成也不算失败。
export const UNDEFINED_REFERENCE_REQUIRED = {
  citation: [],
  missing_bibliography: ['closure'],
};

/// 任务复选框的标签：`- [x] 已完成` / `- [ ] 未完成`。
///
/// 仓库里有 2733 条这种行（`openspec/**/tasks.md` 全是），它们与资料清单条目**字形相同**
/// （都是一个列表项以方括号开头）。`[ ]` 归一后是空标签，已被「空白标签不是引用」那条挡掉；
/// `[x]` 必须显式排除，否则任何文档只要有一条已完成任务，就凭空多出一个名为 `x` 的清单条目。
const TASK_CHECKBOX_LABEL = 'x';

/// 文档域的**唯一定义**：索引里以 `DOC_SUFFIX` 结尾的路径。
///
/// 调用方 MUST 用它来构造文档集，MUST NOT 各自再写一遍过滤条件（两处定义 ⇒ 迟早一处漂移）。
export function documentPathsOf(tracked) {
  const paths = tracked instanceof Set ? [...tracked] : Array.from(tracked ?? []);
  return paths.filter(path => String(path).endsWith(DOC_SUFFIX));
}

/// 把代码段（围栏块 + 行内代码）遮蔽成空格：**字符换空格、换行保留**。
///
/// 为什么要遮蔽而不是「跳过含反引号的行」：链接与反引号可以同处一行（`见 [x](y)，写在 `code` 里`），
/// 按行跳过会因为一个反引号丢掉同一行里的真指针。遮蔽只影响判定，不改行号。
///
/// 围栏识别按 CommonMark 的宽松形态：行首最多 3 个空格 + 至少 3 个 ` 或 ~；闭合要求同字符且不短于开启。
///
/// **已知不精确**：切行用 `/\r?\n/`、拼回用 `\n` ⇒ 在 CRLF 文本上全文长度会变短（每行少一个 `\r`），
/// W153 实测最长的一处差 3215 字符。**行号与行数不变**，所以凡是按行取用的判定都不受影响；
/// 但任何「按全文字符偏移与原文对齐」的用法都会错位 ⇒ 调用方 MUST 逐行对照，MUST NOT 做跨行偏移。
export function maskCode(text) {
  const lines = String(text).split(/\r?\n/);
  const out = [];
  let fence = null;
  for (const line of lines) {
    const open = line.match(/^ {0,3}(`{3,}|~{3,})/);
    if (fence === null) {
      if (open) {
        fence = open[1];
        out.push(' '.repeat(line.length));
        continue;
      }
      out.push(maskInlineCode(line));
      continue;
    }
    if (open && open[1][0] === fence[0] && open[1].length >= fence.length) fence = null;
    out.push(' '.repeat(line.length));
  }
  return out.join('\n');
}

/// 行内代码：成对的反引号串（长度相等才算一对；落单的反引号是字面量，不是代码段）。
function maskInlineCode(line) {
  let result = '';
  let index = 0;
  while (index < line.length) {
    if (line[index] !== '`') {
      result += line[index];
      index += 1;
      continue;
    }
    let run = 0;
    while (line[index + run] === '`') run += 1;
    const marker = '`'.repeat(run);
    const close = line.indexOf(marker, index + run);
    if (close === -1) {
      result += marker;
      index += run;
      continue;
    }
    result += ' '.repeat(close + run - index);
    index = close + run;
  }
  return result;
}

/// 从一份文档里取出所有内联链接：`[标签](url)`，含图片 `![alt](src)`。
///
/// 只认行内形态；返回 `{url, line}`（`line` 是 1 起的行号，用来把失败指到具体那一行——
/// 「某份报告里有一条断链」对修的人是没用的）。
///
/// **已知边界**：一条链接里再嵌一张图片（`[![alt](a.png)](b.md)`）时只取到内层 `a.png`，
/// 外层 `b.md` 取不到。W153 勘定实测全仓 0 处，未修；这条注释就是它的登记。
export function linkTargetsOf(text) {
  const out = [];
  const lines = String(text).split(/\r?\n/);
  lines.forEach((line, index) => {
    const re = /\[[^\]]*\]\(([^)\s]+)(?:\s+"[^"]*")?\)/g;
    let match;
    while ((match = re.exec(line))) out.push({ url: match[1], line: index + 1 });
  });
  return out;
}

/// 引用式标签归一：去首尾空白、内部连续空白折成一个空格、大小写折叠。
///
/// CommonMark 规定 label 的匹配是 case-fold + 空白折叠的。不归一的话 `[x][s2]` 对着 `[S2]:` 会被
/// 判成「无定义」，而任何渲染器都会把它化开——判据错怪记录比漏判更难查（记录方会以为自己写错了）。
export function normalizeLabelOf(label) {
  return String(label ?? '')
    .trim()
    .replace(/\s+/g, ' ')
    .toLowerCase();
}

/// 引用式定义行：`[label]: url`（行首最多 3 个空格；URL 允许 `<...>` 包裹，尾部 title 不取）。
///
/// 只认「URL 与 label 同一行」。CommonMark 也允许 URL 落到下一行，W153 勘定实测全仓 0 处；
/// 不实现即落向「label 无定义 ⇒ 失败」，方向是拒绝。重复定义同一 label 时**每一条都判**——
/// 渲染器取第一条，但后面那几条也是人写下的死指针。
///
/// 返回 `{label, rawLabel, url, line}`：`label` 是归一后的比对键，`rawLabel` 用于**报错**——
/// 把 `[S2]` 报成 `[s2]` 会让读者按小写去文件里找而找不到，那是判据自己制造的第二处缺陷。
export function referenceDefinitionsOf(text) {
  const out = [];
  String(text).split(/\r?\n/).forEach((line, index) => {
    const match = line.match(/^ {0,3}\[([^\]]+)\]:[ \t]*(<[^>]*>|\S+)/);
    if (!match) return;
    const label = normalizeLabelOf(match[1]);
    if (label === '') return;
    out.push({ label, rawLabel: match[1], url: match[2], line: index + 1 });
  });
  return out;
}

/// 引用式使用：`[text][label]`（full）与 `[text][]`（collapsed，label 取 `text`）。
///
/// 返回 `{label, rawLabel, line}`。**不区分 full / collapsed**：判定只用得上归一后的 label
/// （collapsed 取前一个方括号里的文本，两者归一到同一个键），多返回一个 `kind` 就是一处没有消费方的接口。
/// `rawLabel`（W154 补）有两个消费者，且都是必需的：台账的键要写**人看得到的那几个字**，
/// 失败话术 likewise——把 `[S3]` 报成 `[s3]` 会让读者按小写去文件里找而找不到。
/// 不返回 shortcut `[label]`——见文件头：它与普通方括号字形上不可区分，算进来就是拿假指针换覆盖率。
export function referenceUsagesOf(text) {
  const out = [];
  String(text).split(/\r?\n/).forEach((line, index) => {
    const re = /\[([^\]]*)\]\[([^\]]*)\]/g;
    let match;
    while ((match = re.exec(line))) {
      const explicit = match[2];
      const rawLabel = explicit === '' ? match[1] : explicit;
      const label = normalizeLabelOf(rawLabel);
      // 空白标签不是引用（CommonMark：label MUST 含非空白字符）。这条若漏，`[x][ ]` 会被当成
      // collapsed 而凭空多出一条指针。
      if (label === '') continue;
      out.push({ label, rawLabel, line: index + 1 });
    }
  });
  return out;
}

/// 资料清单条目：列表项的第一段以方括号开头。W154 量全仓，这种条目**有三种写法**：
/// `- [S3] 标题`（V3.2）、`- **[S14] 标题**`（V2.3）、`- [S1 标题](url)`（V3.5——方括号里
/// 是「标签 + 标题」的 markdown 链接）。三者都是条目，所以这里**只做抽取**，返回方括号里的
/// 归一文本 `{text, line}`；「这段文本是不是在给 label 作证」由 `bibliographyCovers` 判。
///
/// 抽取与比对分两处，是因为它们答的是两个问题（「这份文档列了哪些条目」vs「其中有没有一条
/// 就是 label」）。但两处 MUST 由同一份归一规则（`normalizeLabelOf`）处理，否则 `[S 2]`
/// 与 `[S2]` 会在一边相等、另一边不等。
///
/// 它**不是**第二种「定义」——定义是 markdown 语法（`[S3]: url`，由渲染器消费），清单条目只是
/// 人读的排版。这里引入它只为一件事：让「`citation` 这条声称」可以被**机器否证**。
///
/// 缩进不设上限：清单条目嵌在标题下的二级列表里同样是条目，判据不该要求排版。
/// 唯一的例外是任务复选框（`- [x] …`），见 `TASK_CHECKBOX_LABEL`。
///
/// 传入的必须是**已遮蔽**的文本：规格里举的「清单条目长什么样」写在代码段里，那不是条目。
export function bibliographyEntriesOf(text) {
  const out = [];
  String(text).split(/\r?\n/).forEach((line, index) => {
    const match = line.match(/^\s*- \*{0,2}\[([^\]]+)\]/);
    if (!match) return;
    const label = normalizeLabelOf(match[1]);
    if (label === '' || label === TASK_CHECKBOX_LABEL) return;
    out.push({ text: label, line: index + 1 });
  });
  return out;
}

/// 一条清单条目是否在给 `label` 作证。
///
/// 两种成立形状，都来自实测：「方括号里只有标签」（`[S3]` / `[S14]`）与「方括号里是标签 +
/// 标题」（`[S1 标题](url)`）。所以判据是**相等**或**以「标签 + 空格」开头**——后者不能在
/// 扫描器里改写成「取第一个词」，因为那样 `[S 2]` 这种带空格的标签会被切成 `s`。
function bibliographyCovers(text, label) {
  return text === label || text.startsWith(`${label} `);
}

/// 外部/非相对目标：判据不判它们。
export function isExternalTarget(url) {
  return /^[a-z][a-z0-9+.-]*:/i.test(url) || url.startsWith('//');
}

/// 把一条链接解析成仓库内路径（保留尾随斜杠，`isDirectory` 靠它判）。
///
/// 解析不了（例如 `%` 不合法的转义）返回 `null`——**不可解析 ⇒ 失败并分类**，
/// MUST NOT 静默跳过（那样断链会以「解析不了」的名义逃逸）。
///
/// 碎片（`#…`）在这里被丢掉：它不属于路径。但**判定侧不因此放行**——`pointerErrorsOf` 会在
/// 调用它之前先把带碎片的指针单独拒掉，本函数不做「碎片是否可达」的声称。
export function resolveTargetOf(fromPath, url) {
  const bare = url.replace(/^<|>$/g, '').split('#')[0];
  if (bare === '') return null;
  let decoded;
  try {
    decoded = decodeURIComponent(bare);
  } catch {
    return null;
  }
  const segments = [];
  const base = fromPath.split('/').slice(0, -1);
  for (const part of [...base, ...decoded.split('/')]) {
    if (part === '' || part === '.') continue;
    if (part === '..') {
      if (segments.length === 0) return null; // 越出仓库根
      segments.pop();
      continue;
    }
    segments.push(part);
  }
  if (segments.length === 0) return null;
  const joined = segments.join('/');
  return decoded.endsWith('/') ? `${joined}/` : joined;
}

/// 一条相对指针的落点判定：通配 / 不可解析 / 索引里没有。返回失败话术或 `null`。
///
/// 内联链接与引用式定义**共用这一处**：同一个问题两处判，迟早一处松。
function targetErrorOf({ fromPath, url, kind, trackedSet, directories }) {
  if (/[*?]/.test(url)) {
    return `the ${kind} "${url}" is a glob — a pointer that stands for several files names none of them, so a reader cannot follow it and a renamer cannot be caught by it`;
  }
  const target = resolveTargetOf(fromPath, url);
  if (target === null) {
    return `the ${kind} "${url}" cannot be resolved inside the repository — an unparseable pointer is a broken pointer, and calling it "not checked" would hide it`;
  }
  if (trackedSet.has(target)) return null;
  const asDirectory = target.endsWith('/') ? target : `${target}/`;
  if (directories.has(asDirectory)) return null;
  return `the ${kind} "${url}" resolves to ${target}, and nothing at that path is tracked — a pointer nobody can follow reads exactly like a record nobody wrote`;
}

/// 判一条指针：碎片在这里被拒，路径部分交给 `targetErrorOf`。
///
/// 返回 `{errors, fragment}`。`fragment` 让调用方知道这条指针落在锚点那一支（报告要用它数）。
function pointerErrorsOf({ fromPath, url, kind, where, trackedSet, directories }) {
  const hash = url.indexOf('#');
  if (hash === -1) {
    const dead = targetErrorOf({ fromPath, url, kind, trackedSet, directories });
    return { errors: dead === null ? [] : [`${where}: ${dead}`], fragment: false };
  }
  const errors = [
    `${where}: the ${kind} "${url}" carries a "#" fragment — how a heading becomes a fragment depends on the renderer, so this criterion cannot tell whether that place exists; reporting it as verified would be a guess, so it is refused instead (drop the fragment, or teach this criterion a slug rule)`,
  ];
  const filePart = url.slice(0, hash);
  if (filePart !== '') {
    const dead = targetErrorOf({ fromPath, url: filePart, kind, trackedSet, directories });
    if (dead !== null) errors.push(`${where}: ${dead}`);
  }
  return { errors, fragment: true };
}

/// 判定入口。
///
/// `documents`：`[{path, text}]`（调用方用 `documentPathsOf` 从索引派生）。
/// `tracked`：索引里的全部路径（数组或 Set）。
/// `undefinedReferenceLedger`：`tests/undefined-references.yaml` 的条目（`{path, label, count, state, closure}`）。
/// **缺省即空台账**——方向是**严格**的：没有认领的未定义引用式使用一律失败，不是「缺了就跳过」。
///
/// 返回 `{errors, documentsChecked, linksChecked, definitionsChecked, referencesChecked,
/// undefinedReferences, undefinedReferenceList, anchorsChecked}`。
/// 计数都是**判据自己记的账**，调用方 MUST NOT 另行统计（「谁构造入参」只能是这一处）：
/// - `linksChecked`：交到判定的内联链接（非外部、代码段外）；
/// - `definitionsChecked`：交到判定的定义行（非外部 URL、代码段外）；
/// - `referencesChecked`：**真有定义**的引用式使用——它是链接，落点在定义那一侧判；
/// - `undefinedReferenceList`：没有定义的引用式使用，**逐一具名**（`{path, line, label, rawLabel, state}`，
///   `state` 是台账给它的认领状态，未认领时是 `null`）。W154 补：计数 ≠ 清单，只回一个数字
///   等于把「是哪几条」这件事留给读者手工 grep，而那正是判据该替他做的事；
/// - `undefinedReferences`：上面那份清单的**长度**（一处的派生，不是第二处统计）；
/// - `anchorsChecked`：前两类里带 `#` 碎片的条数（**子集标记**，不与它们相加）。
export function docLinkErrors({ documents, tracked, undefinedReferenceLedger } = {}) {
  const empty = {
    errors: [
      'no document was provided, so "every relative pointer in this repository resolves to a tracked path" would hold over an empty set',
    ],
    documentsChecked: 0,
    linksChecked: 0,
    definitionsChecked: 0,
    referencesChecked: 0,
    undefinedReferences: 0,
    undefinedReferenceList: [],
    anchorsChecked: 0,
  };
  if (!Array.isArray(documents) || documents.length === 0) return empty;

  const trackedSet = tracked instanceof Set ? tracked : new Set(tracked ?? []);
  if (trackedSet.size === 0) {
    return {
      ...empty,
      errors: [
        'the repository index is empty, so "every relative pointer resolves to a tracked path" would hold over an empty set',
      ],
    };
  }

  const directories = new Set();
  for (const path of trackedSet) {
    const parts = String(path).split('/');
    for (let i = 1; i < parts.length; i += 1) directories.add(`${parts.slice(0, i).join('/')}/`);
  }

  const errors = [];
  let linksChecked = 0;
  let definitionsChecked = 0;
  let referencesChecked = 0;
  let anchorsChecked = 0;
  const undefinedReferenceList = [];
  const maskedByPath = new Map();

  for (const document of documents) {
    const path = String(document.path ?? '');
    // 遮蔽一次，三支判定共用：内联链接 / 定义行 / 引用式使用。各自再遮蔽一遍会让
    // 「哪些字是代码」出现三份答案。
    const masked = maskCode(document.text ?? '');
    maskedByPath.set(path, masked);

    const definitions = referenceDefinitionsOf(masked);
    const definedLabels = new Set(definitions.map(entry => entry.label));

    // ① 内联链接。
    for (const { url, line } of linkTargetsOf(masked)) {
      if (isExternalTarget(url)) continue;
      linksChecked += 1;
      const judged = pointerErrorsOf({
        fromPath: path,
        url,
        kind: 'link target',
        where: `${path}:${line}`,
        trackedSet,
        directories,
      });
      if (judged.fragment) anchorsChecked += 1;
      errors.push(...judged.errors);
    }

    // ② 定义行：URL 照内联链接同一套判。指向站外的定义，其 label 照样算「有定义」，URL 不判。
    for (const { rawLabel, url, line } of definitions) {
      if (isExternalTarget(url)) continue;
      definitionsChecked += 1;
      const judged = pointerErrorsOf({
        fromPath: path,
        url,
        kind: `reference definition "${rawLabel}"`,
        where: `${path}:${line}`,
        trackedSet,
        directories,
      });
      if (judged.fragment) anchorsChecked += 1;
      errors.push(...judged.errors);
    }

    // ③ 引用式使用：不是指针，但**要有人认领**（认领在 ④ 核对）。
    //
    // 按 CommonMark，一个引用式使用只有在 label 有定义时才是链接；没有定义时它退化成**字面文本**，
    // 而原文写的是什么，读者看到的就是什么 ⇒ 它不是指针，不判「指向哪里」。
    // W153 实测全仓 6 处 `[Sx][Sy]`，判定它们「都是引注写法，标签就在同文件的资料清单里」。
    // W154 逐份核对：**这句话成立**（V3.2 的 3 处、V2.3 的 2 处、V3.5 的 1 处，四份清单里都能
    // 找到对应的 `[Sn]` 条目）。但它当时是**人读出来的**，判据验不了——W154 把它变成一条
    // 可被机器否证的声称（`citation`）。
    // ⚠️ 量「有没有清单」时踩了一次坑：V3.5 的清单写成 `- [S1 标题](url)`（方括号里是标签 + 标题的
    // markdown 链接），第一版扫描器要求方括号里**只有** `[Sn]` ⇒ 量出 0 条 ⇒ 差点记成「该文件
    // 根本没有清单」。**报告 0 同样是入参信号**——这是本仓第二次（第一次是 W153 的引用式计数）。
    for (const { label, rawLabel, line } of referenceUsagesOf(masked)) {
      if (definedLabels.has(label)) {
        referencesChecked += 1; // 真有定义 ⇒ 它是链接；它的落点在 ② 已判，不重判
        continue;
      }
      // 没有定义 ⇒ 退化成字面文本，不是指针。但「不是指针」不等于「不需要有人认领」：
      // 一处笔误（本想写链接，少打了一个字符）与一处引注在这里**字形完全一样**，
      // 只有人能分辨 ⇒ 清单里的每一条都必须被台账认领，见 ④。
      undefinedReferenceList.push({ path, line, label, rawLabel, state: null });
    }
  }

  // ④ 未定义引用式使用的认领核对（W154）。
  //
  // 台账是**关于语料的一句声称**，所以两个方向都要核：实际有而台账没有 ⇒ 新的未认领指针；
  // 台账有而实际没有（或条数变了）⇒ 台账过期。两个方向都是红——「登记就不红」会把它变成豁免清单。
  errors.push(...reconcileUndefinedReferences({ list: undefinedReferenceList, ledger: undefinedReferenceLedger, maskedByPath }));

  const undefinedReferences = undefinedReferenceList.length;

  if (linksChecked + definitionsChecked + referencesChecked + undefinedReferences === 0) {
    errors.push(
      'the documents carry no relative link at all, so "every pointer resolves to a tracked path" would hold over an empty set',
    );
  }

  return {
    errors,
    documentsChecked: documents.length,
    linksChecked,
    definitionsChecked,
    referencesChecked,
    undefinedReferences,
    undefinedReferenceList,
    anchorsChecked,
  };
}

/// 台账与语料的对账，并把认领状态写回清单条目。
///
/// 三件事，缺一件这张台账就退化成豁免清单：
/// 1. **认领的形状**（闭世界）：state 必须在枚举内、`missing_bibliography` 必须写 `closure`、
///    `count` 必须是正整数。形状不对的条目**不参与后面的对账**——它没有资格覆盖任何一条用法。
/// 2. **双向**：实际有而台账没有 ⇒ 未认领（红）；台账有而实际没有或条数变了 ⇒ 过期（红）。
/// 3. **声称可被否证**：`citation` 要求同文件真有那条清单条目；`missing_bibliography` 要求
///    同文件**一条清单都没有**。方向相反，所以「随手登记一条」既糊不过去，记录一改还会自己过期。
function reconcileUndefinedReferences({ list, ledger, maskedByPath }) {
  const errors = [];
  const entries = Array.isArray(ledger) ? ledger : [];
  const declared = new Map();
  const keyOf = (path, label) => `${path}\u0000${label}`;

  entries.forEach((entry, index) => {
    const at = `tests/undefined-references.yaml entry #${index + 1}`;
    const path = String(entry?.path ?? '').trim();
    const rawLabel = String(entry?.label ?? '');
    const label = normalizeLabelOf(rawLabel);
    const state = String(entry?.state ?? '');
    const count = entry?.count;

    // 形状先核，**核不过就不登记**：一条形状不对的条目没有资格覆盖任何一条用法。
    // 反过来（先登记再报错）会让「随便写一条 `state: typo`」把那条用法从「未认领」里抹掉。
    const shape = [];
    if (path === '') {
      shape.push(`${at} names no path — a claim about an unnamed document cannot be checked, so it cannot excuse anything`);
    }
    if (label === '') {
      shape.push(`${at} (${path}) names no label — a claim about an unnamed usage cannot be checked either`);
    }
    if (!UNDEFINED_REFERENCE_STATES.includes(state)) {
      shape.push(
        `${at} (${path}, label "${rawLabel}") declares state "${state}", which is not one of ${UNDEFINED_REFERENCE_STATES.join(' / ')} — a state outside the closed set is a claim this criterion has no rule for, and inventing one on the fly is how a ledger stops being checkable`,
      );
    } else {
      for (const field of UNDEFINED_REFERENCE_REQUIRED[state]) {
        if (String(entry?.[field] ?? '').trim() === '') {
          shape.push(
            `${at} (${path}, label "${rawLabel}") is a "${state}" claim without "${field}" — that is exactly what an entry nobody owns looks like: it says the gap is real without saying what would close it`,
          );
        }
      }
    }
    if (!Number.isInteger(count) || count <= 0) {
      shape.push(
        `${at} (${path}, label "${rawLabel}") carries count ${JSON.stringify(count)} — a claim MUST count the usages it covers; covering an unknown number is the same as covering all of them`,
      );
    }
    if (shape.length > 0) {
      errors.push(...shape);
      return;
    }
    declared.set(keyOf(path, label), { at, path, label, rawLabel, state, count });
  });

  const actual = new Map();
  for (const item of list) {
    const key = keyOf(item.path, item.label);
    const bucket = actual.get(key);
    if (bucket) {
      bucket.count += 1;
      bucket.lines.push(item.line);
      continue;
    }
    actual.set(key, { path: item.path, label: item.label, rawLabel: item.rawLabel, count: 1, lines: [item.line] });
  }

  // 方向一：语料里有、台账里没有。
  for (const [key, bucket] of actual) {
    if (declared.has(key)) continue;
    errors.push(
      `${bucket.path}:${bucket.lines[0]}: the reference usage labelled "${bucket.rawLabel}" has no definition in this document, so readers see literal text rather than a link — that alone is not a defect, but nobody claims it either, and a misspelt link looks exactly like a citation here ⇒ add it to tests/undefined-references.yaml (path + label + count) and say which of ${UNDEFINED_REFERENCE_STATES.join(' / ')} it is`,
    );
  }

  // 方向二：台账里有、语料里没有（或条数变了）；条数对上时才去核声称。
  for (const [key, entry] of declared) {
    const bucket = actual.get(key);
    const found = bucket ? bucket.count : 0;
    if (found !== entry.count) {
      errors.push(
        `${entry.at}: ${entry.path} is claimed to carry ${entry.count} use(s) of "${entry.rawLabel}", but it now carries ${found} — a ledger that stopped matching the corpus is a sentence that stopped being true, and a stale excuse is worse than no excuse`,
      );
      continue;
    }
    const bibliography = bibliographyEntriesOf(maskedByPath.get(entry.path) ?? '');
    if (entry.state === 'citation') {
      if (!bibliography.some(item => bibliographyCovers(item.text, entry.label))) {
        errors.push(
          `${entry.at}: "${entry.rawLabel}" in ${entry.path} is claimed to be a citation, but that document carries no "- [${entry.rawLabel}] …" entry — a citation whose source entry is nowhere to be found is the very thing this claim says is fine`,
        );
      }
      continue;
    }
    if (bibliography.length > 0) {
      errors.push(
        `${entry.at}: ${entry.path} is claimed to carry no bibliography at all, but it now carries ${bibliography.length} entr${bibliography.length === 1 ? 'y' : 'ies'} (first at line ${bibliography[0].line}) — the gap has been closed, so the claim MUST stop saying it is open`,
      );
    }
  }

  // 认领状态写回清单：报告与失败话术都要说得出「这一条是谁认领的、按哪种理由」。
  for (const item of list) {
    const entry = declared.get(keyOf(item.path, item.label));
    if (entry && actual.get(keyOf(item.path, item.label))?.count === entry.count) item.state = entry.state;
  }

  return errors;
}
