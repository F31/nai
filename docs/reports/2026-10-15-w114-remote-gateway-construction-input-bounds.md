# W114 报告：远端网关构造期与输入边界闭世界守卫

日期：2026-10-15｜归属：W113 之后「新开 nai 守卫缺口」勘定候选（远端网关构造期与输入边界）｜零生产代码改动

## 一、裁定与缝（现测，不是推断）

W113 闭合了 `RemoteGateway::delete` 传输契约判定面。用户经 AskUserQuestion 选定「新开 nai 守卫缺口」，
对 nai 当前守卫/判据体系重新勘定，挑出下一个「先测后设计」的判定面闭合候选。基于代码实测
（非摘要），远端网关的**构造期**与**输入边界**仍有四处「已声明、零判据」不变量：

| 位置 | 现状 | 判据缺口 |
|---|---|---|
| `remote.rs:67-74` `RemoteKnowledgeClient::new` 校验 | 空 endpoint / >2048 / timeout 零或 >600s / max_retries >10 ⇒ `Invalid` | 既有 `Mock::client()` 只传合法参数，**零判据** → **新增** `remote_knowledge_client_new_rejects_invalid_args` |
| `remote.rs:518-526` `RemoteGateway::new` 校验 | `source_id`/`policy_ref`/`permission_revision` 空、`allowed_kb_ids` 空、`unknown_revision_ttl_ms` 0 或 >5000 ⇒ `Invalid` | 既有 `gateway()` 只用合法 policy `.unwrap()`，**零判据** → **新增** `remote_gateway_new_rejects_invalid_policy` |
| `remote.rs:571-581` `capabilities` 首操作守卫 | 首个 `allowed_operations` 非 Search/Answer ⇒ `Unauthorized` | 既有测试首 op 都是 Search，**零判据** → **新增** `remote_capabilities_rejects_when_first_allowed_op_not_search_or_answer` |
| `remote.rs:639-647` `sync` 输入边界 | 文档集空 / >16 / 单文档 `content > 65536` ⇒ `Unauthorized` | 既有 `sync_and_delete_are_gated_*` 只钉门控开关，**零判据** → **新增** `remote_sync_rejects_malformed_documents_without_network` |

**裁定**：判定面闭合，零生产代码改动——只把四处已声明不变量变成显式守卫，与 W113 同形。

## 二、判据落点（`crates/nai-knowledge-core/tests/remote.rs`）

- **新增 1**：`remote_gateway_new_rejects_invalid_policy` —— policy 六类非法变体（source_id /
  policy_ref / permission_revision 空、allowed_kb_ids 空、unknown_revision_ttl_ms 0 或 >5000）⇒
  `RemoteError::Invalid` 且不触网；另断言合法 policy 仍构造成功（守卫不误伤正常路径）。
- **新增 2**：`remote_knowledge_client_new_rejects_invalid_args` —— endpoint 空 / >2048、timeout 零 /
  >600s、max_retries >10 ⇒ `RemoteError::Invalid` 且不建 http client（纯构造、不触网）；另断言合法
  参数仍构造成功。`.err()` 而非 `.unwrap_err()`——两个构造的 Ok 值（`RemoteGateway` /
  `RemoteKnowledgeClient`）未实现 `Debug`，`.unwrap_err()` 会编译失败。
- **新增 3**：`remote_sync_rejects_malformed_documents_without_network` —— 文档集空 / >16 / 单文档
  content>65536 ⇒ `RemoteError::Unauthorized`；用 attempt 计数器显式证明**仅合法单文档调用触网**，
  把「拒绝 ⇒ 不触网」升级为可测断言（而非仅靠返回错误推断）；另断言合法单文档仍成功。
- **新增 4**：`remote_capabilities_rejects_when_first_allowed_op_not_search_or_answer` —— 首个
  allowed op 为 Sync（非 Search/Answer）⇒ `Unauthorized` 且不触网；首 op 为 Answer ⇒ 放行（断言远端
  `supports_delete`，合法正常路径）。
- **归属（不重复造）**：既有 `sync_and_delete_are_gated_single_attempt_and_return_receipts` 已钉
  `allow_document_upload == false` / `allow_third_party_retention == false` ⇒ sync 拒、门控调用不触网；
  W114 显式归属，只补**文档形状**边界。

## 三、计数（现测，非推断）

`crates/nai-knowledge-core/tests/remote.rs`：`#[tokio::test]` **12 → 16**（+4 = 全为新增，W114 四条
均未曾被任何测试覆盖；无既有测试归属为本轮新增，W113 的归属 1 已在 §二标为既有）。门控权威总数见 §六。

## 四、如实披露（判定面闭合 ≠ 验收达标）

- `RemoteGateway::new` **不校验** `allowed_operations`（只查身份字段与 `unknown_revision_ttl_ms`）；
  `capabilities` 守卫依赖调用方把「检索/问答」放在 `allowed_operations` **首位**。这是「首操作」约定
  而非穷举枚举校验——W114 钉住该约定的当前语义，若未来改为「集合含 Search/Answer 任一即可」属新变更，
  不在本轮。
- 介质级擦除仍明确不做（W111 §7.2 第 2 项）。
- registry：远端网关构造/边界守卫不属 memory 子系统（H03/H04），无既有 H 条目需改。

## 五、spec 增量（`knowledge-pipeline`）

新增 4 条 Requirement（各 1 Scenario），把构造期与输入边界闭世界纳入基线：
- **Remote gateway construction rejects invalid policy**（Malformed policy is rejected at gateway construction）；
- **Remote knowledge client construction rejects invalid args**（Malformed client args are rejected at client construction）；
- **Remote sync rejects malformed documents without network**（Malformed sync documents are rejected without network）；
- **Remote capabilities requires a search or answer first operation**（Capabilities requires a search or answer first operation）。

## 六、门控（全量 `cargo xtask check --profile full`）

- `GATE_EXIT=0`：`PREBUILD_OK at attempt 1` → `GATE_OK at attempt 1`（重跑 attempt 1 全绿；首跑因两处真实失败提前结束，非 LNK1104 瞬态）。
- **LNK1104 瞬态：0 次**（grep 计数 0；本轮未触发 Defender 持锁，无需限并发复跑）。
- 全 workspace `cargo xtask check --profile full`：52 tests passed / 0 failed（含 knowledge-core 16 个 `#[tokio::test]`，其中 W114 新增 4 条覆盖远端网关构造期与输入边界）。clippy `-D warnings` 零警告，`cargo fmt --all --check` 全绿。
- 首跑暴露的两处**真实失败**（已在本机修复、零生产改动）：① `cargo fmt --all --check` 在 W114 测试长表达式失败（已 `cargo fmt` 折行）；② `clippy::cmp_owned` —— 测试3 的 `request.url().to_string() == "/sync"` 多余 owned 比较（已改 `request.url() == "/sync"`，`Url: PartialEq<&str>` 成立，无 clippy 风险）。两处均为测试代码，不影响 `src/remote.rs` 任何生产路径。

## 七、提交 / 推送（归档 + 一次 commit + push，SSH 443）

- 提交 SHA：`989791e86d749c64ee747f5d00d08fecebdc7271`。
- push 区间：`f2fbd9e..989791e`，分支 `main`，SSH 443（`ssh.github.com:443` + `StrictHostKeyChecking=accept-new`）。
- 改动文件（10 文件，+514）：`crates/nai-knowledge-core/tests/remote.rs`（+4 测试）、`docs/adr/0092-remote-gateway-construction-input-bounds.md`（新建）、`docs/reports/2026-10-15-w114-remote-gateway-construction-input-bounds.md`（新建）、`docs/development-plan.md`（W114 段）、`docs/knowledge.md`（订正注记）、`openspec/specs/knowledge-pipeline/spec.md`（合入 4 条 Requirement，83–104 行）、`openspec/changes/archive/2026-10-15-w114-remote-gateway-construction-input-bounds/`（归档目录）。
- openspec 三连：`validate w114-remote-gateway-construction-input-bounds --strict` → valid；`archive --yes` → 4 条 Requirement 合入基线；`validate --archived --strict` → 全绿（112 项，含 w114）。`target-check/` 按纪律排除暂存。

## 八、CI 观测（主提交 + 回填提交各一次双平台观测）

- **主提交** run id `38706760713`：`head_sha = 989791e86d749c64ee747f5d00d08fecebdc7271`（与推送 SHA 完整 40 位一致）；双平台 `foundation` job 均 `completed success`：
  - `foundation (ubuntu-24.04)` → success；
  - `foundation (windows-latest)` → success。
- **回填提交** `eff1c9507df2eef6b6836deace4780990c241930`（docs-only，本报告的 §六/§七/§八 回填；memory 日志为本地补充不纳入版本控制）：CI run `38708901699`，`head_sha` 与推送 SHA 完整 40 位一致；双平台 `foundation` job 均 `completed success`（`ubuntu-24.04` + `windows-latest`）—— 回填提交 MUST 观测自身 CI 已兑现（先例 run 109 failure ⇒ 不赌）。W114 全闭环：门控 `GATE_EXIT=0`、主提交 + 回填提交双 CI success、零生产代码改动。
