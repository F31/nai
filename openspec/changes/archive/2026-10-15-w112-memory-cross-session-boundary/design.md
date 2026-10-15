# W112 设计：§42.5 跨会话边界在 memory 面的判定面闭合

## 1. 缝在哪（现测，不是推断）

| 事实 | 取证 |
| --- | --- |
| `scope.session_id` 来自控制面绑定，请求同名字段被忽略 | `crates/nai-control-api/src/lib.rs:708-712` + 注释 642-644 |
| 跨场读取靠 `scope_matches` 的 session / viewer_scoped_id 匹配 | `crates/nai-kernel/src/evaluate/memory.rs:619-628` |
| memory 路由鉴权走 `self.access`（绑定 session + device） | `crates/nai-control-api/src/lib.rs:393` + `memory_read` 649 |
| 记忆记录按 `expires_at_ms` 保留，无「会话结束即清」逻辑 | `crates/nai-storage` 的 `memory_records` 表无 session-end 触发器 |
| §42.5 在 memory 面无显式判据 | ADR 0087/0088/0089、kernel.md:123、runtime.md:305 均记「未覆盖」 |

「不跨场」目前是**设计意图**，不是**可测守卫**。回归即可悄悄改坏（例如把 `scope_matches` 改成「就近匹配」、或 memory 路由漏掉 access 校验）。

## 2. 决策

1. **判定面闭合，不引入新执行体。** §42.5 在 memory 面的「不跨场」由**既有**两个机制保证：
   (a) runtime `scope_matches` 的 session / viewer_scoped_id 隔离；
   (b) ControlApi `self.access` 的场绑定鉴权。
   本轮只把这两个机制变成**显式、可测**的判据（补测试），不新增生产代码。
2. **作用域是事实，不是请求参数。** 沿用 W109/W111 的纪律：`scope.session_id` 来自控制面绑定，请求伪造同名字段被忽略。补测试钉死这条。
3. **场次记忆跨会话保留是存储层事实。** `memory_records` 按 `expires_at_ms` 保留，无 session-end 清除。补判据钉死「会话结束不清空场次记忆」（回归保护，防止将来误加清除）。
4. **观众 `authorization_ref` 的当场绑定如实披露为遗留。** `read_records`（line 652）只查 `authorization_ref` 非空，不验证它属于当前 session。验证需授权系统配合或改冻结契约（`memory.access@1` frozen），超界——本轮**不解决**，如实披露（与 W111 同纪律）。

## 3. 明确不做

- 不改 `admit_write` / `read_records` / `forget` / `scope_matches` 的任何判定规则（只补判据，不改逻辑——除非测试暴露缺口）。
- 不改 `authorization_ref` 的契约来源（请求参数 → 控制面派生会破坏冻结契约）。
- 不建「跨会话运营」能力（§14.1 Pro 档位）——那是个独立的大能力，不在本轮。
- 不碰审批 / 租约 / 设备身份 / 自动权限的**独立生命周期**（§21.4 / §8.5 / §9.2 通用规则，非 memory 模块独占）。
- 不做远端留存清除（§24.4 知识链，沿用 W111）。

## 4. 已知代价

- 本轮**零生产代码改动**（判定面闭合，补测试）；若测试暴露既有行为缺口，则按缺口补最小守卫，不扩面。
- registry `H03` / `H04` 的 `status` 仍 `pending`（本轮闭合的是判定面，不是验收达标——沿用 W107-W111 的纪律）。
