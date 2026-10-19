# Spec delta: W154 — 「未定义引用式使用」的认领台账

## MODIFIED Requirements

### Requirement: Every relative pointer in this repository resolves to a tracked path

每份**已跟踪**的 `*.md` 里的每条 markdown 指针 MUST 解析到 `.git/index` 里存在的路径。
**域 = 「markdown 认为是链接的东西」**，MUST NOT 是「看起来像链接的东西」。代码段（行内与围栏）里的
链接形状不是指针，遮蔽后不参与判定，行号不变。目录算存在；通配（`*`/`?`）MUST 直接失败；外部与
协议相对在域外；越界或不可解码 MUST 分类失败。**引用式定义行 `[label]: url` 在域内**，URL 走与内联
链接同一套判定；**引用式使用**的 label 有本文件定义时它是链接（落点由定义判），无定义时 MUST NOT
视为指针（认领要求见 `Every undefined reference usage is claimed by name`）；shortcut `[label]` 在域外。
**带 `#` 碎片的指针 MUST 拒绝**：碎片怎么生成由渲染器定，判据无从校准，「跳过」等于把看不了的
算进绿里。文档集为空、域内指针为 0 时 MUST 失败。

#### Scenario: An archived change document points at an ADR with the wrong depth

- **WHEN** 归档变更文档（`openspec/changes/archive/<名>/design.md`）里写着
  `[ADR 0044](../../docs/adr/0044-…md)`，而它住在第 4 层，该相对路径解析成
  `openspec/changes/docs/adr/0044-…md`
- **THEN** 判据具名失败并给出文件、行号与解析后的路径。域是整仓时这一条才被看见——
  域是 `docs/` 时它静默通过，而「零错误」看不出差别

#### Scenario: A specification quotes a bad pointer as an example

- **WHEN** 规格在反引号里写着 `[ADR 0016](adr/0016-…md)`，用来说明「这样写是坏的」
- **THEN** 判据 MUST NOT 判它：代码段里的链接形状是被引用的文本，不是本文件承诺要走到的位置。
  同一个形状一旦落在代码段之外，仍 MUST 按断链失败——遮蔽不许变成「不判」

#### Scenario: A pointer that names several files

- **WHEN** 某条链接写成 `[ADR 0051](0051-*.md)`
- **THEN** 判据以「is a glob」具名失败，且 MUST NOT 用「文件不存在」那句措辞：通配说的是
  「我指的是哪一份」这件事没说，它的缺陷是**没说清**，不是**说错了**

#### Scenario: A reference definition that points at nothing

- **WHEN** 某份文档写着 `[S2]: adr/0027-missing.md`，且正文用 `[x][S2]` 引用它
- **THEN** 判据在**定义行**那一行具名失败，并说清它是 `reference definition "s2"` 而不是一条
  普通链接。若只判内联链接，这条指针既不绿也不红——它在判据的视野之外，而不是在判据的保护之下

#### Scenario: A reference usage whose label was never defined here

- **WHEN** 正文写着 `见 [S2][S3]。`，而本文件里没有 `[S3]: …` 定义行（标签只出现在资料清单的
  `- [S3] …` 里）
- **THEN** 判据 MUST NOT 判它「指向哪里」：CommonMark 会把这串括号降级成字面文本，读者看到的
  正是原文写下的字。但它 MUST 出现在 `undefinedReferenceList` 上，且 MUST 被台账认领——
  「不是指针」不等于「不需要有人认领」

#### Scenario: A pointer into a heading

- **WHEN** 某条链接写成 `[本节](#requirements)` 或 `[见安装](deployment.md#安装)`
- **THEN** 判据 MUST 拒绝并说清理由（碎片怎么生成由渲染器定，判据无从校准），MUST NOT 静默跳过：
  「跳过」的意思是「这条我看不了，但我把它算进绿里了」。带文件名的：文件部分仍照判，
  文件与碎片同时坏时 MUST 报两条——那是两个缺陷

#### Scenario: The domain shrinks without a single pointer going bad

- **WHEN** 文档域的过滤条件从「全部已跟踪 `*.md`」退回 `docs/`
- **THEN** 真实语料用例 MUST 失败。它断言的不只是「零错误」，还有「域确实越过了 `docs/`」——
  只断言前者时，缩域会让 3 条归档断链重新隐形，而用例照样全绿

## ADDED Requirements

### Requirement: Every undefined reference usage is claimed by name

每处**未定义**的引用式使用（`[text][label]`，label 无本文件定义）MUST 在
`tests/undefined-references.yaml` 里被**具名认领**，否则失败：一处笔误（本想写链接、少打一个字符）
与一处引注在这里字形相同，判据 MUST NOT 猜。台账键为 `(path, label)` 加出现次数，MUST 双向核对——
实际有、台账无 MUST 失败；台账有而实际无或条数变 MUST 失败（过期的声称与没写声称同样是缺陷）。
`state` 为封闭枚举：`citation` MUST 由同文件资料清单条目作证（方括号里只有标签、加粗、标签 + 标题的
markdown 链接三种写法都算），`missing_bibliography` MUST 在该文件一条清单都没有时成立且必填 `closure`。
`state` 域外、缺必填、`count` 非正整数的条目 MUST NOT 覆盖任何用法。

#### Scenario: A misspelt link that looks exactly like a citation

- **WHEN** 正文写着 `[安装文档][install]`，而本文件没有 `[install]: …` 定义行，台账里也没有这条
- **THEN** 判据在该行具名失败，说出**原文拼写**（`install`，不是归一后的键）与台账路径。
  MUST NOT 因为「它不是指针」就放过：这正是 W153 那条规则留下的洞——一处真正的笔误在那里
  既不绿也不红

#### Scenario: A citation claim is not an excuse by itself

- **WHEN** 台账把某条用法登记为 `citation`，而该文件没有给这个标签作证的清单条目
- **THEN** 判据失败并指出来源条目不存在。只认「登记过」就会把台账变成豁免清单；
  声称 MUST 被否证，MUST NOT 只被记录

#### Scenario: The three measured bibliography shapes all count as a source

- **WHEN** 清单写成 `- [S3] 标题`、`- **[S14] 标题**`、`- [S1 标题](url)` 三种之一
- **THEN** 三者 MUST 都算作证。W154 量全仓时这三种都真实存在，只认一种会把其余两种误判成
  「无出处」——判据错怪记录比漏判更难查。作证的边界是「标签 + 空格」，`S30` MUST NOT 给 `S3` 作证

#### Scenario: A ledger entry that stopped matching the corpus

- **WHEN** 台账写着某文件有 3 处 `[S3]`，而文档后来只剩 2 处（或 0 处）
- **THEN** 判据失败并报出实际条数。台账是**关于语料的一句声称**，过期即是假话；
  「登记过就永远不红」会让它从台账退化成豁免清单

#### Scenario: An orphan citation in a document with no bibliography

- **WHEN** 台账把某条用法登记为 `missing_bibliography` 并写了 `closure`，而该文件确实一条清单都没有
- **THEN** 判据 MUST NOT 失败（问题在仓外：关闭它需要真实来源，编不出来），但 MUST 报到清单上并
  带上认领状态。一旦该文件补出清单，这条登记 MUST 失败——**自行过期**，MUST NOT 留着
