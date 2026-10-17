# UI 层（`tests/ui`）

设计 §30.1 为 UI 层指定了 `tests/ui` 与四条断言。本轮（W38）首次建立该目录，并把**能脱离渲染被断言**的部分落成可执行检查。
它测的是**界面行为逻辑**，不是像素与浏览器渲染。

## 断言清单与实现状态

| 断言 | 状态 | 落点 | 说明 |
|---|---|---|---|
| 审批失效 | **implemented** | `approvalFreshness` / `runApprovalDecision` | 过期、会话换代、已不在队列、无有效期 ⇒ **不发请求**并具名理由 |
| 模式切换条件 | **unimplemented** | 缺 `POST /sessions/{id}/mode` 实现 | 契约已定义（`schemas/openapi/control.yaml:66`），控制面未实现，界面无从表达切换条件 |
| 静音 ACK | **implemented** | `describeCommandResult` / `HARDWARE_ACK_NOTE` | 命令结果消息必须写明**控制 ACK 非硬件 ACK**，不得写成「已静音」 |
| 升级与回滚提示 | **unimplemented** | 缺真实渲染的升级面板（UI 层无 DOM）；控制面**没有执行/回滚的写入方** | **W138 已补上状态读者**：`GET /jobs/{id}` + `ControlClient.fetchJob` + `jobStatusNote` 能把「停在第几步、是不是终态」说清楚。但那一行**只能来自 journal**，而控制面的 execute/rollback 具名拒绝（缺平台安装器 `UpgradeHost`），故「能否回滚」仍不可呈现——**能读出步骤不等于能改变步骤** |

未实现的两条**登记为未实现并点名缺失前置**，不伪造「已覆盖」。这条纪律本身有测试守着：`an unimplemented assertion names the prerequisite it lacks, an implemented one claims none`。

> **别把 O05 与第 4 条混淆**：§30.1 第 4 条是「升级与回滚**提示**」（需要控制面暴露 `plan`/`journal` 状态来源），
> 与之相关但**不同**的是 O05「升级**成本**显示」。后者的呈现层已在 W49 落地、来源在 W137 落地（见下）；
> 第 4 条的两个来源**都已落地**（plan 在 W137、journal 读数在 W138），它现在缺的不是「读不到」，
> 而是**没有写入方与渲染面**：execute/rollback 具名拒绝（缺平台安装器），`apps/desktop` 也没有真实渲染的升级面板。
> 「状态读得出来」与「升级与回滚提示已实现」是两件事，MUST NOT 因为前者升级后者。

## 升级成本显示（O05 的库内部分，W49）

`apps/desktop/src/upgrade.ts` 把「成本逐项**准确列示**」落成可断言的呈现口径，与 `console.ts` 同形（零 DOM）：

| 性质 | 落点 | 断言 |
|---|---|---|
| 未知分项呈现为**原因**，绝不呈现为 0 | `costRows` | `未知分项呈现为原因，绝不呈现为 0` |
| 显式的 0 是一个断言，被如实呈现为 0 | `costRows` | `显式的 0 是一个断言，被如实呈现为 0` |
| 全已知 ⇒ 合计 | `totalPresentation` | `全已知时总额是合计` |
| 含未知 ⇒ **下界 + 未知计数**，且**不是**总额 | `totalPresentation` | `含未知分项时总额是下界加未知计数，不是总额` |
| 缺字段/未知字段/非法金额/空原因 ⇒ **具名拒绝** | `parseCostEstimate` | 十条拒绝用例（缺分项、多分项、负金额、小数、字符串金额、空原因、两者并存、非法货币码、非对象…） |

**W137 补上来源**：控制面 `POST /upgrades/plan`（权限 `runtime.upgrade`）收下**包清单**、
交给 `nai-upgrade-core` 的 `CostEstimate`（成本算法的**定义处**，控制面与前端都不另算）
并把四分量搬出来；桌面端 `parseUpgradePlan` 只判信封，成本一律交给 `parseCostEstimate`。
这条链上三层各有判据：契约（响应必须满足 `UpgradePlanView` 自己声明的 schema）、
控制面（权限/清单不合规/币种/金额上界/体上限）、桌面（信封多余键、身份为空、成本解析只有一处）。

**它仍不构成 O05 的完整验收**：`tests/ui` 证明的是**呈现口径与信封**，不是渲染；W137 补的是
**成本来源**，`apps/desktop` 里**没有真实渲染的升级面板**（也无 DOM 测试环境），生产接线未做。
⇒ `tests/registry.yaml` 的 **O05 保持 `pending`**，与 O02 同一惯例（界面出现不等于验收达标）。

## 位置与分层

- 断言：`tests/ui/*.test.mjs`（Node 22 + `--experimental-strip-types` 直接加载 `.ts`）。
- 被测逻辑：`apps/desktop/src/console.ts`（**零 DOM 依赖**，只用标准库）、`apps/desktop/src/control.ts`
  与 `apps/desktop/src/upgrade.ts`（成本显示口径，W49）。
- 运行：`pnpm test:ui`（根 `package.json`），即 `node --experimental-strip-types --test tests/ui/*.test.mjs`。

为什么把逻辑抽到 `console.ts` 而不是写进 React 组件：§30.1 要求这四条**可被机器检查**，而 React 组件需要 DOM 才能驱动。
抽出的部分是**判据与编排**（新鲜度、两步确认、消息文案、断言声明），组件只把结果贴回界面——于是「失效时不发请求」成为一条**可断言的性质**。

`.ts` 显式扩展名：Node 的 ESM 解析**不做扩展名补全**，相对导入必须写 `./control.ts`；`apps/desktop/tsconfig.json` 因此开启 `allowImportingTsExtensions`。

## `tests/ui` 的断言（W38 起）

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

`upgrade-cost.test.mjs`（19 项，W49）：见上「升级成本显示」一节——逐项呈现、总额呈现与十条严格解析用例。

`upgrade-plan.test.mjs`（10 项，W137）：信封与来源一侧——只认 `package_id`/`package_version`/`cost`
三个键（多一个也拒）、身份为空拒、成本**只由** `parseCostEstimate` 解析（判别性用例：坏成本必须由那一处拒绝）、
未知分项呈现为原因、部分和不是总额、`COST_DECLARED_BASIS_NOTE` 三点内容锚点（包清单 / 未验签 / 升级事务），
以及客户端把清单交给 `/upgrades/plan` 且请求体**只有** `manifest`。

`upgrade-job.test.mjs`（9 项，W138）：状态一键清——信封只认契约声明的八个键（多一个 `rollback_allowed`
也拒）、六个必填缺一即拒、**未知步骤具名拒绝而不猜**、可选的两列只认缺省不认空串、
`JOB_STEP_LABELS` 的键集与 `JOB_STEPS` 两集相等、状态说明分开说终态与未竟、未知步骤有「不做猜测」的出口、
`ROLLBACK_UNAVAILABLE_NOTE` 三点内容锚点（安装器 / 只读 / 不提供回退），
以及客户端对 `/jobs/{id}` **只发 GET 且不带请求体**、空 id 连请求都不发。

## 边界

- **不测渲染**：本轮不引入 DOM 库（jsdom/playwright 等），**零新增依赖**。像素、布局、可访问性、真实浏览器行为均不在本层。
- **不宣称 UI 验收达标**：`tests/ui` 通过只是**逻辑面**成立；设计 §662（O02）/ §425（T-14）的「审批按钮显示最终文本」**W51 起逻辑面已满足**——载荷返回候选展示文本、界面显示它（见下）。但仍**不**据此宣称 UI 验收达标：真实候选来源、桌面渲染与生产 root 接线未接，故 O02 保持 `pending`。
- **`local_control` 示例未装配审批 backend**：故手工点开界面时审批请求仍显式返回 `CAPABILITY_NOT_READY`；`tests/ui` 用替身驱动，证明的是**编排与判据**。

## 审批显示文本（W51）

设计 §662 与 §425 要求审批按钮**显示最终文本**。W51 起本栈做得到：存储侧迁移 `0006-utterance-display-text.sql`
新增 `utterances.display_text`，`record_candidate` 写入候选自身的 `display_text`；`list_pending_approvals` 与
`read_approval` 返回该列；控制面 `ResourceItem` 新增可选 `display_text`，列表载荷随之携带。
`apps/desktop/src/console.ts` 的 `candidateText()` 把载荷映射为 `{present, text, label}`——**有文本则显示文本**，
命令与批准冒泡仍以**候选哈希**为准（`id`），二次确认文案（`approveConfirmPrompt`）也点名「身份为候选哈希」。

界面对**缺文本**如实具名（`FINAL_TEXT_GAP`：本载荷未随候选提供展示文本，该行早于 `display_text` 字段），
**绝不**把哈希或空串当作将要播报的内容——`label !== ''` 与「`text !== hash`」在 `tests/ui/approval-surface.test.mjs` 被断言。

**真值边界（ADR 0045）**：`display_text` 在候选哈希覆盖范围内（`candidate_hash` 对整个 cue 含 `display_text` 做 SHA-256），
但存储只保留哈希与文本两列、不保留完整 cue ⇒ **无法**由文本重算哈希本地核对，故**不声称「文本已被哈希校验」**；
文本篡改不会让审批绑定到另一候选，但**可能误导操作员**（剩余风险）。存储字段就位**不等于 O02 达标**：真实候选来源、
桌面渲染与生产 root 接线未接 ⇒ **O02 保持 `pending`**，见 `tests/registry.yaml` O02 notes。

## 可执行验证

```bash
pnpm test:ui
pnpm --filter @nai/desktop test
pnpm typecheck
```

## 桌面壳宿主边界（W54）

`tests/ui` 的另一半在**宿主侧**：`apps/desktop/src-tauri/` 的形态声明与最小权限面
（[ADR 0046](adr/0046-desktop-shell-form-and-tauri-boundary.md)／[控制面指南](control.md) 末节）。

`desktop-shell.test.mjs`（15 项）读 `tauri.conf.json`、`capabilities/default.json` 与 `shell-policy.json` 的**形状**，断言：
权限集合与声明**逐一相等**（多授少授都失败；聚合权限集被拒）、CSP 无 `unsafe-eval` 与通配源且 `connect-src` 含 `ipc:`、
sidecar 逻辑名**不带**目标三元组且对应真实二进制包、后台生命周期不绑界面、桌面／macOS／移动端三形态共用边界、
前端源码不 import `@tauri-apps/*`。每条检查都配**非空洞守卫**（注入违反样本必须被抓到）。

**它不是壳体验收**：本目录不引入 `tauri` crate、不建 Rust 壳主体、不打包（ADR 0046 D3）。校验读的是**配置的形状**，
**不能**证明配置被某个 Tauri 版本接受，`tests/registry.yaml` 的 A12 因此保持 `pending`。
同理，`tests/ui` 通过仍**不构成**浏览器渲染或人工 UI 验收。

相关：[控制面指南](control.md)、[本轮报告](reports/2026-10-10-w38-desktop-approval-surface.md)。
