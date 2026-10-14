## ADDED Requirements

### Requirement: Every registered judging step's target is paid for by parsing its command

每个在册判定步（`kind` ∈ `local` / `ci-only`）SHALL 声明它所跑的**目标**（`target`），
且该目标集合 SHALL 由该步的 `run` 文本**解析**得出并与声明**双向相等**。解析出的目标的**形状**
SHALL 落在闭集里。`infra` 与 `diagnostic` 步 SHALL NOT 声明目标，且其 `run` SHALL 解析出
**零个**目标。缺输入、空注册表或不可解析的输入 SHALL 具名失败，而非静默降级。

#### Scenario: A step whose target was swapped to another suite fails

- **WHEN** 一个声明跑 `test --suite kernel` 的判定步，其 `run` 被改成
  `cargo xtask test --suite hosts`
- **THEN** 检查失败并具名该步，指出解析出的目标与声明的目标不一致——一条声明与它实际跑的
  目标分家，就是那条判定面从 CI 静默消失

#### Scenario: A step whose judgement was replaced by another command fails

- **WHEN** 一个判定步的 `run` 被整条换成**另一条合法的**判定命令
  （例如把 `test --suite provider` 换成 `check --secrets`）
- **THEN** 检查失败并具名该步——「这一步像不像一条判定」与「这一步跑的是哪个目标」
  是两件事，前者为真不能让后者免检

#### Scenario: A declared target with no command fails

- **WHEN** 一个判定步声明了目标，而它的 `run` 解析出**空**目标集合
- **THEN** 检查失败并具名该步——一条没有兑现物的目标声明是一句空话

#### Scenario: A judgement hidden in an infrastructure step fails

- **WHEN** 一个 `infra` 或 `diagnostic` 步的 `run` 解析出**非空**目标集合
  （例如把 `node --test …` 塞进「环境准备」）
- **THEN** 检查失败并具名该步——把一条判定标成环境准备是绕过整套检查的后门

#### Scenario: A declared target shape outside the closed set fails

- **WHEN** 一个声明的目标形状不在 `TARGET_SHAPES` 里，或声明了一个空值的目标
- **THEN** 检查失败并具名该处的取值——域外的形状是一个标签，不是一个分类。
  解析一侧的形状由构造保证落在闭集里（`targetsOf` 只产出闭集内的形状），
  **能由结构蕴含的 MUST NOT 再具名一次**；于是「命令不是这三种形状之一」这件事
  由「解析出零个目标」两个方向分别具名

#### Scenario: A missing input or an empty registry fails

- **WHEN** workflow 文本未被提供、为空、或注册表为空
- **THEN** 检查失败并具名缺的是哪一项——一条会因入参缺失而恒真的断言比没有断言更坏
