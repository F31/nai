# Design: W119 — 远端出站请求构造闭世界守卫

## 缝（现测，不是推断）
基于代码实测（非摘要，`crates/nai-knowledge-core/src/remote.rs:100-112`）：

```rust
fn request(&self, path: &str, body: &Value) -> reqwest::RequestBuilder {
    let mut request = self
        .http
        .post(format!("{}{}", self.endpoint.trim_end_matches('/'), path))
        .json(body);
    if let Some(secret) = &self.api_key {
        request = request.header(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", String::from_utf8_lossy(secret.as_bytes())),
        );
    }
    request
}
```

| 不变量 | 位置 | 判据缺口 |
|---|---|---|
| `api_key = Some` ⇒ `Authorization: Bearer <key>` | `remote.rs:105-110` | `Mock::client()` 恒传 `None` ⇒ **零判据** |
| `api_key = None` ⇒ 不带 `Authorization` 头 | 同上（if let 的 else 分支） | 同上 ⇒ **零判据**（闭世界的另一半） |
| endpoint 尾斜杠被 trim ⇒ 路径 `/search` | `remote.rs:103` | 既有 endpoint 均无尾斜杠 ⇒ **分支从未执行** |

## 决策
1. **判定面闭合，零生产代码改动**：只补测试，不引入新执行体、不改 `remote.rs`。
2. **用「观测型 Mock」而非「响应型断言」**：既有 Mock 只按 path 回包、从不检查请求内容，因此「凭证是否真的
   发出去」这种**出站侧**事实无法被测到。本轮引入 `ObservedRequest { url, authorization }` + 记录型 Mock，
   在处理器里读取 `request.headers()` / `request.url()` 并存下来，测试再断言——把出站侧变成可观测面。
3. **凭证头要断言精确值**：只断言「有 `Authorization` 头」不够（若格式退化成裸 token 或前缀丢失仍会通过），
   故断言 `Some("Bearer test-api-key")` 逐字相等。
4. **闭世界的另一半同样要钉**：`api_key = None` ⇒ `authorization == None`。只测「有凭证时发头」会漏掉
   「无凭证时凭空造头」这一方向；两个方向合起来才是闭世界。
5. **endpoint 尾斜杠取单/多两种**：`trim_end_matches('/')` 会 trim **全部**尾随斜杠，故单 `/` 与 `///`
   同测，钉住「不是只 trim 一个」这一语义。
6. **确定性离线**：Mock 为本地 `127.0.0.1:0`；`Secret::new`（`nai_security`，上限 2560 字节）构造测试用凭证；
   本地复跑 `running 36 tests ... 36 passed; 0 failed`。

## 明确不做
- 不改 `remote.rs` 任何判定规则（不改头名/格式、不新增 endpoint 校验）。
- 不断言「远端真的接受了凭证」（无真实远端；本轮只钉**出站侧事实**：头被发出/不被发出）。
- 不引入真实网络、不引入回放录制（全本地 Mock）。
- 不碰介质级擦除、不建跨 memory↔remote 传播（与 W113–W118 一致）。

## 如实披露（判据的判别力边界）
- endpoint 尾斜杠两条测试**钉住的是「最终拼出的路径为 `/search`」**这一可观测事实；若将来 reqwest/url 层
  自身就会折叠 `//`，则即使 `trim_end_matches` 被移除这两条也仍会绿（判据的判别力依赖下层是否折叠）。
  故这两条的定位是**回归哨兵 + 语义记录**，其判别力弱于凭证头两条（后者是纯粹的闭世界双向断言）。
