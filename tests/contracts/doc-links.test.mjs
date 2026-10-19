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
  normalizeLabelOf,
  referenceDefinitionsOf,
  referenceUsagesOf,
  resolveTargetOf,
} from '../../scripts/contracts/doc-links.mjs';

/// W151 立项、W152 扩域、W153 补引用式与锚点：**文档内部指针的可达性**
/// （ADR 0125 / ADR 0126 / ADR 0127）。
///
/// 判据守的是一段**没人看管**的距离：文档之间的相对链接是「记录的记录」——报告指向 ADR、
/// ADR 指向基线、报告指向归档变更——而**没有任何门控解析 markdown 链接**。W151 实测
/// `docs/` 的 793 条相对链接里有 36 处走不到；W152 把域扩到全仓（991 份已跟踪 `*.md`、
/// 963 条链接、925 条相对），又量出 3 处；W153 把引用式定义纳入域、把锚点从「静默跳过」
/// 改成「拒绝」。
///
/// 本文件分三层，**各自有独立输入域**：
/// 1. 真实语料（域来自 `.git/index`）——只证明「今天为绿」，且域是**全量**而非抽样；
/// 2. 合成样本——每条规则各有一个「只有它错时才红」的样本；
/// 3. 域函数本身（`linkTargetsOf` / `referenceDefinitionsOf` / `referenceUsagesOf` /
///    `normalizeLabelOf` / `resolveTargetOf` / `isExternalTarget` / `maskCode`）。

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

  // 计数与清单必须分开报：把「没定义」的引用式写法计成 0 会让「域边界」在报告里消失
  // （W153：今天 6 处，全是引注写法，都不是缺陷）。这里**只钉字段存在且是非负整数**，
  // 不钉具体数字——那个数会随记录增长而漂移，钉它等于给每份新报告留一个必须改的断言。
  for (const key of ['definitionsChecked', 'referencesChecked', 'undefinedReferences', 'anchorsChecked']) {
    assert.equal(Number.isInteger(result[key]), true, `${key} must be reported as an integer`);
    assert.ok(result[key] >= 0, `${key} must not be negative`);
  }
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

test('external targets are outside the judged set, by rule', () => {
  const outside = errorsOf(
    [
      doc(
        'docs/a.md',
        '[b](b.md)',
        '[外链](https://example.com/x.md)',
        '[邮件](mailto:x@example.com)',
        '[协议相对](//cdn.example.com/x.md)',
      ),
    ],
    TRACKED,
  );
  assert.deepEqual(outside, []);
  assert.ok(isExternalTarget('https://example.com/x.md'));
  assert.ok(isExternalTarget('mailto:x@example.com'));
  assert.ok(isExternalTarget('//cdn.example.com/x.md'));
  assert.equal(isExternalTarget('adr/0001-x.md'), false);

  // 只有外部链接的文档集：在域内一条都没有 ⇒ 必须报空集，MUST NOT 记作「已检查且无问题」。
  const noRelative = errorsOf([doc('docs/a.md', '[外链](https://example.com/x)')], TRACKED);
  assert.equal(noRelative.length, 1, JSON.stringify(noRelative));
  assert.match(noRelative[0], /carry no relative link at all/);
});

test('a fragment is refused instead of skipped — the destination cannot be verified', () => {
  // 为什么是**拒绝**而不是「跳过」：标题怎么变成碎片由渲染器决定（GitHub 的 github-slugger、
  // GitLab、编辑器预览、静态站生成器各一套规则；中文标题更是各写各的），判据无从校准。
  // 而「跳过」的意思是「这条我看不了，但我把它算进绿里了」——W151 起的一贯立场是拒绝。
  const pure = docLinkErrors({ documents: [doc('docs/a.md', '[b](b.md)', '[本节](#requirements)')], tracked: TRACKED });
  assert.equal(pure.errors.length, 1, JSON.stringify(pure.errors));
  assert.match(pure.errors[0], /^docs\/a\.md:2: /);
  assert.match(pure.errors[0], /carries a "#" fragment/);
  assert.match(pure.errors[0], /depends on the renderer/, 'the failure must say why it cannot be judged');
  assert.equal(pure.anchorsChecked, 1, 'the refused fragment must be counted, not silently dropped');
  assert.equal(pure.linksChecked, 2, 'a fragment-bearing link is still a link that was looked at');

  // 带文件名的碎片：文件部分照判，碎片本身单独失败。
  const withFile = errorsOf([doc('docs/a.md', '见 [b](b.md#安装)。')], TRACKED);
  assert.equal(withFile.length, 1, JSON.stringify(withFile));
  assert.match(withFile[0], /carries a "#" fragment/);

  // **判别性**：文件部分本身也不存在时，两件事都要说——碎片不可判，和文件走不到，
  // 是两个缺陷，合并成一条会让修的人只改一半。
  const both = errorsOf([doc('docs/a.md', '见 [b](missing.md#安装)。')], TRACKED);
  assert.equal(both.length, 2, JSON.stringify(both));
  assert.match(both[0], /carries a "#" fragment/);
  assert.match(both[1], /nothing at that path is tracked/);
});

test('a reference definition is judged exactly like an inline link', () => {
  // 域的形状变了（W153）：定义行的 URL 走**同一套**可达性判定。W153 勘定前全仓定义行 0 条，
  // 所以这条规则的全部证据只能是合成样本——这不妨碍它是判据，只是说明它今天还没被真实语料检验过。
  assert.deepEqual(errorsOf([doc('docs/a.md', '[b]: b.md', '见 [x][b]。')], TRACKED), []);

  const broken = errorsOf([doc('docs/a.md', '[b]: missing.md')], TRACKED);
  assert.equal(broken.length, 1, JSON.stringify(broken));
  assert.match(broken[0], /^docs\/a\.md:1: /, 'the failure must point at the definition line');
  assert.match(broken[0], /reference definition "b"/, 'the message must name what kind of pointer it is');
  assert.match(broken[0], /nothing at that path is tracked/);

  // 通配、越界、不可解析：定义行与内联链接共用同一处判定，所以这三族也必须同时成立
  // （**同一件事 MUST NOT 两处定义**：如果定义行另写一套，迟早一处松）。
  assert.match(errorsOf([doc('docs/a.md', '[b]: 00?.md')], TRACKED)[0], /is a glob/);
  assert.match(errorsOf([doc('docs/a.md', '[b]: ../../../etc/passwd')], TRACKED)[0], /cannot be resolved/);
  assert.match(errorsOf([doc('docs/a.md', '[b]: bad%zz.md')], TRACKED)[0], /cannot be resolved/);

  // 定义指向站外：URL 不判，但**label 照样算「有定义」**——否则 `[x][b]` 会被当成退化文本，
  // 而渲染器明明会把它化成一条外链。
  const external = docLinkErrors({
    documents: [doc('docs/a.md', '[b]: https://example.com/x', '见 [x][b]。')],
    tracked: TRACKED,
  });
  assert.deepEqual(external.errors, []);
  assert.equal(external.definitionsChecked, 0, 'an external definition target is not judged');
  assert.equal(external.referencesChecked, 1, 'but the label still resolves, so the usage is a pointer');
  assert.equal(external.undefinedReferences, 0);
});

test('a reference usage is a pointer only when its label is defined here', () => {
  // 这是 W153 最容易被写错的一格：`[x][y]` 在 y 无定义时**不是链接**，CommonMark 把它降级成
  // 字面文本，读者看到的就是原文写下的字 ⇒ 没有读者可见的缺陷 ⇒ 判它等于判排版。
  // 全仓 6 处 `[Sx][Sy]` 全是这一种（标签活在同一份文档的 `- [S3] …` 清单里）。
  const undefinedUse = docLinkErrors({
    documents: [doc('docs/a.md', '见 [S2][S3]。')],
    tracked: TRACKED,
  });
  assert.deepEqual(undefinedUse.errors, [], 'an undefined reference degrades to text, it is not a dead pointer');
  assert.equal(undefinedUse.undefinedReferences, 1, 'but it MUST be counted, or the boundary goes invisible');
  assert.equal(undefinedUse.referencesChecked, 0);

  // **判别性**：同一个写法，label 一旦有定义就变成指针——计数必须跟着翻面。
  const definedUse = docLinkErrors({
    documents: [doc('docs/a.md', '[b]: b.md', '见 [x][b]。')],
    tracked: TRACKED,
  });
  assert.equal(definedUse.referencesChecked, 1);
  assert.equal(definedUse.undefinedReferences, 0);

  // collapsed `[label][]` 取方括号里的文本当 label，同样分两种情形。
  assert.deepEqual(errorsOf([doc('docs/a.md', '[b]: b.md', '见 [b][]。')], TRACKED), []);
  const collapsedUndefined = docLinkErrors({ documents: [doc('docs/a.md', '见 [c][]。')], tracked: TRACKED });
  assert.deepEqual(collapsedUndefined.errors, []);
  assert.equal(collapsedUndefined.undefinedReferences, 1);

  // 定义按**文档**生效（CommonMark）：a.md 里的定义不能给 b.md 的用法兜底。
  const crossFile = docLinkErrors({
    documents: [doc('docs/a.md', '[b]: b.md'), doc('docs/b.md', '见 [x][b]。')],
    tracked: TRACKED,
  });
  assert.deepEqual(crossFile.errors, []);
  assert.equal(crossFile.referencesChecked, 0, "another document's definition does not count");
  assert.equal(crossFile.undefinedReferences, 1);

  // label 的匹配是 case-fold + 空白折叠的：不归一的话 `[x][s 2]` 会被判成「无定义」，
  // 而任何渲染器都会把它化开——判据错怪记录，比漏判更难查（记录方会以为自己写错了）。
  const folded = docLinkErrors({
    documents: [doc('docs/a.md', '[S 2]: b.md', '见 [x][s 2]。')],
    tracked: TRACKED,
  });
  assert.equal(folded.referencesChecked, 1, 'labels match case-insensitively with whitespace collapsed');
  assert.equal(folded.undefinedReferences, 0);
});

test('a reference definition inside a code span is a quotation too', () => {
  // 与内联链接同一条豁免：规格里要能写出「定义行长什么样」。
  const fenced = docLinkErrors({
    documents: [doc('docs/a.md', '[b]: b.md', '```md', '[c]: nowhere.md', '```')],
    tracked: TRACKED,
  });
  assert.deepEqual(fenced.errors, []);
  assert.equal(fenced.definitionsChecked, 1, 'only the definition outside the fence is judged');

  const inline = docLinkErrors({
    documents: [doc('docs/a.md', '[b]: b.md', '写法是 `[c]: nowhere.md`。')],
    tracked: TRACKED,
  });
  assert.deepEqual(inline.errors, []);
  assert.equal(inline.definitionsChecked, 1);

  // 用法在代码段里同理：不数、不判。
  const usedInCode = docLinkErrors({
    documents: [doc('docs/a.md', '[b]: b.md', '写法是 `[x][c]`。')],
    tracked: TRACKED,
  });
  assert.equal(usedInCode.undefinedReferences, 0, 'a shape inside a code span is not a usage');
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
});

test('the link scanner reports the line that carries each pointer', () => {
  const found = linkTargetsOf('# 标题\n\n一段 [a](a.md)，一段 [b](b.md#x)。\n\n![图](c.png)\n');
  assert.deepEqual(
    found.map(entry => `${entry.line}:${entry.url}`),
    ['3:a.md', '3:b.md#x', '5:c.png'],
    'images are pointers too, and the line number is what makes a failure actionable',
  );
});

test('the reference scanners separate a definition from a usage, and keep the original spelling', () => {
  assert.deepEqual(
    referenceDefinitionsOf(['[b]: b.md', '   [c]: <d e.md> "标题"', '- [S3] 不是定义行', '    [i]: i.md'].join('\n')).map(
      entry => `${entry.line}:${entry.label}:${entry.url}`,
    ),
    ['1:b:b.md', '2:c:<d e.md>'],
    'up to three leading spaces; a four-space indent is code, and a list item is not a definition',
  );
  // 定义行的 label 也只认到 `]` 为止，且 URL 与 label 必须同一行。
  assert.deepEqual(referenceDefinitionsOf('[b]:'), []);
  assert.deepEqual(referenceDefinitionsOf('正文里 [b]: b.md 不算定义行'), []);

  const usages = referenceUsagesOf(['见 [S2][S3]。', '与 [b][]，与 [c][ ]', '与 [d][e]'].join('\n'));
  assert.deepEqual(
    usages.map(entry => `${entry.line}:${entry.label}`),
    ['1:s3', '2:b', '3:e'],
    'collapsed takes the bracket text as its label; a whitespace-only label is not a reference at all',
  );

  // 定义行 MUST 连原文标签一起返回：报错时说 `[s2]` 会让读者按小写去文件里找而找不到，
  // 那是判据自己制造的第二处缺陷。
  assert.equal(referenceDefinitionsOf('[S 2]: b.md')[0].rawLabel, 'S 2');
  assert.equal(referenceDefinitionsOf('[S 2]: b.md')[0].label, 's 2');

  assert.equal(normalizeLabelOf('  S  2  '), 's 2');
  assert.equal(normalizeLabelOf(''), '');
});

test('the path resolver is the single place that turns a url into a repository path', () => {
  assert.equal(resolveTargetOf('docs/a.md', 'b.md'), 'docs/b.md');
  assert.equal(resolveTargetOf('docs/a.md', '../openspec/x/y.md'), 'openspec/x/y.md');
  assert.equal(resolveTargetOf('docs/reports/r.md', '../../openspec/changes/archive/c/'), 'openspec/changes/archive/c/');
  assert.equal(
    resolveTargetOf('docs/a.md', 'b.md#anchor'),
    'docs/b.md',
    'the fragment is not part of the path — the refusal to judge it lives in the criterion, not here',
  );
  assert.equal(resolveTargetOf('docs/a.md', '直播%20V3.11.md'), 'docs/直播 V3.11.md');
  assert.equal(resolveTargetOf('docs/a.md', '../../../etc/passwd'), null);
  assert.equal(resolveTargetOf('docs/a.md', 'bad%zz.md'), null);
  assert.equal(resolveTargetOf('docs/a.md', ''), null);
});
