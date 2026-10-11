# W51 设计

## 1. 为什么要存文本，而不是「让界面去问生成侧」

候选的最终文本在生成侧（`v1::PerformanceCue.display_text`）确实存在，但**入库之后**它只以
`text_hash` 留在 `utterances`；审批发生在**异步**的时刻与进程（控制面读存储，不持有生成侧内存）。
若界面靠内存里的生成侧对象显示文本，就要求审批与生成本轮同进程同刻——那正是审批要解耦的东西。
所以正确的位置是**存储**：候选入库时一并保存可展示文本（registry O02 notes 的自裁定亦如此）。

## 2. 迁移策略：只增不改

`migration.rs::verify()` 按**每个文件**的 SHA-256 校验已应用迁移，并对比重建库的 DDL 布局。因此：

- **不得**修改 `0001`（会让所有既有库校验和漂移）；
- 新增 `0006` 用 `ALTER TABLE utterances ADD COLUMN display_text TEXT NOT NULL DEFAULT ''`。

`DEFAULT ''` 让旧库补列后**语义可辨**：空串 = 「该行早于补列，未保存文本」。呈现层据此走具名缺口分支，
而不是显示空内容——这与「未测量 ≠ 0」同一条纪律。

## 3. 身份与文本的关系（本变更的核心边界）

`candidate_hash(cue)`（`nai-contracts/src/lib.rs`，`sha256-canonical-v1`）哈希的是**除 `candidate_hash`
与 `approval_ref` 之外的整个 cue**，`display_text` **在哈希覆盖之内**。但存储只保留哈希与文本两列，
**没有**保留完整 cue，故**无法**由文本重算哈希做本地核对。

因此本变更**不声称**「文本已被哈希校验」，只声称：

- 批准/拒绝的**身份**仍是候选哈希（`ApprovalCommand.candidate_hash`，pattern `^[a-f0-9]{64}$`）；
- 展示文本是**只读**信息，界面不得用它构造命令；
- 文本列被篡改**不会**让审批绑定到另一个候选（身份来自哈希），但**可能误导操作员**——这是一条如实登记的
  剩余风险，缓解手段（保留完整 cue 或存文本哈希）不在本轮范围，记入 ADR 0045。

## 4. 契约变更：`ResourceItem` 增可选 `display_text`

审批列表复用 `ResourceView`/`ResourceItem`（`schemas/openapi/control.yaml`），`id` 即候选哈希。
最小改动是在 `ResourceItem` 增**可选** `display_text`，而不是新造一个审批专用视图：

- 其它资源列表不提供该字段，故必须可选（`additionalProperties:false` 仍关闭未知字段）；
- 字段语义在描述里写死「仅供显示；身份是 `id`；非审批载荷时缺省」。

**生成物必须同步**：`cargo xtask schema --check` 以 `schemas/openapi/control.yaml` 为源，
比对 `crates/nai-contracts/src/generated/control_http.rs`（`SCHEMAS_JSON`）与
`packages/contracts-ts/src/generated/http.ts`。改 yaml 而不重建 ⇒ CI 该步必红。

## 5. 环境边界（必须如实记录）

本轮本机环境**阻断 Node 的同步子进程创建**（`spawnSync`/`execSync` 一律 `EBUSY`，异步 `spawn` 正常）。
`scripts/contracts/generate.mjs` 内部用 `spawnSync` 调 `protoc-gen-ts_proto` 与 `rustfmt`，故
`cargo xtask schema --check` 在本机**必然失败**，与代码无关。本轮的处置是：以**同一份生成逻辑**
（仅把同步 spawn 换成异步、其余不改）在本地重建生成物，再**逐字节比对**；CI（ubuntu/windows）
用未被改动的原脚本执行真正的 `schema --check`。该限制同时使 `scripts/frontend-gate.mjs`（第 176 行
`spawnSync`）在本机不可用，故本机前端门控改为逐条直跑（typecheck/build/`node --test tests/ui`）。

## 6. 验收与不升级

O02 的 `status` 不因本变更升级：验收原文含**桌面渲染**（GUI）与真实候选来源、生产 root 接线，
本轮交付的是「存储保存 + 契约承载 + 呈现口径 + 测试」，**不构成**人工 GUI 验收。按 W38/W49 先例，
`pending` 保持不变并在 notes 记明本次交付与剩余缺口。
