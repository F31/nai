/// **文档内部指针的可达性**（W151 立项，ADR 0125；W152 扩域并补代码段豁免，ADR 0126）。
///
/// 问题形状：文档之间的相对链接是「记录的记录」——报告指向 ADR、ADR 指向基线、报告指向归档变更。
/// 它们一旦断开，读者落在空处，而**没有任何门控会说一句话**：`--strict` / `--archived` / registry /
/// 各条契约判据都不解析 markdown 链接。W151 实测 `docs/` 的 793 条相对链接里有 36 处缺陷；
/// W152 把域扩到**全仓**（991 份已跟踪 `*.md` / 963 条链接 / 925 条相对），又量出 3 处。
///
/// 判据只做一件事：**每条相对链接的落点，都必须是索引里真的存在的东西**。它不判内容对不对
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
/// - **锚点不在域内**：全仓实测 0 条带 `#anchor` 的链接，锚点 slug 规则随渲染器而变，先不做；
///   **引用式链接不在域内**：全仓实测 0 条 `[x]: url` 定义行，同上。两者都 MUST NOT 当作「已覆盖」。
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

/// 外部/非相对目标：判据不判它们。
export function isExternalTarget(url) {
  return /^[a-z][a-z0-9+.-]*:/i.test(url) || url.startsWith('//');
}

/// 把一条链接解析成仓库内路径（保留尾随斜杠，`isDirectory` 靠它判）。
///
/// 解析不了（例如 `%` 不合法的转义）返回 `null`——**不可解析 ⇒ 失败并分类**，
/// MUST NOT 静默跳过（那样断链会以「解析不了」的名义逃逸）。
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

/// 判定入口。
///
/// `documents`：`[{path, text}]`（调用方用 `documentPathsOf` 从索引派生）。
/// `tracked`：索引里的全部路径（数组或 Set）。
///
/// 返回 `{errors, documentsChecked, linksChecked}`。`linksChecked` 只数**在域内**的链接
/// （代码段之外、相对且非纯锚点），由判据自己记账——调用方 MUST NOT 另行统计
/// （「谁构造入参」只能是这一处）。
export function docLinkErrors({ documents, tracked } = {}) {
  const empty = {
    errors: [
      'no document was provided, so "every relative pointer in this repository resolves to a tracked path" would hold over an empty set',
    ],
    documentsChecked: 0,
    linksChecked: 0,
  };
  if (!Array.isArray(documents) || documents.length === 0) return empty;

  const trackedSet = tracked instanceof Set ? tracked : new Set(tracked ?? []);
  if (trackedSet.size === 0) {
    return {
      errors: [
        'the repository index is empty, so "every relative pointer resolves to a tracked path" would hold over an empty set',
      ],
      documentsChecked: 0,
      linksChecked: 0,
    };
  }

  const directories = new Set();
  for (const path of trackedSet) {
    const parts = String(path).split('/');
    for (let i = 1; i < parts.length; i += 1) directories.add(`${parts.slice(0, i).join('/')}/`);
  }

  const errors = [];
  let linksChecked = 0;

  for (const document of documents) {
    const path = String(document.path ?? '');
    for (const { url, line } of linkTargetsOf(maskCode(document.text ?? ''))) {
      if (isExternalTarget(url)) continue;
      if (url.startsWith('#')) continue;
      linksChecked += 1;

      const where = `${path}:${line}`;
      if (/[*?]/.test(url)) {
        errors.push(
          `${where}: the link target "${url}" is a glob — a pointer that stands for several files names none of them, so a reader cannot follow it and a renamer cannot be caught by it`,
        );
        continue;
      }

      const target = resolveTargetOf(path, url);
      if (target === null) {
        errors.push(
          `${where}: the link target "${url}" cannot be resolved inside the repository — an unparseable pointer is a broken pointer, and calling it "not checked" would hide it`,
        );
        continue;
      }
      if (trackedSet.has(target)) continue;
      const asDirectory = target.endsWith('/') ? target : `${target}/`;
      if (directories.has(asDirectory)) continue;

      errors.push(
        `${where}: the link target "${url}" resolves to ${target}, and nothing at that path is tracked — a pointer nobody can follow reads exactly like a record nobody wrote`,
      );
    }
  }

  if (linksChecked === 0) {
    errors.push(
      'the documents carry no relative link at all, so "every pointer resolves to a tracked path" would hold over an empty set',
    );
  }

  return { errors, documentsChecked: documents.length, linksChecked };
}
