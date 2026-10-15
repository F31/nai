# W113 报告：§24.4 远端留存清除 —— `RemoteGateway::delete` 传输契约判定面闭合

日期：2026-10-15｜归属：W111 §7.2 遗留清单唯一尚存真实候选｜零生产代码改动

## 一、裁定与缝（现测，不是推断）

W111 报告 §7.2 把「远端留存清除（§24.4 知识链那套 `RemoteRetention`，与记忆记录不共用落点）」
列为 memory 子系统边界工作的**唯一尚存真实候选**。§24.4 的删除不变量已在
`crates/nai-knowledge-core/src/remote.rs:658-659` 以注释钉死：「Delete must follow a local
tombstone；the receipt is remote acknowledgment, **never proof that third-party copies were
erased**」。但实测该不变量**零判据**：

| 位置 | 现状 | 判据缺口 |
|---|---|---|
| `remote.rs:669` `if !self.policy.require_remote_delete` | 已门控，`false` ⇒ `Unauthorized` | 既有测试已钉（归属） |
| `remote.rs:670-672` delete 输入边界（空集 / >16 / 含空 id） | 已门控返回 `Unauthorized`，**无单测** | **新增** |
| `remote.rs:658-659` 注释「receipt ≠ 第三方擦除证明」 | 行为成立（`delete` 透传 `RemoteReceipt`），**无判据**防契约新增 `erased` 变体 | **新增** 枚举闭世界守卫 |
| `remote.rs:682` 透传 | 回执原样返回 | 新增透传断言 |

**裁定**：判定面闭合，零生产代码改动——只把既有的两条 §24.4 机制变成显式守卫。

## 二、判据落点（`crates/nai-knowledge-core/tests/remote.rs`）

- **新增 1**：`remote_delete_rejects_malformed_document_ids` —— 空集 / >16 / 含空 id ⇒
  `RemoteError::Unauthorized` 且不触网（钉死 remote.rs:670-672 输入边界闭世界）。
- **新增 2**：`remote_delete_receipt_is_passed_through_and_never_claims_erasure` —— 远端返回
  `Deleted` 回执时，网关原样透传（`request_id` / `policy_ref` / `status`），并对 `RemoteReceiptStatus`
  穷尽 match 四态（accepted / pending / deleted / retained）；**若契约新增 `erased` / `purged`
  之类变体，match 编译失败**，强制就 §24.4 不变量评审一次（把注释 658-659 变成可测守卫）。
- **归属 1**：`sync_and_delete_are_gated_single_attempt_and_return_receipts`（既有）已钉
  `require_remote_delete == false` ⇒ 拒、`allow_third_party_retention == false` ⇒ sync 拒、
  门控调用不触网；W113 显式归属，不重复造。

## 三、计数（现测，非推断）

`crates/nai-knowledge-core/tests/remote.rs`：`#[tokio::test]` **10 → 12**（+2 = 新增 1 + 2；
归属 1 为既有）。门控权威总数见 §六。

## 四、如实披露（判定面闭合 ≠ 验收达标）

- `max_retention_seconds`（`RemotePolicy` 字段）声明但**未被 `gate` 强制**（remote.rs:538-569
  只查 `expires_at_utc_ms` 等，不查保留期上限）；`RemoteRetention.retention_seconds` 仅是远端
  能力声明元数据，本地无保留期执行机制。W113 不建该机制（属「远端留存保留期执行」能力，非传输
  契约判定面），如实记为遗留。
- `tombstone → 远端删除` 的本地前置由存储层 `Store::remote_delete_targets` 负责
  （`docs/knowledge.md:65`），`RemoteGateway::delete` 是传输层、假定 tombstone 已存在——W113
  不重复存储层判定。
- 介质级擦除仍明确不做（W111 §7.2 第 2 项）。
- registry：§24.4 远端留存不在 H03/H04（记忆）登记范围，无既有 H 条目需改。

## 五、spec 增量（`knowledge-pipeline`）

新增 Requirement **Remote delete contract enforces input bounds and never claims third-party
erasure** + 2 Scenario（Malformed delete targets are rejected without network / Remote receipt is
passed through and never claims erasure），把 §24.4 delete 契约纳入基线。

## 六、门控（全量 `cargo xtask check --profile full`）

- `GATE_EXIT=0`（日志 `target-check/w113-gate-final.log`）。
- 三步全绿：`cargo fmt --all --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、整 workspace 测试套件（`--profile full` 含前端门控，前端 `全部步骤通过`）。
- 实跑确认：W113 两项新测试均 `ok`（`remote_delete_rejects_malformed_document_ids`、`remote_delete_receipt_is_passed_through_and_never_claims_erasure`）；整 workspace 各测试 bin `fail 0`（24 / 34 / 22 / 15 / 16 / 15 / 5 / 52 等）。
- **门控瞬态与处置（如实）**：本机初跑命中间歇 `LNK1104`（Defender 实时扫描持锁刚写出的测试 exe，os error 5），且 `~/.cargo/bin/cargo.exe` 此前后被 Defender 在 rustup 操作中改名成 `*.zero-byte-backup`（零字节，代理损坏）。二者**均非 W113 代码回归**（W113 仅改测试文件 `remote.rs`）。处置：直接用 toolchain 内真实 `cargo.exe`（`~/.rustup/toolchains/1.96.0-x86_64-pc-windows-msvc/bin`）绕过损坏代理 + `CARGO_BUILD_JOBS=1` + 预构建重试循环（每次 LNK1104 删除被锁 stale exe 后复跑），最终 `PREBUILD_OK at attempt 1` 且 `GATE_OK`。环境脚本 `target-check/msvc-env.sh` / `gate-w113.sh` 不纳入提交。

## 七、提交 / 推送

- 主提交 `803924f35f062fa033ee4db41299aff57faeb363`（`W113：§24.4 远端留存清除 delete 传输契约判定面闭合`），10 文件 +315/−2：`remote.rs` 测试 +2、ADR 0091、W113 报告、development-plan W113 段、knowledge.md §24.4 订正、spec 合入 `openspec/specs/knowledge-pipeline/spec.md`、归档目录。
- 推送 `bcca316..803924f` → `main`（SSH 443，`git@github.com:F31/nai.git`）。

## 八、CI 观测

- run `38695867812`，`head_sha` 完整匹配 `803924f35f062fa033ee4db41299aff57faeb363`，`conclusion: success`。
- 双平台 job 均 `success`：`foundation (ubuntu-24.04)`、`foundation (windows-latest)`。
- 观测时间 2026-10-15 ~07:12 UTC；查询带 `Cache-Control: no-cache` + 缓存破坏参数，`head_sha` 与推送 SHA 一致，非代理缓存过期响应。
- 本机门控绿 + CI 双平台绿 ⇒ W113 闭环达成；判定面闭合（零生产改动），**不**声称验收达标（§24.4 保留期执行 / 介质级擦除仍如实记为遗留，见 §四）。
