## Why

`tests/registry.yaml` 是唯一的验收来源，其自身规格（`admission-registry`）要求它
「列出**全部**验收项」。但基线 V3.7 §16 与汇总索引 §39.5 把开放平台与技能市场的
**M01–M18** 明确定义为「唯一验收编号体系」的一部分（§37.5 与 §38.10 给出逐条用例），
而注册表里 **M 家族一条都没有**：

```
$ grep -cE '\{id: "M[0-9]' tests/registry.yaml
0
```

这不是「尚未开始」的表达，而是**登记缺失**——`pending` 才是在册未开始的表达。
后果有两层：

1. **验收来源不完整**：R1 退出条件（§37.5：`M01–M07、M13–M16、M18`）与 R2 退出条件
   （`M08–M12、M17`）中有一整段在注册表里不可见，按注册表读发布线会读到一份**漏项**的退出条件。
   而项目规则是「验收与任务卡不一致时以注册表为准」——以一份不完整的注册表为准，会把整个第三部分
   （§33–§38、工作项 W26–W40）从验收视野里抹掉。
2. **判定机制看不到它**：`cargo xtask check --registry` 只校验已登记条目的合法性
   （id 家族、release_line、status、映射悬空），**不校验家族是否齐全**，所以这个缺口能长期不被工具发现。

此外，验证这一缺口时还发现两处附带的不一致：

- **id 家族白名单不含 `M`**：`scripts/contracts/registry.mjs` 的 `ID_RE` 与
  `schemas/tests/registry.schema.json` 的两处 pattern 都是 `^(S|R|A|B|O|P|X)[0-9]{2}$`，
  直接登记 M01 会被校验器判为非法 id。
- **活文档的家族简写本身就不准**：多处写「S/A/R/O/X」，而基线 §39.5 的家族是
  **S/R/A/B/O/P/M/X**——B 与 P 早已有条目（B01–B06、P01），简写却把它们漏了。

## What Changes

- `tests/registry.yaml`：登记 **M01–M18**（title/release_line 取自 §37.5 与 §38.10 逐条表格；
  R1: M01–M07、M13–M16、M18；R2: M08–M12、M17），`status: pending`（在册未开始），
  `automation` 为**规划意图**（设计 §16 要求登记「是否自动化」，但未开始 ⇒ 非测得结论），
  `contract` 仅在 §37.4 为该能力命名了契约时填（M10/M12 留空）。
  `mapping.notes` 补一句：M 家族**不在** D01–D14 映射内（源自 W26–W40）。
- **id 家族白名单扩展到 M**：`scripts/contracts/registry.mjs` 的 `ID_RE`、
  `schemas/tests/registry.schema.json` 的两处 pattern，按 §39.5 顺序改为 `^(S|R|A|B|O|P|M|X)[0-9]{2}$`。
- `tests/registry.test.mjs`：新增**家族守卫**——M01–M18 必须全部在册（缺任一即失败）；
  条目下限随之上调（44 → 66）。
- 活文档的家族简写订正为准确全表 `S/R/A/B/O/P/M/X`：`AGENTS.md`（含 id 正则那处）、
  `docs/design.md`、`docs/development-plan.md`、`openspec/config.yaml`、`README.md`。
- `admission-registry` 新增一条需求：注册表 SHALL 覆盖基线定义的全部验收家族；未开始的家族
  MUST 以 `pending` 在册而 MUST NOT 省略。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `admission-registry`：新增一条需求——**注册表 SHALL 覆盖基线定义的全部验收家族（S/R/A/B/O/P/M/X）；
  未开始 MUST 以 pending 在册，不得省略；校验 MUST 拒绝家族外编号并断言 M 家族在册**。

## Impact

- 修改：`tests/registry.yaml`、`tests/registry.test.mjs`、`scripts/contracts/registry.mjs`、
  `schemas/tests/registry.schema.json`、5 个活文档、`openspec/specs/admission-registry/spec.md`（经归档吸收）、
  报告与 OpenSpec 变更文档。
- **不改任何既有 `status`**：S/R/A/B/O/P/X 共 48 条的 status 一字不动；新增 18 条为 `pending`。
  `pending` 由 3 → 21（其中 18 条是本轮登记的在册未开始）。
- **零 Rust 源码改动、零 workspace 包增减、零新依赖**；`xtask` 只在输出串上支持 `M`（`check --registry`
  本就只跑 Node 脚本，Rust 不解析 YAML）。
- **不做**：不实现 M01–M18 任何一条（W26–W40 未动工）；不把 M 家族并入 `check --profile portable`
  （未实现的目标不应让工程门控变红）；不为「家族齐全性」写通用的设计文档解析器——
  本轮只对 M 家族落一个确定的守卫，通用性留给需要时再评估。
