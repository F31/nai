# Spec delta: W153 — 指针的域收口：引用式链接与锚点

## MODIFIED Requirements

### Requirement: Every relative pointer in this repository resolves to a tracked path

每份**已跟踪**的 `*.md` 里的每条 markdown 指针 MUST 解析到 `.git/index` 里存在的路径。
**域 = 「markdown 认为是链接的东西」**，MUST NOT 是「看起来像链接的东西」。代码段（行内与围栏）里的
链接形状不是指针，遮蔽后不参与判定，行号不变。目录算存在；通配（`*`/`?`）MUST 直接失败；外部与
协议相对在域外；越界或不可解码 MUST 分类失败。**引用式定义行 `[label]: url` 在域内**，URL 走与内联
链接同一套判定；**引用式使用**的 label 有本文件定义时它是链接（落点由定义判），无定义时 CommonMark
把它降级成字面文本 ⇒ MUST NOT 视为指针，只数不判；shortcut `[label]` 在域外。**带 `#` 碎片的指针
MUST 拒绝**：碎片怎么生成由渲染器定，判据无从校准，「跳过」等于把看不了的算进绿里。
文档集为空、域内指针为 0 时 MUST 失败。

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
- **THEN** 判据 MUST NOT 判它失败：CommonMark 会把这串括号降级成字面文本，读者看到的正是原文
  写下的字，**没有读者可见的缺陷**；把它判红就是要求记录改排版。它 MUST 被计进
  `undefinedReferences`——不判不等于不看，边界要数得出来

#### Scenario: A pointer into a heading

- **WHEN** 某条链接写成 `[本节](#requirements)` 或 `[见安装](deployment.md#安装)`
- **THEN** 判据 MUST 拒绝并说清理由（碎片怎么生成由渲染器定，判据无从校准），MUST NOT 静默跳过：
  「跳过」的意思是「这条我看不了，但我把它算进绿里了」。带文件名的：文件部分仍照判，
  文件与碎片同时坏时 MUST 报两条——那是两个缺陷

#### Scenario: The domain shrinks without a single pointer going bad

- **WHEN** 文档域的过滤条件从「全部已跟踪 `*.md`」退回 `docs/`
- **THEN** 真实语料用例 MUST 失败。它断言的不只是「零错误」，还有「域确实越过了 `docs/`」——
  只断言前者时，缩域会让 3 条归档断链重新隐形，而用例照样全绿
