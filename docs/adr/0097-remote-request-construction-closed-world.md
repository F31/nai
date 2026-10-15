# ADR 0097：远端出站请求构造闭世界守卫（凭证头 + endpoint 规范化）

- 状态：Accepted
- 日期：2026-10-16
- 归属：W119（从「回包解析」转向**出站请求构造**的判定面缺口勘定，延续 W113–W118 「先测后设计」判定面闭合节奏）
- 关联：ADR 0096（§parse_reply 的 RemoteStatus 映射）、ADR 0095（§top_k 截断）、ADR 0094（§RemoteError::status()）

## 背景
W118 闭合了 `parse_reply` 的 body status 映射。把视线移到**出站请求构造**：
`RemoteKnowledgeClient::request()`（`remote.rs:100-112`）在每次 POST 前组装 URL 与请求头，其两条不变量
**整段零判据**——既有测试经 `Mock::client()` 恒传 `api_key: None`、且 endpoint 形如 `http://127.0.0.1:{port}`
（无尾斜杠）：

- **凭证头（安全相关）**：`api_key = Some(secret)` ⇒ 每个出站请求 MUST 带 `Authorization: Bearer <key>`
  （`remote.rs:105-110`）。**从未被任何测试观测**。若该头被静默丢弃，请求会以「未认证」身份发出而无人察觉
  （远端可能以匿名/降级身份应答，或全部 401 而定位不到根因）。反向闭世界同样零判据：`api_key = None` ⇒
  MUST 不带 `Authorization` 头。
- **endpoint 规范化**：`endpoint.trim_end_matches('/')`（`remote.rs:103`）⇒ 路径 `/search` 而非 `//search`。
  既有 endpoint 均无尾斜杠 ⇒ 该分支从未执行。

## 决策
**判定面闭合，零生产代码改动**。只补可测判据，把出站请求构造的不变量钉成显式闭世界守卫：

1. **用「观测型 Mock」而非「响应型断言」**：既有 Mock 只按 path 回包、从不检查请求内容 ⇒ **出站侧事实**
   （凭证是否真的发出去）无法被测到。引入 `ObservedRequest { url, authorization }` + 记录型 Mock，在处理器里
   读 `request.headers()` / `request.url()` 并存档，测试再断言——把出站侧变成可观测面。
2. **凭证头断言精确值**：只断言「有 `Authorization` 头」不够（格式退化成裸 token 或前缀丢失仍会通过）⇒
   断言 `Some("Bearer test-api-key")` 逐字相等。
3. **闭世界的另一半同样要钉**：`api_key = None` ⇒ `authorization == None`。只测「有凭证时发头」会漏掉
   「无凭证时凭空造头」方向；双向合起来才是闭世界。
4. **endpoint 尾斜杠取单/多两种**：`trim_end_matches('/')` trim **全部**尾随斜杠 ⇒ 单 `/` 与 `///` 同测，
   钉住「不是只 trim 一个」。

## 后果
- 正向：凭证发送从「不可观测」变为「双向闭世界断言」，是安全相关事实；endpoint 规范化分支首次被执行。
- 零运行时变化、零契约字段改动、零迁移、零 schema 改动。
- 如实披露（非验收达标）：
  - 本轮只钉**出站侧事实**（头被发出 / 不被发出），**不断言远端真的接受了凭证**（无真实远端）。
  - endpoint 尾斜杠两条钉的是「最终拼出的路径为 `/search`」这一可观测事实；若将来 reqwest/url 层自身就会
    折叠 `//`，则即使 `trim_end_matches` 被移除这两条也仍会绿。故其判别力弱于凭证头两条，定位是
    **回归哨兵 + 语义记录**。
  - 不断言「凭证不泄露到日志/盘」（属独立面，本轮不做）。
  - registry：守卫不属 memory 子系统（H03/H04），无既有 H 条目需改。
