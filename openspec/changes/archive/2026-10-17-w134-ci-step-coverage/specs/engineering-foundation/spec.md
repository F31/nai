## ADDED Requirements

### Requirement: No CI step re-runs tests another step already covers
同一 workflow 内，两个判定步的测试覆盖集合 MUST NOT 互相包含（含相等）；违反者 SHALL 具名报出**两条步与各自的包集**。已由另一条步覆盖的包集再跑一遍，不产生任何判定，只消耗墙钟。

#### Scenario: A suite the gate already runs
- **WHEN** 门控步覆盖 `nai-storage` 与 `nai-security`，另有一条 `test --suite storage` 步
- **THEN** 判据具名这两条步，指出后者是重复执行

#### Scenario: A suite the gate does not cover
- **WHEN** 一条步跑的是门控不覆盖的东西（`contracts` 的 C++ 夹具）
- **THEN** 它不是重复；判据 MUST NOT 因「同为套件步」而假红

### Requirement: A step's test coverage is computed from the harness
判定步的覆盖集合 SHALL 由 `xtask/src/lib.rs` 的 dispatch 臂与门控区域解析得出，MUST NOT 在声明表里另写一份。实参不可解析 SHALL 具名失败，MUST NOT 静默当成空集。

#### Scenario: The gate coverage is a union
- **WHEN** 门控区域里有多条 `["test", …]`（workspace 全量 + harness 自己的 lib 测试）
- **THEN** 门控覆盖是它们的并集；只取第一条会漏报 harness

#### Scenario: An unresolvable operand
- **WHEN** 某个 suite 的 dispatch 臂找不到、或实参引用了未知常量
- **THEN** 判据具名报「覆盖不可解析」，而不是返回空列表

### Requirement: Every judged step target falls into a coverage class
每个判定步的 `target` SHALL 归入闭集 `gate` / `suite` / `non-test-target` 之一；域外取值 SHALL 具名失败。无类别的目标是**没有覆盖判据的判定**，MUST NOT 被跳过。

#### Scenario: An unknown target shape
- **WHEN** 名册里出现一个形状不在闭集内的目标
- **THEN** 判据具名该步与该目标，报「落入不了任何覆盖类别」

#### Scenario: A class with no instance
- **WHEN** 闭集里某一格在仓库中没有任何实例
- **THEN** 该格的判据对空集恒真，因此仓库 MUST 至少保留一条 `suite` 实例（`contracts`）
