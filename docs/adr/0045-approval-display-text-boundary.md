# ADR 0045：审批展示文本的边界——展示文本是只读呈现，身份始终是候选哈希

- 日期：2026-10-11
- 状态：Accepted
- 基线：V3.7 §662（O02 高危命令确认）、§425（T-14 审批显示最终文本）、§30.1（测试分层）、§16（发布线退出条件）
- 变更：`openspec/changes/w51-approval-final-text/`
- 判据：`tests/registry.yaml` O02「高危命令确认与审批上限」；`migrations/sqlite/0006-utterance-display-text.sql`

## 背景

设计 §662（O02）与 §425（T-14）要求「**审批按钮显示最终文本**」。W09 交付了审批 API 与存储原子日志、
W38 交付了桌面审批面，但这条例验**一直未满足**：`utterances` 表只保存 `text_hash`
（`migrations/sqlite/0001-foundation.sql:10`），审批列表接口也只返回哈希/元信息引用/期限/证据计数。
W38 的做法是界面**如实具名缺口**（`FINAL_TEXT_GAP`：本载荷不含最终文本、身份为候选哈希），
**不**把哈希当文本展示。W51 补齐存储侧与契约，使这条例验在**逻辑面**成立。要做出的判断有六项：

1. 审批和生成是**异步解耦**的——审批人此刻看到文本，为什么不去问生成侧、而要**存**下来？
2. 迁移怎么加才不破坏既有库的完整性校验？
3. 文本存下来之后，「**身份**」是谁？文本和候选哈希是什么关系？
4. 契约怎么改——`ResourceItem` 加必填还是可选？
5. 本机门控跑不了契约生成（子进程创建被阻断），怎么保证生成物没被人手改坏？
6. 存储字段就位，O02 能不能因此升级为 `tested`？

## 决策

### D1 由存储承载文本，而不是让界面回问生成侧

| 方案 | 做法 | 结论 |
|---|---|---|
| A. 界面/控制面向生成侧实时查询候 selected 文本 | 按需拉取 | **否决**：「审批」与「生成」在架构上**异步解耦**——候选入库时生成侧可能早已退出、会话可能已换代、原始 cue 可能已不被任何进程持有；且会把审批面绑到生成链路的可用性上。 |
| B. 候选入库时把展示文本**一并存下** | `utterances.display_text` | **采纳**：审批是**事后**操作，其依据必须与候选一同**落盘**，才能在生成侧不在场时仍然成立。 |

### D2 迁移只增不改：`0006` 新增列，`DEFAULT ''`

`crates/nai-storage/src/migration.rs::verify()` 按**每个文件**的 SHA-256 校验已应用迁移，
并对比重建库的 DDL 布局。**不得修改 `0001`**（会让所有既有库的校验和漂移）。故：

- 新增 `migrations/sqlite/0006-utterance-display-text.sql`：`ALTER TABLE utterances ADD COLUMN display_text TEXT NOT NULL DEFAULT '';`
- `migration::VERSION` 5 → 6，`SQL` 数组由 5 项增至 6 项；
- `schemas/config/backup.schema.json` 的 `db_schema_version` 由 `{"const":5}` 改为 `{"const":6}`（与 `migration::VERSION` 同步）。

`DEFAULT ''` **有意义**：它标记**早于该列的旧行**（迁移引入前入库的候选没有文本）。
界面必须把空串**具名**为缺口（D4），**不得**把它当「候选文本就是空」。

### D3 展示文本在哈希覆盖范围内，但**无法**由文本重算哈希核对（剩余风险）

`candidate_hash(cue)`（`crates/nai-contracts/src/lib.rs:26`）clone 后清空 `candidate_hash`/`approval_ref`、
置 `hash_algorithm = "sha256-canonical-v1"`，对**整个 cue**（**含 `display_text`**）做 SHA-256。
故文本**在**哈希覆盖范围内。但存储只保留**两列**——`text_hash` 与 `display_text`——**不保留完整 cue**
（会话、证据、性能参数、语音文本等都未随行保存），因此**无法**由文本重算哈希来做本地核对。

| 方案 | 做法 | 结论 |
|---|---|---|
| A. 声称「文本已被哈希校验」 | 表述为完整性保证 | **否决**：这是**不实声明**。重算需要完整 cue，而存储没有。 |
| B. 用存储文本重新计算一个哈希与 `text_hash` 比对 | 局部校验 | **否决**：若把 `text_hash` 的语义悄悄改成「只覆盖文本」，就是**改哈希定义**并让既有行失效；这是对既有契约的破坏，不可取。 |
| C. 如实披露「无法本地重算」，把「文本被篡改可能误导操作员」记为**剩余风险** | 诚实边界 | **采纳**。 |

**这不改变身份语义**（D4）：文本被篡改**不会**让审批绑定到**另一个**候选——身份是候选哈希，命令也以哈希为准；
它**可能误导操作员**（让人以为将要播报的是别的内容），这是本决策**接受**的剩余风险，登记在案而非掩盖。

### D4 展示文本是**只读呈现**，身份**始终**是候选哈希

| 方案 | 做法 | 结论 |
|---|---|---|
| A. 用文本作为审批对象身份 | 文本即 key | **否决**：文本可被改写、可缺失、可重复；身份必须是不变且唯一的东西。 |
| B. 载荷携带文本、但身份仍是 `id`（候选哈希） | `display_text` 只读 | **采纳**：文本供**操作员阅读**，`approve`/`reject` 命令仍以候选哈希为准。界面二次确认文案（`approveConfirmPrompt`）也**点名**「身份为候选哈希」。 |

**缺文本时具名缺口**：`display_text` 为空串 ⇒ 界面显示 `FINAL_TEXT_GAP`（本载荷未随候选提供展示文本），
**绝不**把哈希或空串当作将要播报的内容——这与 W38 的 `FINAL_TEXT_GAP` 文案同源，只是触发条件从「本栈做不到」
变为「该行早于字段」。`tests/ui/approval-surface.test.mjs` 断言 `label !== ''` 且 `text !== id`。

### D5 契约变更选 `ResourceItem` **可选**字段

| 方案 | 做法 | 结论 |
|---|---|---|
| A. 必填字段 | 生成物中 `display_text: string` | **否决**：会**破坏**对旧行的兼容性——早于 `0006` 的候选没有文本，必填会让此类载荷**无法表示**「文本缺失」这一真实状态。 |
| B. 可选字段 | `display_text?: string`（OpenAPI `required` 不含它） | **采纳**：可选**准确表达**了「可能没有」，把缺失交给界面**具名**（D4），而不是用空串冒充。 |

契约改动：`schemas/openapi/control.yaml` 的 `ResourceItem.properties` 增 `display_text`（**description 含逗号必须加引号**，
否则 YAML 流式映射会把它解析成多余键、触发 `additionalProperties: false`）；生成物
`crates/nai-contracts/src/generated/control_http.rs`、`packages/contracts-ts/src/generated/http.ts`、
`packages/contracts-ts/src/generated/config/backup.ts` 随之重建。**生成代码不得手工修改**。

### D6 本机门控跑不了契约生成 ⇒ 用**同一生成逻辑 + 逐字节比对**代替

本机 Node 的**同步子进程创建被阻断**（`spawnSync`/`execSync` 一律 `EBUSY`，managed 22.22.2 与 system 24.13.0
均如此，`dangerouslyDisableSandbox` 亦然），而**异步 `spawn` 正常**。`scripts/contracts/generate.mjs` 三处用
`spawnSync`（第 23/117/129 行）⇒ 在本机**必然失败**。

| 方案 | 做法 | 结论 |
|---|---|---|
| A. 把生成脚本的同步 spawn 永久改成异步 | 改仓库 | **否决**：为一个**主机限制**改仓库代码，会把环境问题固化进源码。 |
| B. **临时**把 `spawnSync` 换成等价异步 shim、跑一次生成、随后**还原脚本** | 门控外的一次性操作 | **采纳**：生成用的是**同一套生成逻辑**（同一 `generate.mjs`、同一 `cargo xtask schema` 的 protoc 步骤），只是把同步等待换成异步；还原后 `git diff` 对该脚本为空。 |
| C. 手工改生成物 | 直接编辑 | **否决**：违反「生成代码不得手工修改」。 |

**验证**：`cargo xtask schema` 生成成功后，`git diff --stat` 对生成脚本 exit=0（已还原）；
生成物 diff 恰为预期行数（`control_http.rs` 1 行、`http.ts` +2 行、`backup.ts` 版本号 5→6）。

### D7 存储字段就位**不**等于 O02 达标：保持 `pending`

`openspec/specs/admission-registry/spec.md`「Honest backfill mapping with traceable evidence」规定：
`tested` 仅可用于**已有归档实测证据**；未完成或**外部阻塞**的目标 MUST 保留 implemented/blocked，
**不得因映射自动升级为 tested**。O02 的真值（**真实候选来源**、**桌面渲染**、**生产 root 接线**、手机审核）
仍未接，故本轮**只补证据、不改 `status`**——**O02 保持 `pending`**，notes 追加本轮成果与剩余项。
这与 W38 的 O02、W49 的 O01/O05 先例一致：**逻辑/界面出现不等于验收达标**。

### D8 审计语义不变：控制面载荷不泄漏语音文本/证据明细/性能参数

`display_text` 是**展示文本**，`speech_text`（语音合成文本）、证据明细与性能参数**不**随之返回。
`crates/nai-control-api/src/tests.rs` 断言列表项**不含** `speech_text`/`candidate_hash`/`performance`/`evidence_refs`
这些键——展示文本的引入**不**扩大载荷暴露面。

## 否决方案

| 方案 | 否决理由 |
| --- | --- |
| 界面回问生成侧取文本 | 审批与生成异步解耦，生成侧可能已不在场；会把审批绑到生成链路可用性 |
| 修改 `0001` 加列 | `verify()` 逐文件 SHA-256 校验，改 `0001` 令所有既有库校验和漂移 |
| 声称「文本已被哈希校验」 | 不实：重算需完整 cue，而存储只留两列 |
| 把 `text_hash` 语义改为「只覆盖文本」 | 改既有契约定义、令既有行失效 |
| 用文本作为审批对象身份 | 文本可改写/缺失/重复，身份必须不变且唯一 |
| `display_text` 设为必填 | 旧行无文本，必填使「文本缺失」不可表示 |
| 手工编辑生成物 | 违反「生成代码不得手工修改」 |
| 永久改写生成脚本以绕过本机限制 | 把主机环境问题固化进源码 |
| 因存储字段就位把 O02 升 `tested` | 违反 admission-registry 的诚实回填；真值未接 |

## 后果

- §662/§425 的「审批显示最终文本」在**逻辑面**成立：`migrations/sqlite/0006-*` + `PendingApproval.display_text`
  + 控制面 `display_text` 载荷 + `ResourceItem` 可选字段 + `console.ts` 的 `candidateText`/`approveConfirmPrompt`。
- **身份语义不变**：审批对象始终是候选哈希，命令也以哈希为准；展示文本是**只读呈现**。
- **接受一项剩余风险**：文本被篡改**不会**让审批绑定到另一候选，但**可能误导操作员**（无本地重算能力）。
- registry 的 **O02 保持 `pending`**（真实候选来源/桌面渲染/生产 root 接线/手机审核未接），notes 追加本轮成果。
- 迁移一致性由 `verify()` 保证；`backup.schema.json` 的 `db_schema_version` 与 `migration::VERSION` 同步为 6。
