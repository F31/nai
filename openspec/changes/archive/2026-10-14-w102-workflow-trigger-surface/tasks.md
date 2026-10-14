# w102 任务

## 1. 判据模块

- [x] `scripts/contracts/workflow-triggers.mjs`：`TRIGGER_FIELDS`、`TRIGGER_KINDS`、`AUTOMATIC_EVENTS`、`MANUAL_EVENTS`、`FILTER_KEYS`、`WORKFLOW_TRIGGERS`、`workflowTriggersOf`、`triggerKindOf`、`filterKeyOf`、`workflowTriggerErrors`
- [x] R1 事件集双向闭世界：解析出的每个事件在册、在册的每个事件被解析出
- [x] R2 过滤器双向对账：判定单元 `(event, filter-key)`，取值集合双向相等
- [x] R3 至少一条自动触发：在册侧与解析侧**各自**断言
- [x] R4 事件名闭世界（自动表 ∪ 手动表、不相交），类别**由事件表推导**、MUST NOT 存成字段
- [x] R5 形状闭集：`on:` 三写法归一化；过滤器键 ∈ `FILTER_KEYS`；取值非空字符串数组
- [x] R6 字段集闭世界：在册条目的键集 MUST 恰好是 `TRIGGER_FIELDS`
- [x] 缺输入 / 空注册表 / `on:` 缺失或空 / 事件重名 / 不可解析 ⇒ 具名失败，MUST NOT 静默降级

## 2. 在册数据与接线

- [x] `WORKFLOW_TRIGGERS` 在册 `push` 与 `pull_request`（`filters={}`；**不带** `kind`）
- [x] 测试逐字钉 `TRIGGER_FIELDS`，并断言每个在册条目的键集恰好等于它
- [x] 登记进 `xtask/src/lib.rs::engineering_checks`
- [x] 登记进 `scripts/contracts/test-reachability.mjs` 的 `TEST_REGISTRATION_SITES`（14 → 15）
- [x] 同步 `tests/registry.test.mjs` 的逐字站点清单（14 → 15）

## 3. 测试

- [x] `tests/contracts/workflow-triggers.test.mjs`：合成世界（三种 `on:` 写法 + 自动／手动 + 至少一个过滤器单元）+ 真实仓库
- [x] 逐条注入：事件消失 / 无自动触发 / 施加了未声明的过滤器 / 声明了未施加的过滤器 / 取值不等 / 键域外 / 事件名域外 / 手写 `kind` / 字段错拼 / 事件重名 / 缺输入 / 空注册表 / `on:` 缺失或空
- [x] 反向不红：合法的多事件多过滤器声明 MUST NOT 红；声明顺序无关
- [x] 非空洞守卫：三个闭集非空；合成世界覆盖三写法与过滤器；`on:` 缺失与空各有一例

## 4. 验证

- [x] P-C/P-D/P-E 重跑：**由零信号变红**（改真文件 + 真跑 10 套件 + 逐字节还原 + `sha256` 一致）
- [x] 新增边界注入：`on:` 缺失 / 空 / 域外键 / 域外取值 / `kind` 标错 / 手动-only，逐条具名
- [x] 边界实测：与 W95 / W99 / W100 / W101 互不顶替（正交性逐条断言）
- [x] 全量门控 `cargo xtask check --profile full` 通过

## 5. 文档与收尾

- [x] `docs/adr/0080-workflow-trigger-surface.md`
- [x] `.workbuddy/memory/KNOWLEDGE.md` 新增 §6.25，并更新 §10 计数索引
- [x] `AGENTS.md`、`docs/development-plan.md` 追加 W102 段
- [x] `docs/reports/2026-10-14-w102-workflow-trigger-surface.md`（含 §八 遗留：跨步数据传递写方 ↔ 读方无对账）

## 6. 每轮闭环（流程项，**不自我勾选**）

归档与提交按 `AGENTS.md` §0 的闭环执行：门控通过 → tasks 归档前全勾 → `archive --yes`（系统日期戳）
→ 两条 validate 都跑 → 一次 commit + push → 单次低频 CI 观测 → 回填报告（回填提交自身不回填）。
