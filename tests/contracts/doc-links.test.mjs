import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import YAML from 'yaml';
import { readTrackedPaths, describeFailure } from '../../scripts/contracts/git-index.mjs';
import {
  bibliographyEntriesOf,
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

/// W151 立项、W152 扩域、W153 补引用式与锚点、W154 把「未定义引用式使用」改成有人认领的名单：
/// **文档内部指针的可达性**（ADR 0125 / 0126 / 0127 / 0128）。
///
/// 判据守的是一段**没人看管**的距离：文档之间的相对链接是「记录的记录」——报告指向 ADR、
/// ADR 指向基线、报告指向归档变更——而**没有任何门控解析 markdown 链接**。W151 实测
/// `docs/` 的 793 条相对链接里有 36 处走不到；W152 把域扩到全仓（991 份已跟踪 `*.md`、
/// 963 条链接、925 条相对），又量出 3 处；W153 把引用式定义纳入域、把锚点从「静默跳过」
/// 改成「拒绝」；W154 让每一处未定义的引用式使用都必须被台账具名认领。
///
/// 本文件分三层，**各自有独立输入域**：
/// 1. 真实语料（域来自 `.git/index`，台账来自 `tests/undefined-references.yaml`）——只证明
///    「今天为绿」，且域是**全量**而非抽样；
/// 2. 合成样本——每条规则各有一个「只有它错时才红」的样本；
/// 3. 域函数本身（`linkTargetsOf` / `referenceDefinitionsOf` / `referenceUsagesOf` /
///    `bibliographyEntriesOf` / `normalizeLabelOf` / `resolveTargetOf` / `isExternalTarget` / `maskCode`）。

/// 真实语料：文档集与索引都从 `.git/index` 派生（未跟踪的文件不在域内），台账从仓库里读。
async function loadCorpus() {
  const index = await readTrackedPaths('.');
  assert.ok(index.ok, describeFailure(index));
  const paths = documentPathsOf(index.paths);
  const documents = [];
  for (const path of paths) documents.push({ path, text: await readFile(path, 'utf8') });
  const ledger = YAML.parse(await readFile('tests/undefined-references.yaml', 'utf8')).entries;
  return { index, paths, documents, tracked: [...index.paths], ledger };
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

/// 台账条目：测试里手写的「认领」。
const claim = (path, label, count, state = 'citation', extra = {}) => ({ path, label, count, state, ...extra });

test('every relative pointer in this repository resolves to a tracked path', async () => {
  const { paths, documents, tracked, ledger } = await loadCorpus();
  const result = docLinkErrors({ documents, tracked, undefinedReferenceLedger: ledger });

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

  // 「未定义引用式使用」这一支（W154）：**每一处都必须被认领**，且认领要报到清单上。
  // 不钉具体数字（记录会增长），钉的是两条性质：这条支路真的走过（否则下面的「全都认领了」
  // 在空集上成立），且清单里没有一条 `state: null`（有就说明有用法没人认领——那种情况
  // `errors` 早就非空了，这里是第二道）。
  assert.equal(Number.isInteger(result.undefinedReferences), true, 'undefinedReferences must be an integer');
  assert.equal(
    result.undefinedReferences,
    result.undefinedReferenceList.length,
    'the count is derived from the list, not counted a second time',
  );
  assert.ok(result.undefinedReferences > 0, 'this branch must actually be exercised by the corpus, or "all claimed" is vacuous');
  const unclaimed = result.undefinedReferenceList.filter(item => item.state === null);
  assert.deepEqual(unclaimed, [], 'every undefined reference in the corpus must be claimed in tests/undefined-references.yaml');
  assert.ok(ledger.length > 0, 'the ledger itself must not be empty — an empty ledger claiming "nothing is undefined" proves nothing');

  // 其余计数只钉「非负整数」：它们的数值随记录增长漂移，钉具体数字等于给每份新报告留一个
  // 必须改的断言。**计数与清单分开报**这一点由 `undefinedReferenceList` 承担（W154）。
  for (const key of ['definitionsChecked', 'referencesChecked', 'anchorsChecked']) {
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
  // 字面文本，读者看到的就是原文写下的字 ⇒ 不判它「指向哪里」，判那个等于判排版。
  // 但「不是指针」不等于「不需要有人认领」（W154）——没定义又没人认领就是红，见下一条用例。
  const definedUse = docLinkErrors({
    documents: [doc('docs/a.md', '[b]: b.md', '见 [x][b]。')],
    tracked: TRACKED,
  });
  assert.deepEqual(definedUse.errors, []);
  assert.equal(definedUse.referencesChecked, 1);
  assert.equal(definedUse.undefinedReferences, 0);
  assert.deepEqual(definedUse.undefinedReferenceList, []);

  // **判别性**：同一个写法，label 一旦没有定义就翻面——计数 1，且它**出现在清单上**（W154：
  // 只回一个数字等于把「是哪几条」留给读者手工 grep）。`rawLabel` 是原文拼写，不是归一后的键。
  const undefinedUse = docLinkErrors({ documents: [doc('docs/a.md', '见 [S2][S3]。')], tracked: TRACKED });
  assert.deepEqual(undefinedUse.undefinedReferenceList, [
    { path: 'docs/a.md', line: 1, label: 's3', rawLabel: 'S3', state: null },
  ]);
  assert.equal(undefinedUse.referencesChecked, 0);

  // collapsed `[label][]` 取方括号里的文本当 label，同样分两种情形。
  const collapsedDefined = docLinkErrors({ documents: [doc('docs/a.md', '[b]: b.md', '见 [b][]。')], tracked: TRACKED });
  assert.deepEqual(collapsedDefined.errors, []);
  assert.equal(collapsedDefined.undefinedReferences, 0);
  const collapsedUndefined = docLinkErrors({ documents: [doc('docs/a.md', '见 [c][]。')], tracked: TRACKED });
  assert.equal(collapsedUndefined.undefinedReferences, 1);

  // 定义按**文档**生效（CommonMark）：a.md 里的定义不能给 b.md 的用法兜底。
  const crossFile = docLinkErrors({
    documents: [doc('docs/a.md', '[b]: b.md'), doc('docs/b.md', '见 [x][b]。')],
    tracked: TRACKED,
  });
  assert.equal(crossFile.referencesChecked, 0, "another document's definition does not count");
  assert.equal(crossFile.undefinedReferences, 1);
  assert.match(crossFile.errors[0], /^docs\/b\.md:1: /, 'the failure must point at the usage, not at the other document');

  // label 的匹配是 case-fold + 空白折叠的：不归一的话 `[x][s 2]` 会被判成「无定义」，
  // 而任何渲染器都会把它化开——判据错怪记录，比漏判更难查（记录方会以为自己写错了）。
  const folded = docLinkErrors({
    documents: [doc('docs/a.md', '[S 2]: b.md', '见 [x][s 2]。')],
    tracked: TRACKED,
  });
  assert.equal(folded.referencesChecked, 1, 'labels match case-insensitively with whitespace collapsed');
  assert.equal(folded.undefinedReferences, 0);
});

test('an undefined reference nobody claims is a failure, and the failure spells the label as written', () => {
  // 一处笔误（本想写链接、少打一个字符）与一处引注在这里**字形完全一样**，判据 MUST NOT 猜。
  // 它能做的只有一件事：要求每一处都有人认领（ADR 0128 决策 1）。
  const undeclared = docLinkErrors({
    documents: [doc('docs/a.md', '[S2][S3]', '与 [S6][S7]。')],
    tracked: TRACKED,
  });
  assert.equal(undeclared.undefinedReferences, 2);
  assert.equal(undeclared.errors.length, 2, JSON.stringify(undeclared.errors));
  assert.match(undeclared.errors[0], /^docs\/a\.md:1: /);
  assert.match(undeclared.errors[0], /labelled "S3"/, 'the raw spelling must be reported — a folded "s3" sends the reader looking for the wrong text');
  assert.match(undeclared.errors[0], /nobody claims it/);
  assert.match(undeclared.errors[0], /tests\/undefined-references\.yaml/, 'the failure must name the place to claim it');
  assert.match(undeclared.errors[1], /^docs\/a\.md:2: /);
  assert.match(undeclared.errors[1], /labelled "S7"/);

  // **判别性**：同一条用法一旦被台账认领，就不再是失败——但前提是声称立得住（见下一条）。
  const claimed = docLinkErrors({
    documents: [doc('docs/a.md', '- [S3] 标题', '- [S7] 另一个来源', '[S2][S3]', '与 [S6][S7]。')],
    tracked: TRACKED,
    undefinedReferenceLedger: [claim('docs/a.md', 'S3', 1), claim('docs/a.md', 'S7', 1)],
  });
  assert.deepEqual(claimed.errors, []);
  assert.deepEqual(
    claimed.undefinedReferenceList.map(item => item.state),
    ['citation', 'citation'],
    'the claim must be reported back on each entry, or the report cannot say who owns it',
  );
});

test('a citation claim is checked against the document own bibliography, and every measured shape counts', () => {
  const ledger = [claim('docs/a.md', 'S3', 1)];

  // 声称是引注，而文档里根本没有那条清单条目 ⇒ 声称倒了。**这是防滥用的一侧**：
  // 只要「登记一条 citation」就能免责，这张台账就是一张豁免清单。
  const noEntry = docLinkErrors({
    documents: [doc('docs/a.md', '见 [S2][S3]。')],
    tracked: TRACKED,
    undefinedReferenceLedger: ledger,
  });
  assert.equal(noEntry.errors.length, 1, JSON.stringify(noEntry.errors));
  assert.match(noEntry.errors[0], /claimed to be a citation/);
  assert.match(noEntry.errors[0], /no "- \[S3\] …" entry/);

  // 三种写法都是条目，全部来自 W154 的实测：V3.2 的 `- [S3] 标题`、V2.3 的 `- **[S14] 标题**`、
  // V3.5 的 `- [S1 标题](url)`（方括号里是标签 + 标题）。只认其中一种会让第三种被误判成「无出处」。
  for (const entry of ['- [S3] 标题', '- **[S3] 标题**', '- [S3 标题](https://example.com)']) {
    const declared = docLinkErrors({
      documents: [doc('docs/a.md', entry, '见 [S2][S3]。')],
      tracked: TRACKED,
      undefinedReferenceLedger: ledger,
    });
    assert.deepEqual(declared.errors, [], `this entry shape must count as a source: ${entry}`);
    assert.equal(declared.undefinedReferenceList[0].state, 'citation');
  }

  // **判别性**：只差一个字符的标签不构成作证（`S30` 不是 `S3` 的条目）。边界必须落在
  // 「标签 + 空格」这个位置上，否则 `S3` 会被任何以 `S3` 开头的条目糊弄过去。
  const neighbour = docLinkErrors({
    documents: [doc('docs/a.md', '- [S30] 另一个来源', '见 [S2][S3]。')],
    tracked: TRACKED,
    undefinedReferenceLedger: ledger,
  });
  assert.equal(neighbour.errors.length, 1, JSON.stringify(neighbour.errors));
  assert.match(neighbour.errors[0], /claimed to be a citation/);

  // 代码段里的清单条目不是条目（规格里要能举「条目长什么样」）。
  const fenced = docLinkErrors({
    documents: [doc('docs/a.md', '```md', '- [S3] 标题', '```', '见 [S2][S3]。')],
    tracked: TRACKED,
    undefinedReferenceLedger: ledger,
  });
  assert.equal(fenced.errors.length, 1, JSON.stringify(fenced.errors));
  assert.match(fenced.errors[0], /claimed to be a citation/);
});

test('a "no bibliography" claim expires the moment the document gains one', () => {
  // `missing_bibliography` 是**另一个方向**的声称：这份文档一条清单都没有，那条引注无出处可核。
  // 没有这个取值，第一处真正的无出处引注就只有「判红」一条路，而修它需要仓外的真实来源。
  const ledger = [claim('docs/a.md', 'S3', 1, 'missing_bibliography', { closure: '补出真实来源清单' })];

  const stillEmpty = docLinkErrors({
    documents: [doc('docs/a.md', '见 [S2][S3]。')],
    tracked: TRACKED,
    undefinedReferenceLedger: ledger,
  });
  assert.deepEqual(stillEmpty.errors, []);
  assert.equal(stillEmpty.undefinedReferenceList[0].state, 'missing_bibliography');

  // **判别性**：一旦文档有了清单，这条声称就是假的 ⇒ 红。登记不是永久的免罪符。
  const closed = docLinkErrors({
    documents: [doc('docs/a.md', '- [S3] 标题', '见 [S2][S3]。')],
    tracked: TRACKED,
    undefinedReferenceLedger: ledger,
  });
  assert.equal(closed.errors.length, 1, JSON.stringify(closed.errors));
  assert.match(closed.errors[0], /no bibliography at all, but it now carries 1 entry/);
  assert.match(closed.errors[0], /MUST stop saying it is open/);

  // 这种状态必须写「取到什么算闭合」——否则「等外部」可以永远既不算完成也不算失败。
  // 注意这里是**两条**红：形状一条，加上「这条条目没资格认领 ⇒ 那条用法仍未认领」一条。
  // 只报形状不报未认领，会让一条 `state: missing_bibliography`（没写 closure）悄悄免责。
  const noClosure = docLinkErrors({
    documents: [doc('docs/a.md', '见 [S2][S3]。')],
    tracked: TRACKED,
    undefinedReferenceLedger: [claim('docs/a.md', 'S3', 1, 'missing_bibliography')],
  });
  assert.equal(noClosure.errors.length, 2, JSON.stringify(noClosure.errors));
  assert.match(noClosure.errors[0], /without "closure"/);
  assert.match(noClosure.errors[1], /nobody claims it/);
});

test('a ledger entry that no longer matches the corpus is a failure, not a note', () => {
  // 台账是**关于语料的一句声称**，所以过期必须红。只要「登记过就永远不红」，它就从台账
  // 退化成豁免清单（MEMORY §7）。方向二：台账有、实际没有或条数变了。
  const ledger = [claim('docs/a.md', 'S3', 2)];

  const shrunk = docLinkErrors({
    documents: [doc('docs/a.md', '- [S3] 标题', '见 [S2][S3]。')],
    tracked: TRACKED,
    undefinedReferenceLedger: ledger,
  });
  assert.equal(shrunk.errors.length, 1, JSON.stringify(shrunk.errors));
  assert.match(shrunk.errors[0], /claimed to carry 2 use\(s\) of "S3", but it now carries 1/);

  const gone = docLinkErrors({
    documents: [doc('docs/a.md', '见 [b](b.md)。')],
    tracked: TRACKED,
    undefinedReferenceLedger: ledger,
  });
  assert.equal(gone.errors.length, 1, JSON.stringify(gone.errors));
  assert.match(gone.errors[0], /but it now carries 0/);

  // 计数是这条声称的**内容**，不是装饰：同一标签出现两次就必须写 2。
  const exact = docLinkErrors({
    documents: [doc('docs/a.md', '- [S3] 标题', '[S2][S3]', '[S2][S3]')],
    tracked: TRACKED,
    undefinedReferenceLedger: ledger,
  });
  assert.deepEqual(exact.errors, []);
});

test('the ledger own shape is closed — an unknown state, a missing closure and a bad count all fail', () => {
  const base = [doc('docs/a.md', '- [S3] 标题', '见 [S2][S3]。')];
  const run = entry => docLinkErrors({ documents: base, tracked: TRACKED, undefinedReferenceLedger: [entry] }).errors;

  assert.match(run({ path: 'docs/a.md', label: 'S3', count: 1, state: 'typo' })[0], /not one of citation \/ missing_bibliography/);
  assert.match(run({ path: 'docs/a.md', label: 'S3', count: 1, state: 'missing_bibliography' })[0], /without "closure"/);
  assert.match(run({ path: 'docs/a.md', label: 'S3', count: 0, state: 'citation' })[0], /carries count 0/);
  assert.match(run({ path: '', label: 'S3', count: 1, state: 'citation' })[0], /names no path/);
  assert.match(run({ path: 'docs/a.md', label: '', count: 1, state: 'citation' })[0], /names no label/);

  // **判别性**：形状不对的条目**不覆盖**任何用法。先登记再报错会让「随手写一条 state: typo」
  // 把那条用法从「未认领」里抹掉——两条都红才是对的（形状一条、未认领一条）。
  const both = run({ path: 'docs/a.md', label: 'S3', count: 1, state: 'typo' });
  assert.equal(both.length, 2, JSON.stringify(both));
  assert.match(both[0], /not one of/);
  assert.match(both[1], /nobody claims it/);
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

test('the bibliography scanner reads the three entry shapes and refuses to see task checkboxes', () => {
  // 三种写法全部来自实测：V3.2 `- [S3] 标题`、V2.3 `- **[S14] 标题**`、
  // V3.5 `- [S1 标题](url)`。第四种「任务复选框」不是来源条目——仓库里有 2733 条这种行。
  const text = [
    '- [S3] 标题',
    '- **[S14] 标题**',
    '- [S1 标题](https://example.com)',
    '- [x] 已完成的任务',
    '- [ ] 未完成的任务',
    '正文里的 [S9] 不是列表项',
    '```md',
    '- [S8] 代码段里的条目不算',
    '```',
  ].join('\n');
  assert.deepEqual(
    bibliographyEntriesOf(maskCode(text)).map(entry => `${entry.line}:${entry.text}`),
    ['1:s3', '2:s14', '3:s1 标题'],
    'the scanner extracts; whether a text covers a label is decided separately',
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
  // `rawLabel` 是**原文拼写**（W154 补）：台账的键与失败话术都要写人看得到的那几个字。
  assert.deepEqual(
    usages.map(entry => entry.rawLabel),
    ['S3', 'b', 'e'],
    'the raw spelling must survive the fold, or the ledger key and the failure message both point at the wrong text',
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
