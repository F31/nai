# Spec delta: W152 — 指针的域与代码段

## REMOVED Requirements

### Requirement: Every relative pointer under docs/ resolves to a tracked path

域从 `docs/` 扩到全仓已跟踪 `*.md`，这条 Requirement 的标题因此变成不实陈述。判定规则本身
（目录算存在 / 通配直接失败 / 外部与锚点在域外 / 空集失败）**原样保留**，改动只在域与代码段两点上，
由下面的新增条目承载。用 `REMOVED` + `ADDED` 而不是 `MODIFIED`：项目的 spec-merge 判据的段标记
闭集里没有 `RENAMED`，而 `MODIFIED` 要求名字不变——两者都不能既改名又保持可重放。

## ADDED Requirements

### Requirement: Every relative pointer in this repository resolves to a tracked path

每份**已跟踪**的 `*.md` 里的每条相对链接 MUST 解析到 `.git/index` 里存在的路径。
**代码段（行内与围栏）里的链接形状不是指针**：它是被引用的文本，遮蔽后不参与判定，行号不变。
**目录算存在**（该前缀下有已跟踪文件）；**通配（`*` / `?`）MUST 直接失败**，MUST NOT 走
「文件不存在」那条；外部链接、协议相对与纯锚点在域外；越界或不可解码 MUST **分类失败**，
MUST NOT 静默跳过。文档集为空、在域内的链接为 0 时 MUST 失败。

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

#### Scenario: The domain shrinks without a single pointer going bad

- **WHEN** 文档域的过滤条件从「全部已跟踪 `*.md`」退回 `docs/`
- **THEN** 真实语料用例 MUST 失败。它断言的不只是「零错误」，还有「域确实越过了 `docs/`」——
  只断言前者时，缩域会让 3 条归档断链重新隐形，而用例照样全绿
