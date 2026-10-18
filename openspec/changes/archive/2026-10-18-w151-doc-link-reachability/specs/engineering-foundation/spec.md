# Spec delta: W151 — 文档内部指针的可达性

## ADDED Requirements

### Requirement: Every relative pointer under docs/ resolves to a tracked path

每份已跟踪的 `docs/**/*.md` 里的每条相对链接 MUST 解析到 `.git/index` 里存在的路径。
**目录算存在**（该前缀下有已跟踪文件）；**通配（`*` / `?`）MUST 直接失败**，MUST NOT 走
「文件不存在」那条（它永远解析得到东西，否则会在「能解析」的名义下全部逃逸）；外部链接、
协议相对与纯锚点在域外；越界或不可解码 MUST **分类失败**，MUST NOT 静默跳过。
文档集为空、在域内的链接为 0 时 MUST 失败——「零错误」不许由「一条都没看」产生。

#### Scenario: A report points at an ADR that is not there

- **WHEN** `docs/reports/<某报告>.md` 里写着 `[ADR 0016](adr/0016-…md)`，而报告住在
  `docs/reports/`，该相对路径解析成 `docs/reports/adr/0016-…md`
- **THEN** 判据具名失败并给出文件、行号与解析后的路径：读者落在空处不会有任何提示，
  而这份记录从此没人能顺着爬回去

#### Scenario: A pointer that names several files

- **WHEN** 某条链接写成 `[ADR 0051](0051-*.md)`
- **THEN** 判据以「is a glob」具名失败，且 MUST NOT 用「文件不存在」那句措辞：通配说的是
  「我指的是哪一份」这件事没说，它的缺陷是**没说清**，不是**说错了**

#### Scenario: A directory is somewhere a reader can go

- **WHEN** 某条链接指向归档目录（`../../openspec/changes/archive/<变更>/`，可带或不带尾随斜杠），
  该前缀下有已跟踪文件
- **THEN** 通过。把目录判成断链会让判据要求 6 条本来正确的记录改写法——那是判据错，不是记录错

#### Scenario: The judged set is empty

- **WHEN** 调用方给出零份文档，或给出的一批文档里一条相对链接都没有
- **THEN** 判据失败并说明「would hold over an empty set」。`errors` 为空只允许意味着
  「看过且都对」

### Requirement: A dead pointer is either corrected in place or demoted, never guessed

处置 MUST 按「意图是否可复原」二分：路径深度或文件名写错、且正确目标由编号或同目录唯一确定的，
MUST **就地订正 URL**（只改目标字符串，不改任何决策正文）；指向**从未存在过**的文件时，
MUST 把指针**降级为纯文本**并在该记录末尾追加具名勘误段，MUST NOT 猜一个编号补上去——
猜错会把断链换成「看起来能点、内容不对」的指针，那比断链更坏。

#### Scenario: The file was never there

- **WHEN** 某份 ADR 的关联列表指向 `0027-trace-id-single-definition.md`，而 `git log --follow`
  证明 `0027-…` 自创建起就是另一件事
- **THEN** 该指针降级为纯文本、附「见文末勘误」，勘误段写明「编号不可复原」与真实落点
  （若可确定）。六个文件的同类指针各自带一份勘误：读者可能从任意一份进来

#### Scenario: The pointer is wrong but recoverable

- **WHEN** 链接写的是 `0015-decision-log-criticality.md`，而「critical 与 telemetry 的分级」
  的正文出处唯一：ADR 0019 决策 5
- **THEN** 就地订正为 ADR 0019，并在勘误段记下依据（`0019:25` 的逐字原文）。
  「指错了但指得回来」与「指的东西不存在」是两类处置，MUST NOT 合并成一句「修一下链接」
