# Spec delta: W148 — 预构建处方的补集：块外不得有第二份

## ADDED Requirements

### Requirement: The prebuild recipe has exactly one home across the live documents

`docs/engineering.md` 的标记块是预构建处方的唯一出处。判据域内（仓库根与 `docs/` 下的活文档，
`docs/adr/` 与 `docs/reports/` 两段历史除外）**围栏内**任何带 `--no-run` 的 `cargo` 命令
MUST 与块内某条处方一一对上；多出来的、域为空、围栏不配对、块内处方未落在任何围栏内，
MUST 各自具名失败并带路径与行号。散文里的命令不被判。

#### Scenario: A copy sits in a fence outside the block

- **WHEN** 承载块的同一份文档在标记块之外又多出一条围栏内的预构建命令
- **THEN** 判据具名失败并给出 `路径:行号` 与整条命令；块自身那两条已经消费掉配额，
  被点名的 MUST 是副本——报成「块内那条重复」会让读者去改对的那一处

#### Scenario: A copy sits in another live document

- **WHEN** 另一份活文档的围栏里出现同一条预构建命令（哪怕旗标顺序不同）
- **THEN** 判据同样具名失败：一条没有任何规则读的命令，就是漂移之后没人发现的那一条

#### Scenario: The command is only mentioned in prose

- **WHEN** 同一条命令写成行内代码或散文里的反例（`docs/engineering.md` L34–36 那条
  `cargo build --workspace --all-targets`）
- **THEN** 判据 MUST 通过。散文里的命令是用来说明的，围栏里的才是可复制执行的；
  判散文就得为那条反例开豁免名单，而豁免名单正是判据退化的入口

#### Scenario: A fence is never closed

- **WHEN** 某份活文档里有一条没闭合的围栏
- **THEN** 判据具名失败并给出行号：边界不可判定时 MUST 报错，MUST NOT 把「后面全算围栏内」
  当成事实——那既会吞掉真正的散文，也会把反例变成处方

#### Scenario: The judged domain is empty or the recipe is not fenced

- **WHEN** 调用方没给兄弟文档、或块内处方一条也没落在围栏里
- **THEN** 两种情形**分别**具名失败。前者是对空集恒真，后者是「处方不在可扫描域内」——
  一条不在围栏里的处方没有判别力，也不该被当成「已检查」

#### Scenario: The excluded history really does carry copies

- **WHEN** 有人把 `docs/adr/` 或 `docs/reports/` 纳入域
- **THEN** 判据变红，且当时那边**确实**有 5 条围栏内副本（2 条 ADR + 3 条报告）——
  历史不可就地订正，把记录该有的样子判成缺陷，是判据自己的错。这条排除 MUST 由断言付代价，
  MUST NOT 只是注释里的理由
