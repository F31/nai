## 1. 规格与设计

- [x] 1.1 建立 OpenSpec 变更 `w38-desktop-approval-surface`，新增 `local-control-surface` 差量需求（3 条需求 / 7 个场景），`validate --strict` 通过。
- [x] 1.2 落设计决策 D1–D8（界面侧复核、二次确认与拒绝单向、看不见文本就如实说、层放在 tests/ui 且不引入 DOM 库、未实现断言登记不伪造、不改服务端与契约、登记状态不变、零新增依赖）。
- [x] 1.3 把两条硬约束写成发现 F1（载荷无候选文本 ⇒「审批按钮显示最终文本」当前无法满足）与 F2（有效性与 TTL/会话代次绑定），各自附可核对的文件行号。

## 2. 控制台纯逻辑（可测面）

- [x] 2.1 新增 `apps/desktop/src/console.ts`：审批新鲜度判据 `approvalFreshness`（未过期、仍在最新队列、会话代次未变；有效期缺失按不可用处理）。
- [x] 2.2 同文件：批准的两步确认状态机 `nextApprovalStep`（首次点击不提交、换候选重新置位）。
- [x] 2.3 同文件：命令结果消息 `describeCommandResult`——变更类命令必须写明「控制 ACK 非硬件 ACK」；审批确认面消息必须写明「本载荷不含最终文本、身份为候选哈希」并指向验收项。
- [x] 2.4 同文件：`UI_ASSERTIONS` 声明 §30.1 四条及其实现状态；未实现两条点名缺失前置。
- [x] 2.5 该模块不得引入 DOM 依赖，只使用标准库。

## 3. 控制客户端与界面

- [x] 3.1 `apps/desktop/src/control.ts`：新增 `parseApprovals`，对 `ResourceView` 与 `ResourceItem` 做严格校验（含 `U64` 字符串、`items` 上限 1024）。
- [x] 3.2 同文件：`ControlClient.approvals()` 读 `/approvals`；`ControlClient.decideApproval()` 构造真实 `ApprovalCommand`（`candidate_hash` 取候选哈希、`hash_algorithm: sha256-canonical-v1`、`seq` 单调、epoch/revision 取自真实视图、TTL 有界）并校验 ACK。
- [x] 3.3 `apps/desktop/src/main.tsx`：待审批面板（候选哈希、证据数、有效期、状态、200 上限提示）、批准（二次确认）与拒绝按钮。
- [x] 3.4 动作前复核新鲜度；不满足即**不发请求**并显示具名理由。
- [x] 3.5 更新页脚文案：不再声称「不提供审批按钮」，改为如实说明审批可用的部分与「最终文本不在载荷内」的缺口。

## 4. UI 层与可执行检查

- [x] 4.1 新建 `tests/ui/`（§30.1 指定目录）并落断言：审批失效（过期/换代/已不在队列 ⇒ 不发请求）。
- [x] 4.2 落断言：批准单击不提交、二次点击才提交；批准命令字段精确（candidate_hash/hash_algorithm/seq/epoch/revision/TTL）。
- [x] 4.3 落断言：静音/接管命令 ACK 消息明确为「非硬件 ACK」，不得写成已静音。
- [x] 4.4 落断言：声明的断言集合恰为 §30.1 四条；未实现两条被登记为未实现并点名缺失前置。
- [x] 4.5 根 `package.json` 增加 `test:ui` 并跑通；`pnpm --filter @nai/desktop test` 与 typecheck 不回归。

## 5. 文档、registry 与闭环

- [x] 5.1 `docs/control.md`：新增审批面一节（可用的部分、失效拒绝、文本缺口与其验收项）。
- [x] 5.2 新增 `docs/ui.md`：UI 层位置与四条断言的实现状态、运行方式、边界（不测渲染）。
- [x] 5.3 `tests/registry.yaml`：O02 notes 追加本轮界面侧证据与**未满足项**（「审批按钮显示最终文本」因载荷无文本字段），状态**保持 `pending`**；O05 不动。
- [x] 5.4 `docs/development-plan.md` 与 `AGENTS.md` 记录本轮。
- [x] 5.5 顺序纪律：`check --secrets` → `check --registry` → `check-deps` → 前端 typecheck/build/desktop 测试 → `test:ui` → portable 全量门控 → 勾 tasks → `openspec archive -y` → `validate --all --strict --archived` 全通过。
- [x] 5.6 一次 commit + push；commit 前确认 `.env` 未被跟踪、无凭据入库。

## 里程碑切分

- **M1（§1–§3）**：规格、设计和控制台实现到位，前端 typecheck/build 通过。
- **M2（§4–§5）**：`tests/ui` 落地并被 `test:ui` 执行，文档与 registry 更新，门控与归档闭环。
