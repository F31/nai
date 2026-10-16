# W124 报告：有界存储维护的零覆盖上限补齐（R08 补覆盖）

日期：2026-10-16｜归属：R1「本仓可闭合」项 R08｜**仅一处可见性改动，无行为改动**

## 一、缝（现测）
| 上限 | 位置 | 实测判据缺口 |
|---|---|---|
| `MAX_PAYLOAD_BYTES` = 65536 | `decision.rs:11` | `grep src/tests.rs` = **0** |
| mutable journal 规则（拒 `-wal/-shm/-journal`） | `snapshot.rs:293` | **0** |
| `MAX_BYTES`（256MiB 备份 / 16MiB 发布） | `snapshot.rs` / `publication.rs` | **0** |

对照：遥测缓冲 `decision_buffer_stats` 有 **9** 处引用 ⇒ **不是整项未测**，而是「部分覆盖」的具体缺口。

## 二、判据（真实证据，55 → 57）
- `the_decision_payload_is_bounded_and_the_refusal_is_named`：`cap/2` 载荷通过；序列化后 `cap+overhead`
  的载荷 ⇒ 具名 `RESOURCE_EXHAUSTED`（**断言了夹具真的越界**，且拒绝必须具名 ⇒ 静默截断也会红）。
- `a_snapshot_carrying_a_mutable_journal_is_rejected`：`-wal` / `-shm` / `-journal` **各注入一次** ⇒
  `CONFLICT`；每个注入之间**删除 sidecar 后同一快照必须能恢复**（对照 ⇒ 拒绝可归因于 sidecar，
  而非坏备份造成的假通过）。
- 为使判据**引用**上限而非复制字面量，`MAX_PAYLOAD_BYTES` 改为 `pub(crate)`（W77 同一条规矩）。
- 本机：`cargo test -p nai-storage --lib` ⇒ **57 passed; 0 failed**。

## 三、registry 判定（**维持 `implemented`**）
`snapshot::MAX_BYTES`(256MiB)、`publication::{MAX_CHUNKS,MAX_BYTES}`、`knowledge::MAX_CONTENT_BYTES`
仍**零覆盖** ⇒ 覆盖仍属部分，不升 `tested`（第三度使用「守卫通过 ≠ 本项达标」）。

## 四、门控（全量 `cargo xtask check --profile full`）
- `GATE_EXIT`：**0**（`PREBUILD_OK at attempt 6` + `GATE_OK at attempt 1`，`rustc 1.96.0 (ac68faa20 2026-05-25)`）。
  **LNK1104 = 10**（全部是预构建阶段的重试，按纪律删被锁 exe 消化，不改代码；真实测试失败 **0**）。
- **95** 个测试二进制全 `0 failed`；门控内实测 `nai-storage` **57 passed; 0 failed**（含 W124 两条新增）；
  clippy `-D warnings` / `fmt --all --check` / 前端门控全部通过。
- 备注：本地跑该 crate 时撞到一次 `LNK1104`（Defender 锁 `nai_storage-*.exe`），删被锁 exe 后即绿，
  按纪律不改代码。

## 五、提交 / 推送
- 提交 SHA：TBD。｜push 区间：TBD，分支 `main`，SSH 443。
- openspec 三连：valid → `archive --yes`（`+2`）→ `--archived --strict` **122 / 0**。

## 六、CI 观测
- **主提交** run id：TBD；`head_sha` 完整 40 位一致；双平台 `foundation` 结论 TBD。
- **回填提交**（docs-only）自身 CI：观测 1 次，结论记入 memory，不在本报告占位。
