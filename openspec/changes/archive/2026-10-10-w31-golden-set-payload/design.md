# 设计：黄金集载荷为何这样落

## 落点：`xtask`，不是 kernel

判定口径在 W30 已落在 `crates/nai-kernel/src/golden.rs`。载荷读取需要三件事：读文件、解析 JSON、构造 `GoldenSet`。

- **kernel 不能读文件**：它只依赖 contracts，且 `xtask/src/deps.rs` 禁止 kernel 接触 native 依赖。把文件 IO 放进 kernel 会破坏分层。
- **`nai-knowledge-core` 不合适**：它是 shared 层，目前不依赖 serde；把载荷读取放进去要么新增依赖，要么复制分位实现。
- **`xtask` 是 tool 层**：分层规则对 tool 层不设禁止（`_ => false`），它已依赖 `serde_json`，且它的 **lib 目标就在 portable 门控里**（`cargo test --package nai-xtask --lib`）。因此放在 `xtask/src/golden.rs` 的逻辑既被门控覆盖，又不给 kernel 增加 IO 职责。

代价是新增一条依赖边 `xtask → nai-kernel`。这是 tool 层允许的方向，`check-deps` 仍报 18 包、依赖方向不变；换来的是**判定逻辑只有一个真相**（不复制 `audit_coverage`、不复制 `MINIMUM_CASES`、不复制类别表）。

## 格式：JSON，不是 YAML

`xtask` 有 `serde_json`、没有 `serde_yaml`。仓库里唯一的 YAML（`tests/registry.yaml`）由 Node 侧读取（`tests/registry.test.mjs`），Rust 侧**没有**读 YAML 的先例。用 JSON 意味着**外部依赖零新增**；引 YAML 则要新增一个解析器及其传递依赖。`schemas/config/*.schema.json` 已确立 JSON Schema 作为结构声明的惯例，本变更沿用。

## 三种结论互不冒充

这是本设计最要紧的一条。载荷有三种状态：

1. **从未提供** —— 目录不存在，或目录内无 `.json`。这是 `CAPABILITY_NOT_READY`，与 kernel 对「未测量的容量」的处理同形。
2. **提供但结构有缺陷** —— 文件读不出用例集。这是缺陷，逐条报出。
3. **提供且结构合规** —— 再跑 §5.9/§6 的结构审计（题数、类别覆盖、期望相容性）。

若把 1 与 2 都压成「失败」，就无法回答「题集到底有没有」。若把 1 压成「通过」，则一份从未提供的题集会为质量背书——这正是本变更要消除的。因此 `PayloadAudit` 同时保留 `files`、`sets`、`defects` 三个字段：`files == 0` 是「未提供」的唯一判据，而不是靠 `sets.is_empty()` 推断（后者在被拒绝的文件上也为真，会把「有文件但全是垃圾」误报成「未提供」）。

## 载荷边界上的确认人

§5.9 的「经人工确认后扩充黄金集」是一条授权边界。W30 已在 `BadCasePool::promote` 上要求具名确认人；本变更把同一要求前移到**读取载荷**：`origin.kind = promoted` 而 `confirmer` 缺失或空白，文件即在读取阶段被拒。

前移的理由是「失败归因」。若等到运行期才判定，一份含未确认候选的题集会先被当作文档级输入参与召回统计，缺陷便可能被读成「系统在某类问题上表现差」；在读取阶段拒绝，缺陷只能归因于题集本身。

## 未做的事

- **不产生题内容**。题集是场景包载荷，由场景包产出方按场景填入。构造 100 题示例会让人把构造数据读成质量水平。
- **不自动填充**。本变更不提供任何「从日志生成题集」的路径：§5.9 要求人工确认，自动生成会让黄金集变成「系统自己给自己出题」。
- **不纳入 portable**。工程基础门控不该因业务题集未建而变红。
