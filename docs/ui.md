# UI 层（`tests/ui`）

设计 §30.1 为 UI 层指定了 `tests/ui` 与四条断言。本轮（W38）首次建立该目录，并把**能脱离渲染被断言**的部分落成可执行检查。
它测的是**界面行为逻辑**，不是像素与浏览器渲染。

## 断言清单与实现状态

| 断言 | 状态 | 落点 | 说明 |
|---|---|---|---|
| 审批失效 | **implemented** | `approvalFreshness` / `runApprovalDecision` | 过期、会话换代、已不在队列、无有效期 ⇒ **不发请求**并具名理由 |
| 模式切换条件 | **unimplemented** | 缺 `POST /sessions/{id}/mode` 实现 | 契约已定义（`schemas/openapi/control.yaml:66`），控制面未实现，界面无从表达切换条件 |
| 静音 ACK | **implemented** | `describeCommandResult` / `HARDWARE_ACK_NOTE` | 命令结果消息必须写明**控制 ACK 非硬件 ACK**，不得写成「已静音」 |
| 升级与回滚提示 | **unimplemented** | 缺 `nai-upgrade-core` 的状态来源 | 控制面未暴露 plan/journal，界面无从提示升级与回滚 |

未实现的两条**登记为未实现并点名缺失前置**，不伪造「已覆盖」。这条纪律本身有测试守着：`an unimplemented assertion names the prerequisite it lacks, an implemented one claims none`。

## 位置与分层

- 断言：`tests/ui/*.test.mjs`（Node 22 + `--experimental-strip-types` 直接加载 `.ts`）。
- 被测逻辑：`apps/desktop/src/console.ts`（**零 DOM 依赖**，只用标准库）与 `apps/desktop/src/control.ts`。
- 运行：`pnpm test:ui`（根 `package.json`），即 `node --experimental-strip-types --test tests/ui/*.test.mjs`。

为什么把逻辑抽到 `console.ts` 而不是写进 React 组件：§30.1 要求这四条**可被机器检查**，而 React 组件需要 DOM 才能驱动。
抽出的部分是**判据与编排**（新鲜度、两步确认、消息文案、断言声明），组件只把结果贴回界面——于是「失效时不发请求」成为一条**可断言的性质**。

`.ts` 显式扩展名：Node 的 ESM 解析**不做扩展名补全**，相对导入必须写 `./control.ts`；`apps/desktop/tsconfig.json` 因此开启 `allowImportingTsExtensions`。

## 本轮落下的 15 项（`tests/ui`）

`approval-surface.test.mjs`（11 项）：

1. 过期候选被拒且**什么都没发**；
2. 没有有效期的候选按**不可用**处理，不按有效处理；
3. 会话在面板列出后换代的动作被拒；
4. 已不在最新队列的候选被拒；
5. 首次点击只**置位**，第二次点击才提交同一候选；
6. 拒绝是安全方向，**不需要**二次确认；
7. 新鲜度判据报**第一条**适用理由；
8. 批准命令**逐字**携带候选哈希与会话上下文（`candidate_hash` / `hash_algorithm` / `seq` / `generation_epoch` / `expected_state_revision` / `ttl_ms`）；
9. **假 ACK 不会变成已批准**，且**不重试**；
10. 审批载荷被**严格解析**或拒绝；
11. 命令 ACK 被报告为**控制 ACK**，绝不报告为硬件静音。

`ui-layer-declaration.test.mjs`（4 项）：

1. 声明的标签**恰为**设计列的四条；
2. 每条断言都有确定状态，且**不发明** id；
3. 未实现的断言**点名前置**，已实现的不声称任何前置；
4. 两个界面尚不存在的断言被登记为未实现并给出锚点。

## 边界

- **不测渲染**：本轮不引入 DOM 库（jsdom/playwright 等），**零新增依赖**。像素、布局、可访问性、真实浏览器行为均不在本层。
- **不宣称 UI 验收达标**：`tests/ui` 通过只是**逻辑面**成立；设计 §662（O02）/ §425（T-14）的「审批按钮显示最终文本」**当前未满足**——载荷里没有候选文本（见下）。
- **`local_control` 示例未装配审批 backend**：故手工点开界面时审批请求仍显式返回 `CAPABILITY_NOT_READY`；`tests/ui` 用替身驱动，证明的是**编排与判据**。

## 文本缺口（诚实披露）

设计 §662 与 §425 要求审批按钮**显示最终文本**。本栈做不到，原因是存储只保存哈希：
`migrations/sqlite/0001-foundation.sql:10` 的 `utterances` 表只存 `text_hash`，审批列表接口也只返回哈希/元信息引用/期限/证据计数。

界面的做法是**如实说出来**（`FINAL_TEXT_GAP` 文案：本载荷不含最终文本、身份为候选哈希、并指向未满足的验收项），
而不是把哈希当文本展示、或含糊地叫「内容」。补齐它需要存储侧保存可展示文本并同步契约——**不在本轮范围**，已登记在 `tests/registry.yaml` 的 O02 notes。

## 可执行验证

```bash
pnpm test:ui
pnpm --filter @nai/desktop test
pnpm typecheck
```

相关：[控制面指南](control.md)、[本轮报告](reports/2026-10-10-w38-desktop-approval-surface.md)。
