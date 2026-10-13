# w98：npm 脚本的执行面必须被守卫——每个脚本「谁调用它」必须被算出来，孤儿必须具名付代价

## Why

仓库里有 **3 份 `package.json`、13 个脚本**（根 7、`packages/contracts-ts` 2、`apps/desktop` 4）。每个脚本都声称做了某件事，但**没有任何判据**回答「**谁调用它**」：

```
检视点（当前全是人的记忆，不在任何判据里）
  root:spec:validate      ← .github/workflows/foundation.yml 里那一行（W95 只把它当 step 的文本）
  root:test:ui            ← scripts/frontend-gate.mjs 的 STEPS 表（`.` → `test:ui`）
  apps/desktop:test       ← 同上
  root:test:control-smoke ← 只在 docs/control.md 的代码块里出现（是「说明」，不是「调用」）
  root:check:frontend     ← W41 报告说它是「便于人工单独调用」的别名 —— 一句散文
  apps/desktop:dev        ← 没有人调用，也没有任何地方说它为什么孤着
```

实测（三个 Mutation，改真文件、真跑现有 6 个 portable 套件、备份拷回、`sha256` 逐字还原）：

| Mutation | 改动 | 当前工艺的结果 |
| --- | --- | --- |
| M2 | 往根 `package.json` 加一个脚本 `"typecheck:fast": "node scripts/nowhere.mjs"` | **158 passed / 0 failed —— 零信号**（一个新的孤儿，无人回答） |
| M3 | 把 `frontend-gate.mjs` 的 `STEPS` 里 `['apps/desktop', 'test']` 改成不存在的 `'tests'` | **158 passed / 0 failed —— 零信号**（静态全绿；只有真跑前端门控才会以 `FRONTEND_SCRIPT_UNSUPPORTED` 红） |
| M5 | 给脚本加自引用 `"test:loop": "pnpm test:loop"` | **158 passed / 0 failed —— 零信号**（`pnpm` 引用扫描若不排除自身，这会**自我满足**） |

**一个反例，如实记录**：M1「把 CI 里的 `pnpm spec:validate` 拼错成 `spec:validat`」**会红 2 条**——
W95 的 `localEvidence: {file: '.github/workflows/foundation.yml', marker: 'pnpm spec:validate'}` 逐字比对抓到了。
但它抓的是「**那一行文本变了**」，不是「**那个脚本没有消费者了**」——同一处文本、两种语义。
⇒ 本轮的判据不与它重叠，见 §边界。

同一母题的**第七次**出现（`repositoryFiles` → `readTrackedText` → W94 两张事实表 → W95 `gateSources`
→ W96 门控闭包区域 → W97 包集合 → 本轮：**脚本的执行面**）。每一轮换一层对象，这一轮的对象是
**npm 脚本名**，而它天然带一个 W91 的陷阱：`typecheck` 在三个包里同名，**判定单元 MUST 是
`(包目录, 脚本名)` 的配对，MUST NOT 是裸脚本名**。

## What Changes

1. **新增 `scripts/contracts/npm-scripts.mjs`**：判定单元是 `(包目录, 脚本名)`，六条判据：
   - **V1 双向闭世界**：`package.json` 里出现的每个 `(dir, script)` 与独立声明的 `SCRIPT_DISPOSITIONS`
     **双向**一致（新增脚本、删除脚本两侧都被点名）。
   - **V2 引用可解析**：扫描面里的每一条 `pnpm X` / `pnpm run X` / `pnpm --filter P X` /
     `pnpm --recursive X` MUST 落到**存在**的脚本上，`P` MUST 是真包名；**落在被消费对象自己的
     定义里不算**（自引用 MUST NOT 充当消费者）。
   - **V3 `invoked` MUST 付代价**：每次真调用 MUST 给出 `{file, marker}`，且 marker 在**剥离注释后**
     逐字存在（W97 教训：注释不是引用）。
   - **V4 `orphan` MUST 有成因**：成因落在闭集 `ORPHAN_CAUSES` 里，且**逐格付自己的代价**
     （`manual-smoke`／`manual-alias`／`superseded-by-gate`／`dev-server` 各自引出一处真的存在的证据）。
   - **V5 驱动表接线**：`frontend-gate.mjs` 的 `STEPS` 每项 `(dir, script)` MUST 落在该包 scripts 里，
     且**第一个 token MUST 在门控认得的可执行名集合内**、MUST NOT 含门控不实现的 shell 语法
     ——把 `frontend-gate.mjs` 的**运行时**具名拒绝**静态化**（M3 的守卫）。
   - **V6 成因闭集非空**：闭集 MUST 至少被用到一格（空闭集是「没有断言」的伪装）。
2. **新增 `tests/contracts/npm-scripts.test.mjs`**：承载判据，并登记进
   `xtask/src/lib.rs::engineering_checks` 与 `test-reachability.mjs` 的站点表。
3. **规格**：`openspec/specs/engineering-foundation/spec.md` 新增 1 条 Requirement（ADDED）。

## Impact

- **零代码改动**：`package.json`、`frontend-gate.mjs`、workflow 与被审脚本一个字符都不动；
  `xtask/src/lib.rs` 只加一行登记。零新依赖。
- Node 套件由 **8 个 → 9 个**。
- **文档不算调用**：`.md` 里的 `pnpm test:control-smoke` 是给**人**的指引，MUST NOT 被当成消费者；
  它只作为 `orphan` 成因的证据形态（`docEvidence`）。这一个分离就是本轮的判据核心。
- **明确不做**：不验「脚本真的能跑通」（那要执行）；不验「`pnpm` 引用是不是真的执行路径」
  （CI 里的 `if:` 条件不判）；不给 `test --suite upgrade` 式的未知事实贴标签（W97 遗留，仍未动）。
- **如实披露**：`--filter P` 形态在当前仓库里**实例数为 0**（`pnpm --filter @nai/desktop test`
  只出现在 `docs/control.md`，文档不在扫描面）⇒ V2 的这条分支当前**没有活样本**，只有测试文件里的
  注入用例作证据。
