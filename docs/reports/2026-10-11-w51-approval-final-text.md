# W51 实测报告：审批显示最终文本（O02 的逻辑面收口）

- 日期：2026-10-11
- 变更：`openspec/changes/w51-approval-final-text/`（归档后为 `openspec/changes/archive/2026-10-11-w51-approval-final-text/`）
- 指南：[docs/ui.md](../ui.md)、[docs/control.md](../control.md)、[ADR 0045](../adr/0045-approval-display-text-boundary.md)
- 判据：`tests/registry.yaml` **O02**（**保持 `pending`**）

## 结论

设计 §662（O02）与 §425（T-14）要求「**审批按钮显示最终文本**」。W09 交付审批 API、W38 交付桌面审批面，
但这条例验**一直未满足**：`utterances` 表只保存 `text_hash`（`migrations/sqlite/0001-foundation.sql:10`），
审批列表接口也只返回哈希/元信息引用/期限/证据计数。W38 的做法是界面**如实具名缺口**（`FINAL_TEXT_GAP`），
**不**把哈希当文本展示。

本轮把缺口的**仓库内那一半**补齐——存储侧保存候选的展示文本、控制面载荷携带它、界面显示它——
**并在同一步定死三条口径**：文本是**只读呈现**、**身份始终是候选哈希**、缺文本**具名**而非空串冒充。
**O02 保持 `pending`**：真实候选来源、桌面渲染与生产 root 接线未接，按 `admission-registry` 的诚实回填
**不得**因映射自动升级为 `tested`。

## 一、存储侧：迁移只增不改

新增 `migrations/sqlite/0006-utterance-display-text.sql`：

```sql
ALTER TABLE utterances ADD COLUMN display_text TEXT NOT NULL DEFAULT '';
```

**不得修改 `0001`**：`crates/nai-storage/src/migration.rs::verify()` 按**每个文件**的 SHA-256 校验已应用迁移、
并对比重建库的 DDL 布局；改 `0001` 会让所有既有库的校验和漂移。故 `migration::VERSION` 5 → 6、
`SQL` 数组由 5 项增至 6 项。

`DEFAULT ''` **有意义**：它标记**早于该列的旧行**（迁移引入前入库的候选没有文本）。界面必须把空串
**具名**为缺口，**不得**当「候选文本就是空」。

`crates/nai-storage/src/store.rs` 三处改动：

- `record_candidate` 改为**具名列**写入：`INSERT INTO utterances(utterance_id,workspace_id,session_id,generation_epoch,status,text_hash,expires_at_ms,actual_played,display_text) VALUES(?1,?2,?3,?4,1,?5,?6,0,?7)`，末位 `?7` 为 `cue.display_text`；
- `PendingApproval` 增字段 `pub display_text: String`（注释说明「Empty 表示该行早于 `0006`，不是候选没有文本」）；
- `list_pending_approvals` 与 `read_approval` 的 SELECT 各增 `u.display_text`，映射 `display_text: r.get(5)?`。

`schemas/config/backup.schema.json` 的 `db_schema_version` 由 `{"const":5}` 改为 `{"const":6}`（与 `migration::VERSION` 同步）。

## 二、契约与生成物

`schemas/openapi/control.yaml` 的 `ResourceItem.properties` 增**可选** `display_text`：

```yaml
display_text: {type: string, description: 'Display-only candidate text for operator review; the item identity remains id and the approval binds to the candidate hash; absent when a row predates the field.'}
```

**description 含逗号必须加引号**——否则 YAML 流式映射会把它解析成多余键，触发 `additionalProperties: false` /
`oneOf` 校验失败（本轮真实踩到一次）。**选可选而非必填**：必填会让早于 `0006` 的候选**无法表示**「文本缺失」，
而可选**准确表达**了「可能没有」，把缺失交给界面**具名**。

三份生成物随之重建（**生成代码不得手工修改**）：`crates/nai-contracts/src/generated/control_http.rs`（diff 1 行）、
`packages/contracts-ts/src/generated/http.ts`（+2 行）、`packages/contracts-ts/src/generated/config/backup.ts`（版本号 5 → 6）。

控制面 `crates/nai-control-api/src/lib.rs`：`PendingApprovalView` 增 `pub text: String`，`From<PendingApproval>`
加 `text: item.display_text`，`approvals_list` 的 `json!` 增 `"display_text": item.text`。

## 三、桌面呈现层：显示文本，身份仍是哈希

`apps/desktop/src/console.ts` 新增：

- `candidateText(item)` → `{id, present, text?, label}`：有文本则 `present=true`、`label` 为文本；
  缺文本（非字符串或空串）则 `present=false`、`label` 为 `FINAL_TEXT_GAP`
  （「该候选未随载荷提供展示文本（该行早于文本保存字段，或载荷未含该字段）；身份是候选哈希，请勿据哈希推断将要播报的内容。」）。
- `approveConfirmPrompt(item)`：有文本时二次确认文案为「将批准播报：「…」；身份为候选哈希。再次点击同一候选才提交批准。」；
  缺文本时退回 `APPROVE_CONFIRM_PROMPT`（= `FINAL_TEXT_GAP + 再次点击…`）。

`runApprovalDecision` 的 armed 分支改用 `approveConfirmPrompt(item)`。`apps/desktop/src/control.ts` 的
`parseApprovals` 增严格校验：`display_text` 若存在则必须是字符串，否则拒绝该载荷。

## 四、真值边界：**不声称「文本已被哈希校验」**（ADR 0045）

`candidate_hash(cue)`（`crates/nai-contracts/src/lib.rs:26`）对**整个 cue**（**含 `display_text`**）做 SHA-256，
故文本**在**哈希覆盖范围内。但存储只保留**两列**——`text_hash` 与 `display_text`——**不保留完整 cue**，
因此**无法**由文本重算哈希做本地核对。两条被否决的替代：

- 声称「文本已被哈希校验」⇒ **不实**（重算需要完整 cue，而存储没有）；
- 把 `text_hash` 的语义悄悄改成「只覆盖文本」⇒ **改既有契约定义、令既有行失效**。

故本轮**如实披露无法本地重算**，并把「文本被篡改**不会**让审批绑定到另一候选、但**可能误导操作员**」
记为**剩余风险**。**身份语义不变**：审批对象始终是候选哈希，命令也以哈希为准，界面二次确认文案也**点名**这一点。

## 五、验证

| 检查 | 结果 |
|---|---|
| `cargo xtask check-deps` | `dependency boundaries passed: 18 workspace packages` **EXIT=0** |
| `cargo xtask check --registry` | `admission registry check passed (V3.7 acceptance + D01-D14 mapping)` **EXIT=0** |
| `cargo xtask check --profile portable` | **EXIT=0**；**83 目标（66 `Running` + 17 `Doc-tests`）、726 passed / 0 failed**（W49 为 725，**+1 = 本轮新增存储测试**） |
| `cargo xtask schema`（临时异步 shim） | `catalog checked: 27 capabilities; 112 protobuf messages` / `TS codecs generated for 10 protobuf sources` / `generation completed` |
| 生成脚本还原 | `git diff --stat scripts/contracts/generate.mjs` **exit=0**（已还原，未留改动） |
| `cargo check --workspace --locked` | **EXIT=0** |
| `node --experimental-strip-types --test tests/ui/*.test.mjs` | **37 passed / 0 failed**（W49 后为 34，**+3 = 本轮新增**） |
| 两包 `tsc --noEmit` | **EXIT=0** |
| `@nai/contracts` build | **EXIT=0** |
| `vite build` | **EXIT=0**（`dist/assets/index-9TygVhoX.js 206.90 kB`） |
| `openspec validate w51-approval-final-text --strict` | **valid** |
| `openspec validate --all --strict` | **39 passed / 0 failed**（W50 后为 38，**+1**） |

**门控记法**：数 `     Running <test target>`（形如 `Running unittests src\lib.rs (…)`）**加** `Doc-tests`；
`Running \`…nai-xtask.exe check --profile …\`` 是 cargo 的**包装行、不是测试目标**（本轮再次出现在日志第 2 行，已剔除）。
**`cargo fmt --all --check` 一次真实失败**：本轮新增的数组断言被 rustfmt 要求换行，已修（fmt 通过后才重跑门控）。

**本机环境边界**：本机 Node 的**同步子进程创建被阻断**（`spawnSync`/`execSync` 一律 `EBUSY`，managed 22.22.2 与
system 24.13.0 均如此，`dangerouslyDisableSandbox` 亦然），而**异步 `spawn` 正常**。`scripts/contracts/generate.mjs`
三处用 `spawnSync`（第 23/117/129 行）⇒ 在本机**必然失败**。处理：**临时**把 `spawnSync` 换成等价异步 shim、跑一次生成、
**随后还原脚本**（生成用的是**同一套生成逻辑**，只是把同步等待换成异步）。**不**为一个主机限制永久改仓库代码。

**本机 `LNK1104` 三次复现（主机事实，非代码问题）**：portable 门控在本机**两次**因链接被瞬时占用失败——
第一次在 `cargo fmt` 修好后重跑时命中 `nai_xtask-…exe`（**xtask 预热未覆盖到**），另有 workspace 侧预热
第 1–6 次连败、第 7 次成功。修法：**预热两个目标**（`cargo test --workspace --exclude nai-xtask --no-run` **与**
`cargo test -p nai-xtask --lib --no-run`），一次成功后再跑门控即 `LNK=0`。

**测试目标与用例**：`crates/nai-storage/src/tests/approval_workflow.rs` 的 `list_read_and_reject_close_awaiting_workflow`
增 `display_text` 断言；**新增** `stored_display_text_is_the_admitted_text_and_never_a_hash`（断言列表与单读都拿到
`限量版 买一送一`、`display_text != candidate_hash`、空文本候选读出为空串且 `!= hash`）；
`crates/nai-control-api/src/tests.rs` 的列表测试断言 `items[0]["display_text"]` 为候选文本、`!= items[0]["id"]`，
且列表项**不含** `speech_text`/`candidate_hash`/`performance`/`evidence_refs`（展示文本的引入**不**扩大载荷暴露面）；
`tests/ui/approval-surface.test.mjs` **新增 3 项**（显示候选文本且绝不替换为哈希 / 无存储文本时具名缺失且绝不当空内容 /
形状错误的 `display_text` 被严格解析拒绝）。

## 六、未做与遗留

- **真实候选来源**、**桌面渲染**（未引入 DOM 库 ⇒ 不构成浏览器或人工 UI 验收）、**生产 root 接线**、**手机审核**、
  `local_control` 的审批装配——均**未做**，故 **O02 保持 `pending`**。
- **不**新增 xtask 命令与证据目录（§30.1 未为审批文本指定目录）。
- 语料侧（W50）的 `display_text` **来源**：真实候选入库时由生成侧提供；本轮只保证「存了就用、没存就具名」。

见 [docs/ui.md](../ui.md)、[docs/control.md](../control.md)、[ADR 0045](../adr/0045-approval-display-text-boundary.md)。
