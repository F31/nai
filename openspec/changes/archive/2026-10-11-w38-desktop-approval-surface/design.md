# 设计：桌面审批面与 tests/ui 层

## 背景

三件事同时成立，才构成本轮：

1. **API 有、界面无**：W09 落地了审批 API（`GET /approvals`、`POST /approvals/{id}/approve|reject`，
   每会话/epoch 上限 200、共享 256 条幂等账本、临界日志原子提交），但桌面控制台自 D14 起只有
   status/mute/takeover，O02（`automation: ui`）因此停在 `pending`。
2. **约定的层不存在**：§30.1 为 UI 指定 `tests/ui` 与四条核心断言（审批失效、模式切换条件、静音 ACK、
   升级与回滚提示），而 `tests/` 下只有 `contracts/`、`fixtures/`、`rag-golden/` 与 registry 测试。
3. **两条硬约束**（本该在开工前就查清，见「发现」一节）：
   - 批准的对象**看不见文本**；
   - 会被批准的可能**是旧快照**。

## 发现

### F1 载荷不含候选文本，故「审批按钮显示最终文本」当前无法满足

| 位置 | 事实 |
|---|---|
| `migrations/sqlite/0001-foundation.sql:10` | `utterances` 表列为 `... text_hash TEXT NOT NULL ...`——**只存哈希** |
| `crates/nai-storage/src/store.rs:750` | `list_pending_approvals` 的 SELECT 取 `u.text_hash`，不取文本 |
| `schemas/openapi/control.yaml:293` | `ResourceItem` 只有 `id`/`revision`/`status`/`encoded_contract_ref`/`expires_at_utc_ms`/`evidence_count` |
| 设计 §662 / §425 | O02 验收与 T-14 都要求「审批按钮显示最终文本」 |

对照 `crates/nai-control-api/src/lib.rs:562`：列表项的 `id` **就是候选哈希**
（`"id": item.candidate_hash`），`revision` 是 `approval_id`。所以界面能拿到的身份是哈希，不是文本。

### F2 候选的有效性与 TTL、会话代次绑定

`list_pending_approvals` 的 WHERE 含 `u.generation_epoch=?3 AND s.generation_epoch=u.generation_epoch`
与 `u.expires_at_ms>?5`（`store.rs:755`），服务端 `approval_action` 另复核 `session_id`/`device_id`
作用域与命令 TTL（`lib.rs:615`、`lib.rs:637`）。**列表是一份快照**：会话换代或候选被处理后，
旧快照里的项不再有效。

## 决策

### D1 界面侧复核，而不是信任快照

审批动作前重新读取 `/runtime/status` 与 `/approvals`，用**纯函数**判定三项新鲜度：候选仍在最新队列、
未过期、会话 `generation_epoch` 与列出时一致。任一不满足即**不发请求**并报具名理由。
判据做成不依赖 DOM 的纯函数，故 §30.1 的「审批失效」可以在 `tests/ui` 里被断言，而不需要浏览器。

否决「信任快照直接批准」：那正是 T-14（操作员误操作）的成因。
否决「让服务端兜住」：服务端**应该**拒绝，但界面把无效动作呈现为可点，本身就是错误反馈。

### D2 批准要二次确认，拒绝不要

批准是**高危方向**（放行内容将要播报），故首次点击只进入待确认态，第二次点击才提交；换候选会重新置位。
拒绝是**安全方向**（不产生播报），单次点击即可。

两次确认状态机做成纯 reducer（`nextApprovalStep`），可被直接断言「单击不发出请求」。

### D3 看不见文本就如实说，不发明字段、不放行误解

F1 决定了本轮**不改**存储、**不新增**暴露候选文本的路由、**不在契约里发明**文本字段。
界面在确认面声明：**本载荷不含候选最终文本，身份是候选哈希**，并指向 O02/T-14。

否决「用 `encoded_contract_ref` 或截断哈希冒充文本展示」：那是把不可见说成可见。
否决「顺手给 `utterances` 加一列文本」：保留播报原文与最小化留存相冲突，属独立设计决策，
不应夹带在 UI 里程碑里。

### D4 UI 层放在 §30.1 指定的 `tests/ui`，且不引入 DOM 测试库

`tests/ui/` 新建，断言运行在 Node（`--experimental-strip-types`），复用桌面应用既有的
「纯逻辑 + 注入 fetcher」模式（见 `apps/desktop/tests/control.test.mjs`）。新增根脚本 `test:ui`。

否决引入 jsdom/Testing Library：本轮要断言的是**判据与消息**，不是渲染；加一个 DOM 依赖会把
「界面诚实」变成「DOM 长得对」，且与零新增依赖的现状不符。

### D5 未实现的断言登记，不伪造

§30.1 的四条断言里，两条所需的界面路径不存在：

| 断言 | 状态 | 缺失前置 |
|---|---|---|
| 审批失效 | 实现 | — |
| 静音 ACK | 实现 | — |
| 模式切换条件 | **未实现** | `POST /sessions/{id}/mode` 契约已定义、控制面未实现（`control.yaml:66`） |
| 升级与回滚提示 | **未实现** | 控制面无升级状态来源（未暴露 `nai-upgrade-core` 的 plan/journal） |

`tests/ui` 声明这四条及其状态，并断言**声明的集合恰为四条**（复用本项目惯用的「审计并集恰为 N」
写法）：既能证明每条被登记，也能防止将来偷偷加一条未登记的断言。对未实现的两条，断言其状态为
「未实现且点名了缺失前置」，而**不是**断言通过。

### D6 不改服务端与契约

控制面路由、载荷、存储 schema/迁移、`schemas/` 全部不动。界面只用契约已有的
`ResourceView.items[]`（`ResourceItem`）读、用 `ApprovalCommand` 写。这样本轮**不可能**通过改服务端
让测试变绿，界面必须真的适配既有载荷——包括适配它的**缺失**。

### D7 登记状态不变

O02 保持 `pending`：本轮交付的是界面侧可测部分，而它的验收（高危二次确认已可演示，但
「审批按钮显示最终文本」未满足；真机/手机审批与视觉验收未执行）尚未达成。一次能点的界面不是 UI 验收。

### D8 零新增依赖

新增的 `console.ts` 只依赖标准库与 `control.ts`；`tests/ui` 只依赖 `node:test`/`node:assert`。
18 包、依赖方向与外部依赖均不变。

## 后果

- 界面能拦下失效候选，但**不能**让操作员看到将被播报的文本——这是本轮的**已知缺口**，
  以 F1 与其验收项具名记录，而不是靠 UI 措辞掩盖。
- `tests/ui` 首次存在，但其覆盖是**部分的**（2/4），且这一部分性本身是被断言的（D5）。
- §30.1 的另外两条断言各需要一个尚未实现的前置：`/sessions/{id}/mode` 与一个升级状态来源。
  它们成为后续里程碑的具名入口，而不是含糊的「以后再说」。
