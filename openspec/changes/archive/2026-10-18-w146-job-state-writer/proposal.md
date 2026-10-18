# Proposal: W146 — 作业状态的写入点、安装器名册与「能轮询 ⇔ 有安装器」

## Why

W144 把 15 条作业面收成闭世界（每条在册，`kind` 或具名 `blockedBy` 恰有其一），W145 让每条在册项
的分类由**路由器自己的处理**作证。两轮收的都是 `202` 那一侧。

`kind: upgrade` 的另一半——「这个 id 能被 `GET /jobs/{id}` 轮询」——仍然只有两个锚点作保：
`UpgradeRecord` 这个结构在、`"kind": "upgrade"` 这句在被打印。**没有任何判据问过谁写那一行。**

本轮勘定（实测）：`INSERT INTO upgrade_journal` 在已跟踪的 `.rs`+`.sql` 里恰一处（`record_upgrade`）；
推进事务的两个执行入口 `stage_upgrade`／`activate_staged` 的调用点**全部在
`crates/nai-runtime/tests/upgrade.rs`**；`impl UpgradeHost for` 恰一处，是演习替身 `RehearsalHost`。
读面是接上的（`apps/engine/src/lib.rs` 真的 `with_jobs`）⇒ `/jobs/{id}` 的 404 分支可达、
200 分支不可达。

本轮不实现安装器（真值缺口不在仓库内），只把三件事变成可判的：写入点唯一、安装器名册封闭、
能轮询的作业面 ⇔ 生产安装器。

## What Changes

- `scripts/contracts/job-surface.mjs`：
  - `JOB_KINDS[upgrade]` 增加 `writer`（文件 + 逐字锚点 + 逐字写入语句）；
  - 新增 `JOB_HOSTS` 名册（今天恰一项 `RehearsalHost`，`production: false` + `why`）；
  - `jobSourceErrors` 的入参从位置参数改成**具名对象**（六项全必需，缺一项即具名失败），
    新增三条规则：写入语句恰一处且在声明的文件里、`impl UpgradeHost for` 集与名册**双向相等**、
    「声称 `kind` 的作业面 ⇔ 生产安装器」成对。
- `tests/contracts/schema.test.mjs`：既有 8 处调用改用具名对象；+2 条用例
  （真实仓库的写入点唯一性与名册封闭；合成闭世界逐条注入，每条**恰好一条**具名错误）。
- `docs/adr/0120-job-state-writer.md`、轮次报告、`docs/development-plan.md` 追加 W146 段。

## Impact

- **行为零变化**：不改路由、不改存储、不铸任何 job id、不实现安装器。`JobAccepted` 仍然零生产者。
- 判据面：Node +2 条用例（其中一条是合成闭世界，9 个注入）；不新增门控目标、不新增依赖。
- 契约面零改动：`control.yaml` 与生成物都不动。
- **一条注入发现的判据缺陷已就地修**：`writer` 原锚点是 `pub fn record_upgrade`（前缀形态），
  函数改名成 `record_upgrade_v2` 仍然命中 ⇒ 判据不会红。改成 `record_upgrade(&mut self`（调用形态），
  并在用例里加了一条结构断言把结论留下。
