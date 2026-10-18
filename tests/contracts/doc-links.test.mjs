import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { readTrackedPaths, describeFailure } from '../../scripts/contracts/git-index.mjs';
import {
  docLinkErrors,
  documentPathsOf,
  isExternalTarget,
  linkTargetsOf,
  maskCode,
  resolveTargetOf,
} from '../../scripts/contracts/doc-links.mjs';

/// W151 立项、W152 扩域：**文档内部指针的可达性**（ADR 0125 / ADR 0126）。
///
/// 判据守的是一段**没人看管**的距离：文档之间的相对链接是「记录的记录」——报告指向 ADR、
/// ADR 指向基线、报告指向归档变更——而**没有任何门控解析 markdown 链接**。W151 实测
/// `docs/` 的 793 条相对链接里有 36 处走不到；W152 把域扩到全仓（991 份已跟踪 `*.md`、
/// 963 条链接、925 条相对），又量出 3 处。扩域同时暴露了一个判据缺陷：**代码段里的链接形状
/// 不是指针**——`docs/` 恰好零样本，而 `openspec/` 有 4 条（规格里举的坏例子）。
///
/// 本文件分三层，**各自有独立输入域**：
/// 1. 真实语料（域来自 `.git/index`）——只证明「今天为绿」，且域是**全量**而非抽样；
/// 2. 合成样本——每条规则各有一个「只有它错时才红」的样本（断链 / 目录 / 通配 / 外部 /
///    纯锚点 / 越界 / 不可解析 / 代码段 / 两个空集）；
/// 3. 域函数本身（`linkTargetsOf` / `resolveTargetOf` / `isExternalTarget` / `maskCode`）。

/// 真实语料：文档集与索引都从 `.git/index` 派生（未跟踪的文件不在域内）。
async function loadCorpus() {
  const index = await readTrackedPaths('.');
  assert.ok(index.ok, describeFailure(index));
  const paths = documentPathsOf(index.paths);
  const documents = [];
  for (const path of paths) documents.push({ path, text: await readFile(path, 'utf8') });
  return { index, paths, documents, tracked: [...index.paths] };
}

const errorsOf = (documents, tracked) => docLinkErrors({ documents, tracked }).errors;

const doc = (path, ...lines) => ({ path, text: lines.join('\n') });

/// 一份最小索引：`docs/a.md` 与它指向的三样东西（文件、目录下的文件、外部）。
const TRACKED = [
  'docs/a.md',
  'docs/b.md',
  'docs/adr/0001-x.md',
  'docs/reports/2026-01-01-r.md',
  'openspec/changes/archive/2026-01-01-c/proposal.md',
];

test('every relative pointer in this repository resolves to a tracked path', async () => {
  const { paths, documents, tracked } = await loadCorpus();
  const result = docLinkErrors({ documents, tracked });

  assert.deepEqual(result.errors, [], 'a pointer nobody can follow must be fixed or named, not tolerated');

  // 非空洞四件：域非空、域是**全量**（索引里每一份 *.md 都进过判定，不是抽样）、
  // 在域内的链接非零（否则「零错误」只是因为一条都没看）、文档数守恒。
  assert.ok(paths.length > 0, 'the documentation set is not empty');
  assert.equal(result.documentsChecked, paths.length, 'every tracked document must be judged, not a sample');
  assert.ok(result.linksChecked > 0, 'the corpus must carry relative links, or the green means nothing');
  const seen = new Set(documents.map(entry => entry.path));
  assert.equal(seen.size, paths.length, 'the derived document set must not collapse two paths into one');

  // 域**真的**是全仓，不是 `docs/`：缩回去会让 3 条归档文档的断链重新隐形，而「零错误」看不出差别。
  assert.ok(
    paths.some(path => path.startsWith('openspec/')),
    'the domain must reach beyond docs/, or narrowing it back would be invisible',
  );
  assert.ok(
    paths.some(path => !path.includes('/')),
    'the domain must include top-level documents such as README.md',
  );
});

test('a broken pointer is named with its file and line, and nothing else is', () => {
  // 基线：合规文档不红（否则下面每一条红都可能是别的原因）。
  assert.deepEqual(errorsOf([doc('docs/a.md', '见 [b](b.md)。')], TRACKED), []);

  const broken = errorsOf([doc('docs/a.md', '第一行', '见 [b](missing.md)。')], TRACKED);
  assert.equal(broken.length, 1, JSON.stringify(broken));
  assert.match(broken[0], /^docs\/a\.md:2: /, 'the failure must point at the line that carries the dead pointer');
  assert.match(broken[0], /"missing\.md" resolves to docs\/missing\.md/);
  assert.match(broken[0], /nothing at that path is tracked/);
});

test('a directory is a place a reader can go, so a directory pointer is not a dead pointer', () => {
  // 6 条真实链接指向归档目录（`…/archive/2026-10-07-d01-…/`）。把目录判成断链会让判据去要求
  // 本来正确的记录改写法——那是判据错，不是记录错。
  assert.deepEqual(
    errorsOf([doc('docs/a.md', '归档：[全量](../openspec/changes/archive/2026-01-01-c/)。')], TRACKED),
    [],
  );
  // 不带尾随斜杠的目录同样算存在。
  assert.deepEqual(
    errorsOf([doc('docs/a.md', '归档：[全量](../openspec/changes/archive/2026-01-01-c)。')], TRACKED),
    [],
  );
  // 但一个**没有已跟踪文件**的目录不算存在：前缀存在性来自索引，不来自磁盘。
  const empty = errorsOf(
    [doc('docs/a.md', '归档：[全量](../openspec/changes/archive/2026-02-02-none/)。')],
    TRACKED,
  );
  assert.equal(empty.length, 1, JSON.stringify(empty));
  assert.match(empty[0], /nothing at that path is tracked/);
});

test('a glob is rejected outright — it names several files and therefore none', () => {
  // 4 条真实链接写成 `0051-*.md` / `0052-*.md`。它们**永远解析得到东西**，所以必须单独判，
  // 否则「通配」这一族会在「能解析」的名义下全部逃逸。
  const glob = errorsOf([doc('docs/a.md', '见 [ADR 0051](0051-*.md)。')], TRACKED);
  assert.equal(glob.length, 1, JSON.stringify(glob));
  assert.match(glob[0], /is a glob/, 'a wildcard must be rejected by name, not by the missing-file rule');
  assert.match(glob[0], /docs\/a\.md:1/);

  // `?` 同样是通配，同样不走「文件不存在」那条。
  assert.match(errorsOf([doc('docs/a.md', '见 [x](00?.md)。')], TRACKED)[0], /is a glob/);
});

test('a link shape inside a code span is a quotation, not a pointer', () => {
  // 规格里要能举坏例子：`[ADR 0016](adr/0016-…md)` 写在反引号里，它是**被引用的文本**。
  // 判它等于要求作者不能举例。`docs/` 恰好零样本，只有扩域到 `openspec/` 才看得见这 4 条。
  assert.deepEqual(
    errorsOf([doc('docs/a.md', '见 [b](b.md)。而 `[ADR 0016](adr/0016-…md)` 是举的例子。')], TRACKED),
    [],
  );

  // 围栏块同理：示例代码里出现的链接形状不指向任何地方。
  assert.deepEqual(
    errorsOf([doc('docs/a.md', '见 [b](b.md)。', '```md', '[x](nowhere.md)', '```')], TRACKED),
    [],
  );

  // **判别性**：同一个形状一旦落在代码段之外，就必须照旧判红——否则「遮蔽」变成「不判」。
  const outside = errorsOf([doc('docs/a.md', '见 [ADR 0016](adr/0016-x.md)。')], TRACKED);
  assert.equal(outside.length, 1, JSON.stringify(outside));
  assert.match(outside[0], /nothing at that path is tracked/);

  // 链接与反引号同处一行：真指针不能被一起吞掉（按行跳过含反引号的行就会犯这个错）。
  const mixed = docLinkErrors({
    documents: [doc('docs/a.md', '见 [b](b.md)，也写着 `[x](nowhere.md)`。')],
    tracked: TRACKED,
  });
  assert.deepEqual(mixed.errors, []);
  assert.equal(mixed.linksChecked, 1, 'only the pointer outside the code span is judged');
});

test('the code mask keeps line numbers, and a lone backtick is not an opening fence', () => {
  const text = ['见 [b](b.md)。', '`[x](y.md)`', '```', '[z](w.md)', '```', '`unclosed [q](r.md)'].join('\n');
  const masked = maskCode(text);
  assert.equal(masked.split('\n').length, text.split('\n').length, 'masking must not move line numbers');
  assert.deepEqual(
    linkTargetsOf(masked).map(entry => `${entry.line}:${entry.url}`),
    ['1:b.md', '6:r.md'],
    'the fence hides its body, and an unmatched backtick stays a literal instead of swallowing the rest of the line',
  );
  // 闭合围栏 MUST 同字符且不短于开启：`~~~` 开、``` 关，不构成闭合。
  const tildes = maskCode(['~~~', '[a](a.md)', '```', '[b](b.md)', '~~~'].join('\n'));
  assert.deepEqual(linkTargetsOf(tildes), [], 'a tilde fence is only closed by a tilde fence');
});

test('external targets, pure anchors and reference definitions are outside the judged set, by rule', () => {
  const outside = errorsOf(
    [
      doc(
        'docs/a.md',
        '[b](b.md)',
        '[外链](https://example.com/x.md)',
        '[邮件](mailto:x@example.com)',
        '[协议相对](//cdn.example.com/x.md)',
        '[本节](#requirements)',
      ),
    ],
    TRACKED,
  );
  assert.deepEqual(outside, []);
  assert.ok(isExternalTarget('https://example.com/x.md'));
  assert.ok(isExternalTarget('mailto:x@example.com'));
  assert.ok(isExternalTarget('//cdn.example.com/x.md'));
  assert.equal(isExternalTarget('adr/0001-x.md'), false);

  // 纯锚点是**在域内但跳过**的：它没有路径可解析，不是「没看」。
  const result = docLinkErrors({
    documents: [doc('docs/a.md', '[b](b.md)', '[本节](#requirements)')],
    tracked: TRACKED,
  });
  assert.equal(result.linksChecked, 1, 'the anchor-only link must not be counted as checked');
});

test('a pointer that escapes the repository or cannot be decoded fails, and it is classified as such', () => {
  // 越界：`../../..` 从仓库根出去 ⇒ 不可解析，不是「文件不存在」。
  const escape = errorsOf([doc('docs/a.md', '[上](../../../etc/passwd)')], TRACKED);
  assert.equal(escape.length, 1, JSON.stringify(escape));
  assert.match(escape[0], /cannot be resolved inside the repository/);

  // 非法百分号转义：`decodeURIComponent` 会抛 ⇒ 必须被分类成「不可解析」而非静默跳过。
  const undecodable = errorsOf([doc('docs/a.md', '[x](bad%zz.md)')], TRACKED);
  assert.equal(undecodable.length, 1, JSON.stringify(undecodable));
  assert.match(undecodable[0], /cannot be resolved inside the repository/);

  // 百分号编码**正确**的链接要能通：基线文件名带空格与 `+`，真实记录就是这么写的。
  assert.deepEqual(
    errorsOf([doc('docs/a.md', '[x](adr/0001%2Dx.md)')], TRACKED),
    [],
    'a correctly encoded pointer must resolve — otherwise the rule would punish the right spelling',
  );
});

test('an empty domain is a failure, not a pass', () => {
  assert.match(errorsOf([], TRACKED)[0], /would hold over an empty set/);
  assert.match(errorsOf([doc('docs/a.md', '[b](b.md)')], [])[0], /index is empty/);
  // 一份只有外部链接的文档集：在域内的链接是 0 ⇒ 必须报，MUST NOT 记作「已检查且无问题」。
  const noRelative = errorsOf([doc('docs/a.md', '[外链](https://example.com/x)')], TRACKED);
  assert.equal(noRelative.length, 1, JSON.stringify(noRelative));
  assert.match(noRelative[0], /carry no relative link at all/);
});

test('the link scanner reports the line that carries each pointer', () => {
  const found = linkTargetsOf('# 标题\n\n一段 [a](a.md)，一段 [b](b.md#x)。\n\n![图](c.png)\n');
  assert.deepEqual(
    found.map(entry => `${entry.line}:${entry.url}`),
    ['3:a.md', '3:b.md#x', '5:c.png'],
    'images are pointers too, and the line number is what makes a failure actionable',
  );
});

test('the path resolver is the single place that turns a url into a repository path', () => {
  assert.equal(resolveTargetOf('docs/a.md', 'b.md'), 'docs/b.md');
  assert.equal(resolveTargetOf('docs/a.md', '../openspec/x/y.md'), 'openspec/x/y.md');
  assert.equal(resolveTargetOf('docs/reports/r.md', '../../openspec/changes/archive/c/'), 'openspec/changes/archive/c/');
  assert.equal(resolveTargetOf('docs/a.md', 'b.md#anchor'), 'docs/b.md', 'the anchor is not part of the path');
  assert.equal(resolveTargetOf('docs/a.md', '直播%20V3.11.md'), 'docs/直播 V3.11.md');
  assert.equal(resolveTargetOf('docs/a.md', '../../../etc/passwd'), null);
  assert.equal(resolveTargetOf('docs/a.md', 'bad%zz.md'), null);
  assert.equal(resolveTargetOf('docs/a.md', ''), null);
});
