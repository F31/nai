/// **文档内部指针的可达性**（W151 立项，ADR 0125；W152 扩域并补代码段豁免，ADR 0126；
/// W153 把引用式链接纳入域、把锚点改为拒绝，ADR 0127）。
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
///     ⇒ CommonMark 把它降级成**字面文本**，读者看到的就是原文写下的字，**没有读者可见的缺陷**
///     ⇒ 它不是指针，**只数不判**。W153 实测全仓 6 处 `[Sx][Sy]` 全是这一种：标签就活在同一份
///     文档的资料清单里（写成 `- [S3] …` 而不是 `[S3]: …`），把它们判红等于要求记录改排版。
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
/// 返回 `{label, line}`。**不区分 full / collapsed**：判定只用得上归一后的 label（collapsed 取前一个
/// 方括号里的文本，两者归一到同一个键），多返回一个 `kind` 就是一处没有消费方的接口。
/// 不返回 shortcut `[label]`——见文件头：它与普通方括号字形上不可区分，算进来就是拿假指针换覆盖率。
export function referenceUsagesOf(text) {
  const out = [];
  String(text).split(/\r?\n/).forEach((line, index) => {
    const re = /\[([^\]]*)\]\[([^\]]*)\]/g;
    let match;
    while ((match = re.exec(line))) {
      const explicit = match[2];
      const label = normalizeLabelOf(explicit === '' ? match[1] : explicit);
      // 空白标签不是引用（CommonMark：label MUST 含非空白字符）。这条若漏，`[x][ ]` 会被当成
      // collapsed 而凭空多出一条指针。
      if (label === '') continue;
      out.push({ label, line: index + 1 });
    }
  });
  return out;
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
///
/// 返回 `{errors, documentsChecked, linksChecked, definitionsChecked, referencesChecked, undefinedReferences, anchorsChecked}`。
/// 计数都是**判据自己记的账**，调用方 MUST NOT 另行统计（「谁构造入参」只能是这一处）：
/// - `linksChecked`：交到判定的内联链接（非外部、代码段外）；
/// - `definitionsChecked`：交到判定的定义行（非外部 URL、代码段外）；
/// - `referencesChecked`：**真有定义**的引用式使用——它是链接，落点在定义那一侧判；
/// - `undefinedReferences`：没有定义的引用式使用——退化成字面文本，**不是指针**，只数不判；
/// - `anchorsChecked`：前两类里带 `#` 碎片的条数（**子集标记**，不与它们相加）。
export function docLinkErrors({ documents, tracked } = {}) {
  const empty = {
    errors: [
      'no document was provided, so "every relative pointer in this repository resolves to a tracked path" would hold over an empty set',
    ],
    documentsChecked: 0,
    linksChecked: 0,
    definitionsChecked: 0,
    referencesChecked: 0,
    undefinedReferences: 0,
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
  let undefinedReferences = 0;
  let anchorsChecked = 0;

  for (const document of documents) {
    const path = String(document.path ?? '');
    // 遮蔽一次，三支判定共用：内联链接 / 定义行 / 引用式使用。各自再遮蔽一遍会让
    // 「哪些字是代码」出现三份答案。
    const masked = maskCode(document.text ?? '');

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

    // ③ 引用式使用：**只数不判**。
    //
    // 按 CommonMark，一个引用式使用只有在 label 有定义时才是链接；没有定义时它退化成**字面文本**，
    // 而原文写的是什么，读者看到的就是什么 ⇒ 没有读者可见的缺陷 ⇒ 它不是指针，不在这里判。
    // W153 实测把这个假设坐实了：全仓 6 处 `[Sx][Sy]`，6 处都是引注写法（标签就在同一份文档的
    // 资料清单里），把它们的 label 补成定义反而会把整份文档的引注变成链接。
    // 数出来的意义是**不让边界隐身**：这条路径走过多少次、全部落在域外，都要在报告里说得出数。
    for (const { label } of referenceUsagesOf(masked)) {
      if (definedLabels.has(label)) {
        referencesChecked += 1; // 真有定义 ⇒ 它是链接；它的落点在 ② 已判，不重判
        continue;
      }
      undefinedReferences += 1; // 没有定义 ⇒ 退化文本，不是指针
    }
  }

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
    anchorsChecked,
  };
}
