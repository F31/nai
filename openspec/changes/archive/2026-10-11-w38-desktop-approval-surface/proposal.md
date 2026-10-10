## Why

`tests/registry.yaml` 的 O02（操作员：高危命令二次确认；待审批数量上限与合并；审批按钮显示最终文本，`automation: ui`）
一直是 `pending`，原因是**真实 UI 未建**。W09 已经把审批 API 落地（`GET /approvals`、`POST /approvals/{id}/approve|reject`，
每会话/epoch 上限 200、共享 256 条幂等账本），但桌面控制台自 D14 起只做 status/mute/takeover，
**没有任何审批入口**——API 有、界面无。

同时设计 §30.1 为 UI 层指定了目录与四条核心断言：

| 测试类 | 目录 | 核心断言 |
| --- | --- | --- |
| UI | `tests/ui` | 审批失效、模式切换条件、静音 ACK、升级与回滚提示 |

而 `tests/ui` **在仓库里并不存在**（`tests/` 下只有 `contracts/`、`fixtures/`、`rag-golden/` 与 registry 测试）。
一条被设计指定的证据层从未建立，也就从未被运行。

本轮把这两件事一起做：给桌面控制台落地**审批面**，并建立 `tests/ui` 层。

### 摸清事实时发现的两个硬约束

**一、批准的对象看不见。** 设计 §662（O02 验收）与 §425（T-14）都写明「**审批按钮显示最终文本**」。
但存储层 `utterances` 表**只保存 `text_hash`，不保存文本**（`migrations/sqlite/0001-foundation.sql:10`），
`list_pending_approvals` 也只 `SELECT u.text_hash`
（`crates/nai-storage/src/store.rs:750`），控制面按契约返回的 `ResourceItem`
只有 `id`（候选哈希）/`revision`/`status`/`encoded_contract_ref`/`expires_at_utc_ms`/`evidence_count`
（`schemas/openapi/control.yaml:293`）——**没有任何文本字段**。

所以「审批按钮显示最终文本」在当前载荷下**无法满足**。这不是可以顺手补的 UI 细节：要么改存储（保留播报文本，
与最小化留存相冲突），要么新增一个暴露候选文本的服务端来源。本轮**不**发明字段、**不**改存储，
而是把界面做成**如实声明**：批准前必须让操作员知道「本载荷不含最终文本，身份是候选哈希」。

**二、会被批准的是旧快照。** 审批有 TTL（`expires_at_utc_ms`），且候选的有效性与会话代次
（`generation_epoch`）绑定：`list_pending_approvals` 用 `u.generation_epoch=?3 AND s.generation_epoch=u.generation_epoch`
过滤，服务端在 K 处另有 `session_id`/`device_id` 作用域与 TTL 复核（`approval_action`）。
界面若拿一份列出的快照一直放着，操作员会在**已被处理或已换代**的候选上点批准。故界面侧必须在发请求前**复核**。

## What Changes

- **`apps/desktop/src/console.ts`（新增）**：控制台的可测纯逻辑——审批新鲜度判据、批准的两步确认状态机、
  命令结果消息（含「控制 ACK 非硬件 ACK」）与 §30.1 四条 UI 断言的**实现状态声明**。不含任何 DOM 依赖。
- **`apps/desktop/src/control.ts`（扩展）**：`parseApprovals`（对 `ResourceView.items[]` 做严格校验）+
  `ControlClient.approvals()` / `decideApproval()`（构造真实的 `ApprovalCommand`：`candidate_hash`/`hash_algorithm`/`seq`/epoch/revision/TTL）。
- **`apps/desktop/src/main.tsx`（扩展）**：待审批面板（候选哈希、证据数、有效期、状态、200 上限提示）、
  批准（**二次确认**）与拒绝；**审批失效**（已过期 / 已换代 / 已不在队列）一律**不发请求**并报出具名理由；
  确认面声明「最终文本不在本载荷内」。
- **`tests/ui/`（新增，§30.1 指定目录）**：UI 层断言——审批失效、静音 ACK 诚实、批准命令字段精确，
  并以「声明的断言集合恰为 §30.1 的四条」断言防止漏登记或臆造；两条暂不可实现的断言（模式切换条件、
  升级与回滚提示）**以未实现记录并点名缺失前置**，不伪造通过。
- **`package.json`**：新增 `test:ui`，把该层纳入可执行检查。
- **新增能力条款**（`local-control-surface` 差量）：三条需求（审批新鲜度、高危审批二次确认与不可见项声明、
  UI 层断言声明），共 7 个场景。
- **不改**：控制面服务端（不新增路由、不改载荷）、存储 schema/迁移、契约（`schemas/`）、
  既有 status/mute/takeover 行为；**零新增依赖**（不引入 jsdom 等 DOM 测试库），18 包与依赖方向不变。

## Capabilities

### New Capabilities

无。UI 层是 `local-control-surface` 的界面侧，不另立能力。

### Modified Capabilities

- `local-control-surface`：新增需求「审批动作只在候选仍有效时可用」（3 场景）、
  「高危审批需二次确认并声明不可见项」（2 场景）、「桌面 UI 层声明每条断言的实现状态」（2 场景）。

## Impact

- 桌面控制台从「只有 status/mute/takeover」变为「含待审批面」，`README`/`docs/control.md` 的
  「本轮不提供审批按钮」一句需随之更新。
- `tests/ui` 从不存在变为可执行层，并首次把 §30.1 的 UI 断言集合写成机器可检查的声明。
- **O02 状态不变（保持 `pending`）**：本轮交付的是界面侧的可测部分；「审批按钮显示最终文本」这一条验收
  **在当前载荷下无法满足**，原因已具名记录。UI 验收（真机/手机审批/视觉确认）仍未执行。
- 无服务端行为变化，现有 `nai-control-api` 测试与 `scripts/control-smoke.mjs` 不受影响。

## Non-Goals

- **不**改存储以保存播报文本，**不**新增暴露候选文本的服务端路由，**不**在契约里发明文本字段。
  「审批按钮显示最终文本」作为**未满足的验收**如实记录，而不是用近似物冒充。
- **不**实现 `/sessions/{id}/mode`（契约已定义、控制面未实现）与升级状态来源，故 §30.1 的
  「模式切换条件」「升级与回滚提示」两条断言**不实现、不伪造**，只登记为未实现并点名缺失前置。
- 不引入 DOM 测试库；UI 层测的是可测的纯逻辑（判据、状态机、消息、声明），不测渲染。
- 不据此把 O02/O05 或任何验收项标为达标；界面能点不等于 UI 验收通过。
