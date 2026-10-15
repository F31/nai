# ADR 0090：§42.5 跨会话边界在 memory 面的判定面闭合

- 状态：已采纳
- 日期：2026-10-15
- 取代：无（**收窄** ADR 0089 交付的清除侧形状，补 §42.5 在 memory 面的跨场守卫判据）
- 关联：ADR 0087（读取面）、ADR 0088（控制面接缝）、ADR 0089（清除侧）、W112 报告

## 一、问题（现测，不是推断）

W107/W109/W110/W111 四轮把 §42.2 写入门、§42.3 读取面、§42.4 遗忘链、控制面接缝、清除侧
逐一接上真实存储与调用方，但 §42.5 跨会话边界（V3.11 §42.5）在 memory 面**始终无判据**：
只靠设计意图断言「授权、审批、租约、设备身份与自动权限不跨场继承」，没有任何可测守卫把它
变成事实。回归即可悄悄改坏——例如把 `scope_matches` 改成「就近匹配」、或 memory 路由漏掉
`self.access` 校验、或将来误加「会话结束即清」逻辑。

现测取证：`scope_matches`（`nai-kernel/src/evaluate/memory.rs:619-628`）按
`record.session_id == scope.session_id`（非观众）与 `viewer_scoped_id`（观众）隔离；`MemoryQuery`
的 `scope.session_id` 来自控制面绑定、请求同名字段被忽略（`nai-control-api/src/lib.rs:708-712`）；
memory 路由鉴权走 `self.access`（`Access` 绑定 `session_id` + `device`）；`memory_records` 按
`expires_at_ms` 保留、无 session-end 触发器。行为都在，但无判据钉死。

## 二、裁定

### 决策 1：判定面闭合，不引入新执行体

§42.5 在 memory 面的「不跨场」由**既有**两机制保证：(a) runtime `scope_matches`；(b) ControlApi
`self.access`。本轮只把两者变成**显式、可测**判据（补测试），不新增生产代码。

### 决策 2：作用域是事实，不是请求参数

`scope.session_id` 来自控制面绑定，请求伪造同名字段被忽略。沿用 W109/W111 纪律，补测试钉死。

### 决策 3：场次记忆跨会话保留是存储层事实

`memory_records` 按 `expires_at_ms` 保留，无 session-end 清除。补判据钉死「会话结束不清空
场次记忆」，作为回归保护。

### 决策 4：观众 `authorization_ref` 当场绑定如实披露为遗留

`read_records`（line 652）只查 `authorization_ref` 非空，不验证它属于当前 session。验证需授权
系统配合或改冻结契约（`memory.access@1` frozen），超界——本轮不解决，如实披露。

### 决策 5：跨设备 token 拒与跨 session token 拒同一条 `self.access` 绑定

`Access` 同时绑定 `session_id` + `device`，任一不符 ⇒ 401。3.1 测试用「另一个控制面签发的 token」
触发该绑定失败，已覆盖 device 维；无独立执行分支，故不单列测试。

## 三、判据落点

- runtime（新增 2）：跨 `viewer_scoped_id` 查观众记录 ⇒ `OutOfScope`（2.2）；场次记忆跨会话保留
  （旧场物理保留、对新场 `OutOfScope`、不删行，2.3）。
- runtime（归属既有 1）：跨 session 查非观众记录 ⇒ `OutOfScope`（2.1，`the_read_back_does_not_
  cross_scopes` 同序列的 `a_record_outside_the_scope_is_refused_by_name_and_not_filtered_away`）。
- control-api（新增 1）：跨场 token 访问本场控制面 memory 路由 ⇒ 401（3.1）。
- control-api（归属既有 1）：作用域来自控制面绑定、请求伪造被忽略（3.3）。

## 四、明确不做

- 不改 `admit_write` / `read_records` / `forget` / `scope_matches` 的判定规则（只补判据）。
- 不改 `authorization_ref` 的契约来源（破坏冻结契约 `memory.access@1`）。
- 不建「跨会话运营」能力（§14.1 Pro 档位）——独立大能力，不在本轮。
- 不碰审批 / 租约 / 设备身份 / 自动权限的独立生命周期（§21.4 / §8.5 / §9.2 通用规则）。
- 不做远端留存清除（§24.4，沿用 W111）。

## 五、已知代价

- 本轮**零生产代码改动**（判定面闭合，补测试）；门控前发现并修复了自身误插属性导致两条既有
  测试被静默取消标记的问题。
- registry `H03` / `H04` 的 `status` 仍 `pending`（判定面闭合 ≠ 验收达标）。

见 [W112 报告](../reports/2026-10-15-w112-memory-cross-session-boundary.md)、
[kernel.md](../kernel.md)、[runtime.md](../runtime.md)。
