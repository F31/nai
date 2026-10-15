# W111 设计：让「清除副本」成为动作，而不是字段

## 1. 缝在哪（现测，不是推断）

| 事实 | 取证 |
| --- | --- |
| `purge_state` / `claims_fully_erased` **全部来自请求体** | `crates/nai-control-api/src/lib.rs:786-819` |
| 运行时只把这两个值**转手**给 `ForgetOutcome` | `crates/nai-runtime/src/memory.rs:230-237` |
| 判定只查**陈述内部自洽**（claims ⇒ `Complete`） | `crates/nai-kernel/src/evaluate/memory.rs:846-869` |
| 零清除动作 | 全仓无任何写 `memory_records.content` 的删除/置空语句 |
| 副本集合**空集** | 读取面直扫表；无记忆索引/缓存/远端同步；控制面与 runtime 零 `decision_log`/`audit_records` 写入 |

判定读的是**调用方的说法**，不是系统做过什么 ⇒ 「删除 MUST 可验证」与「MUST NOT 宣称已完全擦除」
两条同时落空，且落空方式是**静默的**（返回 200）。

## 2. 九条决策

1. **结论不进请求体。** `purge_state` / `claims_fully_erased` 是**结论**，结论只能由执行得出；
   留在请求体里等于让调用方填空题。删除二者（**破坏性**，理由是纪律：同一件事 MUST NOT 两处定义）。
2. **事实留在请求体。** `third_party_retention` 是调用方**独有**的事实（本仓不接触第三方留存），
   删除它会把真实信息赶进「要么谎称没有、要么不告知」⇒ 保留。
3. **顺序在入口内完成。** 阻断与清载荷在**同一次**调用里先后发生。分两次请求会留下
   「已阻断未清除」的中间态，而那个中间态没有任何东西看守。
4. **行留作 tombstone。** §24.4「删除先发布 tombstone」⇒ 清的是 `content`，不是行。
   行的存在让「可验证」有对象（还能证明这条记录被遗忘过）。
5. **只置一次、不覆盖。** `payload_purged_at_ms` 与 W110 的 `revoked_at_ms` 同形：
   重复清除返回**同一个**时刻，否则「什么时候清的」会被后来的一次改写。
6. **副本清单是闭世界。** `COPY_SITES` 在册为空集，但**空集必须是被守卫的事实**：
   ① 每个在册点 MUST 有清除体（逐字存在）；② 记忆载荷落点由**代码派生**（扫存储层写记忆的
   SQL 目标表）与在册集合比对——新增落点不入册即红；③ 清点为空 MUST 报 `0` 且得出 `Complete`，
   **MUST NOT** 当成「跳过」（W92 的 `continue` 陷阱：没实例 ≠ 不再被扫）。
7. **擦除口径写进形状。** 响应里的 `erased` 恒为 `{ scope: "logical", physical_bytes_erased: false }`。
   置空一列**不是**介质级擦除（SQLite 的页与 WAL 可能仍留有字节）⇒ 本仓**永远**不宣称介质级擦除。
   「不宣称」由**类型**保证，而不是靠调用方自觉。
8. **判定输入是事实。** `ForgetOutcome` 增 `body_purged: bool`，由「载荷真的被清过」得出，
   **不是**参数——否则又是一个恒真构造（与 `read_side_blocked` 同一纪律）。
9. **窄化而非新增端点。** 遗忘端点仍是 `POST /api/v1/memory/forget`，权限仍是 `memory.forget`；
   新增的是**它做的事**（清除），不是第二条路由。

## 3. 明确不做

- 不做远端留存清除（`RemoteRetention` 那套契约属 §24.4 知识链，不与记忆记录共用落点）。
- 不做介质级擦除（`VACUUM`/`secure_delete` 会改变全库文件布局，超出本轮边界，且**不构成**承诺）。
- 不改 `admit_write` / `read_records` 的任何判定规则。
- 不改 §42.5（跨会话边界）与桌面 UI。

## 4. 已知代价

- 迁移 0009 ⇒ **版本号成套五处**（W109 在第五处踩过）。
- `MemoryForgetRequest` 收窄是**破坏性**变更：W110 的请求体若带 `purge_state`，改后按**未知字段**处理
  （MUST NOT 静默忽略一个自称已擦除的字段）。