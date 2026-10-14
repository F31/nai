# w99：站点图层声明（`tier`）必须由宿主解析出的区域兑付

## Why

`scripts/contracts/test-reachability.mjs` 的每个登记站点都声明了一个 `tier`（∈ `portable` / `full` / `ci`），
意思是「这个测试在哪一层被跑」。但**没有任何判据**回答「**谁兑付这个声明**」：

```
TEST_REGISTRATION_SITES[i].tier   ← 唯一的证据是……它自己写了 tier
siteShapeErrors()                 ← 只做 TEST_TIERS.includes(site.tier)，形状检查
```

⇒ 一个站点把 `tier` 从 `portable` 改成 `full`（或反过来）**不会改变任何东西**：
覆盖集不变、闭世界不变、门控全绿。这正是母题「**声明 MUST 付代价**」的第八次出现
（`repositoryFiles` → `readTrackedText` → W94 事实表 → W95 `gateSources` → W96 门控闭包区域
→ W97 包集合 → W98 npm 脚本执行面 → **本轮：站点的层级声明**）。

实测（探针 P-A，改真文件、真跑 7 个 portable 套件、备份拷回、`sha256` 逐字还原）：

| 探针 | 改动 | 结果 |
| --- | --- | --- |
| P-A | 把 `tests/contracts/npm-scripts.test.mjs` 的注册从 `engineering_checks` **移进** `["check","--profile","full"]` arm，站点仍声明 `tier: 'portable'` | **192 passed / 0 failed —— 零信号** |

## What Changes

1. **新增 `scripts/contracts/tier-payment.mjs`**：`tierErrors({sites, libText, driverText, workflowText, packages})`，
   四条判据：
   - **T1 规则闭世界**：`TIER_PAYMENTS` 的键集 MUST 等于 `TEST_TIERS`（新增一个 tier 取值而不归类 ⇒ 红），
     且每个 tier 取值 MUST 至少有一个站点（恒真的规则是「没有断言」的伪装）。
   - **T2 `portable` 由 portable 区域兑付**：host 是 `xtask/src/lib.rs` 时 marker MUST 在
     **剥离注释后**的 `engineering_checks` 区域里逐字存在；否则该 host 的模块 MUST 在该区域里被调用
     （`<module>::<fn>(root`）。**区域经 `stripRustComments`**——实测该区域原文含
     `tests/contracts/descriptor.test.mjs`，但它**只在注释里**（`stripped=false`）。
   - **T3 `full` 由驱动表兑付**：host MUST 解析成包目录，该目录 MUST 是驱动表某个 STEP 的 `dir`，
     且**该 STEP 的 `script` 的脚本体** MUST 含 marker。
   - **T4 `ci` 由具名 CI step 兑付**：marker MUST 逐字出现在某个**具名** step 的 `run` 文本里
     （剥整行 `#` 注释）——把 marker 写进注释不算「有 CI step 跑它」。
   - **T5 反向（对称方向）**：`full`/`ci` 站点的 marker MUST NOT 落在 portable 区域里
     ——否则该声明**少报了本机门控真正跑到的东西**，「逃离 portable 面」就成了一条合法路径。
2. **新增 `tests/contracts/tier-payment.test.mjs`**：承载判据（合成世界 + 注释陷阱 + 真实仓库），
   并登记进 `xtask/src/lib.rs::engineering_checks` 与 `test-reachability.mjs` 的站点表。
3. **规格**：`openspec/specs/engineering-foundation/spec.md` 新增 1 条 Requirement（ADDED）。

## Impact

- **零代码改动**：被审对象 `xtask/src/lib.rs`（除加一行登记）、`scripts/frontend-gate.mjs`、
  `.github/workflows/foundation.yml`、三份 `package.json`、`TEST_REGISTRATION_SITES` 的**既有 11 条**
  一个字符都不动。零新依赖。
- Node 套件由 **9 个 → 10 个**；站点由 **11 → 12**。
- **明确不做**：不验「测试真的能跑通」（那要执行）；不判 CI 里的 `if:` 条件；
  不判 `frontend::check` 内部的真实执行（只判驱动表这一层接线）。
- **如实披露**：`portable` 的拒否靠**剥离注释后的文字域**，不是「marker 是否为一次真实调用」
  ⇒ 若将来把某个 `ci`/`full` 站点的 marker 放进 `engineering_checks` 调用脚本的**字符串字面量**里，
  本轮的 T2/T5 会放行。这是**有意的宽松**，记入报告 §九。
