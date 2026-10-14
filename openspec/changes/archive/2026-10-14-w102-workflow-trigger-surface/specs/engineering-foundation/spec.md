## ADDED Requirements

### Requirement: Every workflow's trigger surface is paid for by parsing its `on:` block

每个在册 workflow 的**触发面**（`on:` 里的事件集合及其过滤器）SHALL 由该文件的 YAML **解析**得出，
并与在册的触发器清单**双向相等**；至少一条 SHALL 能由仓库事件**自动**驱动。每条在册触发器 SHALL
恰好携带 `host`／`event`／`filters` 三个字段，其事件名与过滤器键 SHALL 落在各自的闭集里，
自动与手动 SHALL 由事件表推导而非手写。缺输入、空注册表、`on:` 缺失或为空、不可解析的输入
SHALL 具名失败，而非静默降级。

#### Scenario: An event removed from the trigger surface fails

- **WHEN** 一个在册被 `push` 与 `pull_request` 触发的 workflow，其 `on:` 里被删掉 `pull_request`
- **THEN** 检查失败并具名该事件，指出它被声明却没有出现在触发面上——一条消失的触发事件意味着
  那条路径从此不再进门控，而所有步层判据**照样全绿**

#### Scenario: A workflow with no automatic trigger fails

- **WHEN** 整个 `on:` 被缩成仅 `workflow_dispatch`（或任何只在手动事件表里的事件）
- **THEN** 检查失败并具名该 workflow——一个永不自动运行的 workflow 不是门控，
  而它里面的每一条判定步都仍然是「在册」的

#### Scenario: A trigger restriction that nobody declares fails

- **WHEN** `push` 被限定到一个**不存在**的分支（例如 `push: {branches: [nope]}`），
  而在册的 `push` 触发器没有声明该过滤器
- **THEN** 检查失败并具名 `(event, filter-key)` 这个单元及两侧取值——事件集合一个字没变，
  但推送事实上不再触发；这正是「门控静默停止运行」的形状

#### Scenario: A declared filter that the workflow does not apply fails

- **WHEN** 在册触发器声明了过滤器，而该 workflow 的触发面**没有**施加它
- **THEN** 检查失败并具名该单元——一条没有兑现物的过滤器声明是一句空话，
  与「施加了却没人声明」是**同一件事的两个方向**，两者都必须具名

#### Scenario: An event outside the classified set fails

- **WHEN** 触发面或在册清单里出现一个既不属于自动事件表、也不属于手动事件表的事件名
  （例如把 `push` 拼成 `pus`）
- **THEN** 检查失败并具名该事件——若把未知事件一律当成自动触发，一个拼错的名字会让
  「至少一条自动触发」在一条**永不触发**的 workflow 上通过

#### Scenario: A hand-written trigger field fails

- **WHEN** 一条在册触发器带上了 `host`／`event`／`filters` 之外的键
  （例如手写一个 `kind`，或把 `filters` 错拼成 `filter`）
- **THEN** 检查失败并具名该字段——自动与手动由事件表推导，一条**没人会读**的字段是一条
  没有代价的声明：写它的人以为它有意义，而判据从来不看它

#### Scenario: A filter key outside the closed set fails

- **WHEN** 触发面或被声明的一方出现一个不在过滤器键闭集里的键，
  或过滤取值不是非空字符串数组
- **THEN** 检查失败并具名该键与其持有者——域外的键是一个标签，不是一个过滤器；
  它可能是笔误，也可能是一条**没人会去读**的抑制条件

#### Scenario: A missing input, an empty registry, or an empty trigger surface fails

- **WHEN** workflow 文本未被提供、为空、无法解析，在册表为空，或 `on:` 缺失／为空
- **THEN** 检查失败并具名缺的是哪一项——一条会因入参缺失而恒真的断言比没有断言更坏，
  而一个空的触发面本身就是一个「永不运行」的判定
