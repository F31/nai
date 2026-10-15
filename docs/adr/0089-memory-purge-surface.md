# ADR 0089：清除副本从「字段」变成「动作」

- 状态：已采纳
- 日期：2026-10-15
- 取代：无（**收窄** ADR 0088 交付的遗忘端点形状）
- 关联：ADR 0087（读取面）、ADR 0088（控制面接缝）、W110 报告

## 一、问题（现测，不是推断）

W110 交付的 `POST /api/v1/memory/forget` 接受请求体里的 `purge_state` 与
`claims_fully_erased`，并把两者原样交给内核判定。W111 立项时先写了一条判据并**先跑红**：

```
test claiming_fully_erased_must_leave_no_payload_in_the_store ... FAILED
宣称已完全擦除 ⇒ 载荷 MUST NOT 仍在库里；实测仍读到
  "the record body that MUST be persisted verbatim, not a pointer"
```

即：调用方自称「副本已清 + 已完全擦除」，系统返回 200，而 `memory_records.content`
一字未动。§42.4 的两条要求同时落空——

- 「删除 MUST 可验证：清理侧给出完成状态」：状态是**调用方给的**；
- 「MUST NOT 宣称已完全擦除」：系统接受了一个**零执行支撑**的宣称。

更糟的是交付面自相矛盾：W110 报告如实写着「副本清除未执行」，而端点却允许宣称已清完。

## 二、裁定

### 决策 1：结论不进请求体（**破坏性**）

`purge_state` 与 `claims_fully_erased` 从请求体**删除**，理由不是「更安全」而是纪律：
**同一件事 MUST NOT 两处定义**。结论由执行得出，请求体只承载事实。

留在请求体里的同名字段按**未知字段**被拒（`400`），**MUST NOT 静默忽略**——
静默忽略会让调用方以为自己的宣称生效了，那等于把缺陷换了个位置。

### 决策 2：事实留在请求体

`third_party_retention` **保留**：它是**事实**（第三方留存只有调用方知道，本仓不接触任何
第三方存储）。删除它会把真实信息赶进「要么谎称没有、要么不告知」。

### 决策 3：`body_purged` 是从**存储读出**的事实

`ForgetOutcome` 增 `body_purged: bool`，由 `Store::memory_body_purged_at` 派生，
**不是**入参。写死 `true` 会让新拒因 `ErasedClaimWithoutPurge` 变成不可达的死分支。

### 决策 4：判定的检查顺序是**可达性**决定的

`forget()` 对「宣称已完全擦除」的检查顺序：

1. `purge_state != Complete` ⇒ `ErasedClaimWithoutCompletePurge`（陈述内部不自洽）；
2. `!body_purged` ⇒ `ErasedClaimWithoutPurge`（陈述与**执行事实**冲突）；
3. 第三方留存未披露 ⇒ `ErasedClaimWithThirdPartyRetention`。

W111 实测：把 2 放在 1 之前会让第 1 条**不可达**（不声称 complete 时根本走不到它），
用例随即红。⇒ **顺序不是风格，是「每条拒因都有构造路径」的一部分**。

### 决策 5：擦除口径写进**形状**

响应含 `erased: { scope: "logical", payload_cleared: <bool>, physical_bytes_erased: false }`。
本仓**恒不**宣称介质级擦除：置空一列不是介质级擦除（SQLite 的页与 WAL 可能仍留有旧字节）。
把「不宣称」交给类型与构造，而不是靠调用方自觉。

### 决策 6：副本清单是**闭世界**，当前为空集

`COPY_SITES` 在册为空，且这是**被守卫的事实**：读取面直扫表（无记忆索引）、runtime 与
控制面**零**缓存层、**零**远端同步、**零** `decision_log`/`audit_records` 写入。
判据扫**代码派生**的落点（存储层写语句的目标表），新增记忆落点而不登记即红。

空集 MUST 被**清点并报 `0`**，MUST NOT 短路成「跳过」（W92 的 `continue` 陷阱）。

### 决策 7：顺序在**一次调用**内完成

`purge_memory` 先 `block_memory_read` 再 `purge_memory_body`。分两次请求会留下
「已阻断未清除」的中间态，而那个中间态没有任何东西看守。

### 决策 8：行留作 tombstone

§24.4「删除先发布 tombstone」⇒ 清 `content`，**不删行**。行还在，
「这条记录被遗忘过」才有凭据。

## 三、代价

- 迁移 `0009-memory-purge.sql` ⇒ 版本号成套**五处**（W109 在第五处踩过）。
- `MemoryForgetRequest` 收窄是**破坏性**变更；W110 规格里「Forget blocks the read side
  first」那条 Requirement 已按 MODIFIED 修订。
- W109 遗留的 `VERSION as usize - 1` 写法在本轮暴露：升到 9 之后那条用例**悄悄改测了
  v8→v9**，用例名仍写着 version_eight ⇒ 已钉死为 `const MEMORY_REVOCATION: usize = 8`。

## 四、明确不做

- 不做远端留存清除（`RemoteRetention` 属 §24.4 知识链，与记忆记录不共用落点）。
- 不做介质级擦除（`VACUUM`/`secure_delete` 改变全库文件布局，超出边界且**不构成**承诺）。
- 不改 `admit_write` / `read_records` 的判定规则，不改 §42.5，不做桌面 UI。