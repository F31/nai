# W38 实测报告：桌面审批面与 §30.1 的 UI 层（`tests/ui`）

- 日期：2026-10-10
- 变更：`openspec/changes/w38-desktop-approval-surface/`
- 指南：[docs/ui.md](../ui.md)、[docs/control.md](../control.md)
- 判据：`tests/registry.yaml` O02（保持 `pending`）

## 结论

W09 交付了审批 **API**（`GET /approvals`、`POST /approvals/{hash}/approve|reject`）与存储原子日志，但**没有界面**；
设计 §30.1 为 UI 层指定的 `tests/ui` 目录在仓库里**从未存在过**。本轮把两者都补上，并在过程中撞到**两条硬约束**——
其中一条使一项既有验收**当前无法满足**，已如实登记而非绕过去。

**登记状态不变**：O02 保持 `pending`。本轮交付的是**界面侧**能力与**可执行断言**，不是 O02 的完整验收。

## 一、两条硬约束（摸清事实时发现）

### F1 载荷不含候选文本 ⇒「审批按钮显示最终文本」当前无法满足

设计 §662（O02 验收）与 §425（T-14）要求「审批按钮显示最终文本」。本栈做不到，原因是**存储只保存哈希**：

- `migrations/sqlite/0001-foundation.sql:10`——`utterances` 表只有 `text_hash`，没有可展示文本；
- `crates/nai-control-api/src/lib.rs:540`（`approvals_list`）——列表项只含 `id`（即 `candidate_hash`）、元信息引用、`expires_at_utc_ms`、证据计数；
- `schemas/openapi/control.yaml`（`/approvals` :84-97、`ResourceView`/`ResourceItem` :279-299）——载荷里没有文本字段。

**做法**：界面**如实说**——文案 `FINAL_TEXT_GAP` 写明「本载荷不含候选的最终文本（存储只保存 `text_hash`），身份为候选哈希」，
并直接指向未满足的 §662。**不**把哈希当文本展示、**不**含糊地叫「内容」、**不**为此临时发明字段。
补齐它需要存储侧保存可展示文本并同步契约，**不在本轮范围**；已写进 O02 notes 的未满足项。

### F2 候选的有效性与 TTL、会话代次绑定

`crates/nai-storage/src/store.rs:731`（`list_pending_approvals`）在 SQL 里就按 `u.generation_epoch=?3` 与 `u.expires_at_ms>?5` 过滤
（`PendingApproval` :893）。也就是说：**面板上看到的队列是有时效的**，出现「界面还显示着、但已经不该批」是可以发生的正常状态。

**做法**：动作前**复核新鲜度**，比较「面板载入时的队列快照」与「动作前的实时快照」（`approvalFreshness`）——会话换代、候选已不在最新队列、
无有效期、已过期，任一不满足即**不发请求**并给出**具名理由**（含下一步该做什么）。把已失效的候选呈现为可点，本身就是错误的反馈。

## 二、决策（D1–D8，见 design.md）

| # | 决策 | 要点 |
|---|---|---|
| D1 | 界面侧复核，不信任快照 | 快照只作**比较基线**；判据在动作前重取 |
| D2 | 批准二次确认，拒绝单步 | 批准是**高危方向**（放行将要播报的内容）；拒绝是安全方向 |
| D3 | 看不见文本就如实说 | 不发明字段、不放行误解（`FINAL_TEXT_GAP`） |
| D4 | 放 `tests/ui`，不引入 DOM 库 | §30.1 **指定**该目录；逻辑抽成无 DOM 的 `console.ts` 以便断言 |
| D5 | 未实现的断言登记，不伪造 | 四条里两条 `unimplemented` 并**点名缺失前置** |
| D6 | 不改服务端与契约 | 本轮零 Rust 改动、零契约改动 |
| D7 | 登记状态不变 | O02 保持 `pending`，不因界面出现而升级 |
| D8 | 零新增依赖 | UI 层用 Node 内置 `node:test`，不引入 jsdom/playwright |

## 三、改动

| 文件 | 改动 |
| --- | --- |
| `apps/desktop/src/console.ts`（新） | 无 DOM 的可测逻辑：`approvalFreshness`、`nextApprovalStep`（两步确认）、`describeCommandResult`、`runApprovalDecision`、`UI_ASSERTIONS` |
| `apps/desktop/src/control.ts` | 新增 `parseApprovals`（严格校验，`U64` 为字符串、`items` ≤1024）、`parseRuntime`、`ControlClient.approvals()` 与 `ControlClient.decideApproval()`（真实 `ApprovalCommand`：`candidate_hash` 取候选哈希、`hash_algorithm: sha256-canonical-v1`、`seq` 单调、epoch/revision 取自真实视图、TTL 有界） |
| `apps/desktop/src/main.tsx` | 待审批面板（候选哈希/状态/证据计数/有效期/200 上限提示）、批准（二次确认按钮文案）与拒绝；动作委托 `runApprovalDecision`；页脚不再声称「不提供审批按钮」，改为如实说明缺口 |
| `apps/desktop/src/style.css` | 审批列表样式 |
| `apps/desktop/tsconfig.json` | `allowImportingTsExtensions`（Node 的 ESM 解析不做扩展名补全） |
| `package.json` | 新增 `test:ui` |
| `tests/ui/approval-surface.test.mjs`（新） | 11 项：失效拒绝、两步确认、命令字段、假 ACK、严格解析、控制 ACK 口径 |
| `tests/ui/ui-layer-declaration.test.mjs`（新） | 4 项：断言集合恰为四条、状态确定、未实现点名前置 |
| 新增文档 | `docs/ui.md`；`docs/control.md` 新增「审批面（桌面，W38）」节 |
| `tests/registry.yaml` | O02 notes 追加界面侧证据与**未满足项**；状态保持 `pending`；O05 未动 |

**不改**服务端、契约、OpenAPI、存储、`nai-control-api`：本轮 Rust 侧**零改动**，18 包与依赖方向不变。

## 四、实测

```
cargo xtask check --secrets
credential scan passed: 689 files git would publish, no prefixed credential material

cargo xtask check --registry
admission registry check passed (V3.6 acceptance + D01-D14 mapping)

cargo xtask check-deps
dependency boundaries passed: 18 workspace packages (normal/build/dev/target declarations)

cargo xtask check --profile portable
EXIT=0；81 个测试目标（64 Running + 17 Doc-tests）；675 passed / 0 failed

tsc --noEmit（contracts-ts / desktop）  EXIT=0
vite build（desktop）  ✓ 30 modules transformed / dist/assets/index-BE98ObnA.js 206.51 kB（gzip 65.91 kB）/ ✓ built in 2.77s
node --experimental-strip-types --test tests/*.test.mjs（desktop）  # tests 5  # pass 5  # fail 0
npm run test:ui  # tests 15  # pass 15  # fail 0
```

`675 passed / 0 failed` 与 W36/W37 持平：**本轮不新增 Rust 测试目标**（UI 层是 Node 侧，计入 `test:ui`）。

**计数勘误**：此前报告写的「82 个测试目标（65 Running + 17 Doc-tests）」**多算了一个**——`65` 里含 cargo 打印的
`Running ...nai-xtask.exe check --profile portable` 包装行，它不是测试目标。按 `Running <test target>` 精确计数，W36 的日志同样是
**64 + 17 = 81**。本轮起按 81 记录；W36/W37 报告已加注记，不改写其正文。

## 五、未覆盖边界

- **不测渲染**：未引入 DOM 库（jsdom/playwright），故像素、布局、可访问性与真实浏览器行为均不在本层；`tests/ui` 通过**不构成**浏览器或人工 UI 验收。
- **`local_control` 示例未装配审批 backend**：手工点开界面时审批请求仍显式返回 `CAPABILITY_NOT_READY`；`tests/ui` 用替身驱动，证明的是**编排与判据**而非端到端。
- **未满足的验收如实登记**：§662/§425「审批按钮显示最终文本」（F1）。补齐需存储侧保存可展示文本并同步契约。
- **§30.1 两条断言登记为未实现**：「模式切换条件」（缺 `POST /sessions/{id}/mode` 实现，契约见 `schemas/openapi/control.yaml:66`）与「升级与回滚提示」（缺 `nai-upgrade-core` 的 plan/journal 状态来源）——**点名缺失前置**，不伪造已覆盖。
- **不改登记状态**：O02 保持 `pending`，O05 未动；不因「界面出现了」而升级验收状态。
- **仍未建**：真实候选来源、证据详情页、手机审核、生产 root/媒体接线。
