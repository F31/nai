# W119 报告：远端出站请求构造闭世界守卫（凭证头 + endpoint 规范化）

日期：2026-10-16｜归属：W118 之后「新开 nai 守卫缺口」勘定候选（从「回包解析」转向**出站请求构造**的判定面）｜零生产代码改动

## 一、裁定与缝（现测，不是推断）

W118 闭合了 `parse_reply` 的 body status 映射。把视线从「回包解析」移到**出站请求构造**：
`RemoteKnowledgeClient::request()`（`remote.rs:100-112`）在每次 POST 前组装 URL 与请求头，其两条不变量
**整段零判据**——既有测试经 `Mock::client()` 恒传 `api_key: None`、且 endpoint 形如 `http://127.0.0.1:{port}`
（无尾斜杠）：

| 不变量 | 位置 | 判据缺口 |
|---|---|---|
| `api_key = Some` ⇒ `Authorization: Bearer <key>` | `remote.rs:105-110` | `Mock::client()` 恒传 `None` ⇒ **零判据** |
| `api_key = None` ⇒ 不带 `Authorization` 头 | 同上（if let 的 else 分支） | 同上 ⇒ **零判据**（闭世界的另一半） |
| endpoint 尾斜杠被 trim ⇒ 路径 `/search` | `remote.rs:103` | 既有 endpoint 均无尾斜杠 ⇒ **分支从未执行** |

**裁定**：判定面闭合，零生产代码改动——只把上述不变量变成显式守卫，与 W113–W118 同形。

## 二、判据落点（`crates/nai-knowledge-core/tests/remote.rs`）

**关键方法差异**：既有 Mock 是「响应型」（只按 path 回包、从不检查请求内容），因此**出站侧事实**（凭证是否真
的发出去）在既有测试里根本不可观测。本轮引入 `ObservedRequest { url, authorization }` + **记录型 Mock**：
处理器读取 `request.headers()` / `request.url()` 存档，测试再断言——把出站侧变成可观测面。新增辅助
`observe_outgoing_request(endpoint, api_key)`：

- **新增 1**：`remote_request_sends_bearer_authorization_header_when_api_key_present` —— 断言
  `authorization == Some("Bearer test-api-key")` **逐字相等**（只断言「有头」不够：格式退化成裸 token 或
  前缀丢失仍会通过）。
- **新增 2**：`remote_request_omits_authorization_header_when_api_key_absent` —— 断言
  `authorization == None`。闭世界的另一半：只测「有凭证时发头」会漏掉「无凭证时凭空造头」方向。
- **新增 3**：`remote_request_normalizes_trailing_slash_in_endpoint` —— endpoint 以单个 `/` 结尾 ⇒
  `url == "/search"`，且该分支此前从未被执行。
- **新增 4**：`remote_request_normalizes_repeated_trailing_slashes_in_endpoint` —— endpoint 以 `///` 结尾 ⇒
  `url == "/search"`，钉住 `trim_end_matches('/')` 是 trim **全部**尾随斜杠（不是只 trim 一个）。

## 三、计数（现测，非推断）

`crates/nai-knowledge-core/tests/remote.rs`：测试 **32 → 36**（+4 = 全为新增，W119 四条均未曾被任何测试覆盖；
W113–W118 的归属项已在各自报告标为既有）。本地复跑：`running 36 tests ... test result: ok. 36 passed; 0 failed`
（含全部 4 条 W119 新增）。门控权威总数见 §六。

## 四、如实披露（判定面闭合 ≠ 验收达标）

- 本轮只钉**出站侧事实**（头被发出 / 不被发出），**不断言远端真的接受了凭证**（无真实远端，全本地 Mock）。
- endpoint 尾斜杠两条钉的是「最终拼出的路径为 `/search`」这一可观测事实；若将来 reqwest/url 层自身就会折叠
  `//`，则即使 `trim_end_matches` 被移除这两条也仍会绿 ⇒ **其判别力弱于凭证头两条**，定位是「回归哨兵 +
  语义记录」，不冒充为强判别判据。
- 不断言「凭证不泄露到日志/盘」（属独立面，本轮不做）。
- 介质级擦除、跨 memory↔remote 传播仍明确不做（与 W113–W118 一致）。
- registry：守卫不属 memory 子系统（H03/H04），无既有 H 条目需改。

## 五、spec 增量（`knowledge-pipeline`）

新增 4 条 Requirement（各 1 Scenario），把出站请求构造闭世界纳入基线：
- **Remote request sends Bearer authorization header when API key present**（API key is sent as an exact Bearer header）；
- **Remote request omits authorization header when no API key**（No API key means no Authorization header）；
- **Remote request normalizes trailing slash in endpoint**（Single trailing slash in endpoint is trimmed）；
- **Remote request normalizes repeated trailing slashes in endpoint**（Repeated trailing slashes in endpoint are trimmed）。

## 六、门控（全量 `cargo xtask check --profile full`）

- `GATE_EXIT`：**0**（后台 `target-check/gate-w119.sh`，`PREBUILD_OK at attempt 1` + `GATE_OK at attempt 1`，修复后首跑即绿）。`rustc 1.96.0 (ac68faa20 2026-05-25)`。
- **首次门控 `GATE_EXIT=1`（真实根因 = clippy `useless_asref`，非 LNK1104、非测试失败）**：观测型 Mock 里
  `header.value.as_str().as_ref()` 被 `clippy::useless_asref` 拦下——`HeaderField::as_str()` 返回 `&AsciiStr`
  （需 `as_ref()` 转 `&str`），而 `Header.value`（`AsciiString`）的 `as_str()` **本身返回 `&str`**，多一次
  `as_ref()` 即触发 lint。去掉该处 `as_ref()` 后通过。
  - **门控脚本改进生效**：改进的 `gate-w119.sh` 把失败**正确归类为 `GATE_FAILED (non-LNK1104)`** 并停止重试，
    没有重演 W117 的「累积日志把真实失败误标为 LNK1104、空转到 8 次上限」——本次处置时间从「空转 8 次」
    降为「一次即定位」。
- **LNK1104 瞬态计数**：**0**（grep `LNK1104` = 0；`PREBUILD_OK attempt 1`）。仅偶发增量编译锁删除告警
  `拒绝访问。 (os error 5)` 按既定纪律认定为非回归、不修代码。
- 全 workspace `cargo xtask check --profile full`：**95** 个测试二进制全 `test result: ok. ... 0 failed`，
  **0 FAILED / 0 error**；clippy `--workspace --all-targets --locked -- -D warnings` 通过（修复后）；
  `cargo fmt --all --check` 通过；前端门控（`[frontend] 全部步骤通过`）通过。权威测试 target 中
  `crates/nai-knowledge-core/tests/remote.rs` 运行 `running 36 tests ... 36 passed; 0 failed`（含全部 4 条 W119
  新增，见 §三）。

## 七、提交 / 推送（归档 + 一次 commit + push，SSH 443）

- 提交 SHA：**`9e6589594c271d69a0cb8e990d0107a50d8ef1e0`**（主提交）。
- push 区间：**`8b86394..9e65895`**，分支 `main`，SSH 443（`ssh.github.com:443` + `StrictHostKeyChecking=accept-new`）。
- 改动文件：`crates/nai-knowledge-core/tests/remote.rs`（+4 测试，含新增 `Secret` 导入）、`docs/adr/0097-remote-request-construction-closed-world.md`（新建）、`docs/reports/2026-10-16-w119-remote-request-construction-closed-world.md`（新建）、`docs/development-plan.md`（W119 段）、`docs/knowledge.md`（订正注记）、`openspec/specs/knowledge-pipeline/spec.md`（合入 4 条 Requirement）、`openspec/changes/archive/2026-10-16-w119-remote-request-construction-closed-world/`（归档目录）。共 10 文件、+404 行。
- openspec 三连：`validate w119-remote-request-construction-closed-world --strict` → valid；`archive --yes` → 4 条 Requirement 合入基线（`+ 4, ~ 0, - 0`）；`validate --archived --strict` → 全绿（**117 项，含 w119**，0 failed）。归档目录按**系统日期戳**记为 `2026-10-16-...`。`target-check/` 按纪律排除暂存（显式 `git add -- <path>` 逐个暂存）。

## 八、CI 观测（主提交 + 回填提交各一次双平台观测）

- **主提交** run id：**38768579361**（workflow `Engineering foundation`，event `push`）。回读 `head_sha` = `9e6589594c271d69a0cb8e990d0107a50d8ef1e0`，与推送 SHA **完整 40 位一致**；双平台 `foundation` job 均 `completed / success`：
  - `foundation (windows-latest)` — completed / **success**
  - `foundation (ubuntu-24.04)` — completed / **success**
  - 本机门控绿（`GATE_EXIT=0`，见 §六）与 CI 双平台绿一致；判定面闭合经端到端 CI 复核。
- **回填提交**（docs-only）自身 CI：按纪律做 **1 次**双平台观测（先例 run 109 failure ⇒ 不观测就是赌）。**本报告不为回填提交写 §九**；其自身的观测结论按「**回填提交自身不回填**」记入 memory 日志，**不在本报告占位**——避免为了填一个 run id 再叠加提交、形成回填链（W116–W118 曾这样叠加，本轮起改用时即记录于 memory 的写法）。
