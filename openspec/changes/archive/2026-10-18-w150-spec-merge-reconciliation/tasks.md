# Tasks: W150 — 归档增量与主规格的对账

## 1. 勘定（先量，再判）

- [x] 量 W149 残余第 1 条（归档 `tasks.md` / `specs/` 结构）：146 / 146 都齐备 ⇒ 这一半**零信号**，
      照抄立项会写出一条今天必然全绿、且没有真实失效模式的判据
- [x] 转向同族真缺口：delta ⇔ 主规格的对账（178 份 delta、43 个能力、539 / 497 条 Requirement）
- [x] 量段标记闭世界：`ADDED` 164 / `MODIFIED` 34 / `REMOVED` 0，非标记标题只有 `Purpose`（53）
- [x] 量两方向：delta 的 539 条全在主规格（缺 0）；主规格 497 条也全有出处（无出处 0）
- [x] 量 `MODIFIED` 先存性：42 条里 **2** 条改的是各自 delta 之前就存在的 Requirement
      ⇒ 判它会把两条合法历史判红，**不判**
- [x] 证实现有门控零信号：手工改掉 `local-autonomy` 一条 Requirement 名后，
      `validate --all --strict` 仍 **43 / 0**、`validate --archived` 仍 **146 / 0**
      （探针备份 + `sha256sum -c` 逐字节还原 `0538cdf9…f438`）
- [x] 量围栏：主规格含围栏 0 份、delta 含围栏 0 份 ⇒ 标题行不会被围栏藏住，不需要围栏解析

## 2. 判据

- [x] `scripts/contracts/spec-merge.mjs`：`DELTA_MARKS` / `DELTA_SECTION_TITLES`（段标记闭世界）
- [x] `deltaRefsOf`：域 = `<change>/specs/<capability>/spec.md`，排序即**重放顺序**（日期前缀）
- [x] `mainSpecRefsOf`：域 = `specs/<capability>/spec.md`（嵌套与非 spec.md 不进域）
- [x] `deltaSections` / `mainRequirements`：段归属唯一出处；段外 Requirement 具名失败；
      空 delta 用「全文件 `### Requirement:` 行数」判，不用「已归属条数」判（后者会说假话）
- [x] `specMergeErrors`：顺序重放 + 两方向具名 + `REMOVED` 专门措辞 + 能力整体缺席只报一条 +
      无法重放的能力**跳过比对**（否则同一条缺口报两遍，且第二遍是假结论）

## 3. 判别性样本（5 条）

- [x] 真实语料基线：零错 + 非空洞五项（178 份 delta 全重放、146 个变更全覆盖、43 个能力全比对、
      重放总量非零、且「重放总量」与「主规格实际条数」两个独立算出的数字相等）
- [x] 重放顺序断言：变更名列表按字典序（= 时间序）
- [x] 合成样本：集合并集（加了又删）/ `MODIFIED` 先存 / 两方向各一条 / `REMOVED` 另一方向 /
      段标记闭世界 / 段外 Requirement / 空 delta / 缺 delta 文件 / 能力整体缺席 / 主规格无段 /
      主规格段外 Requirement / 空集三形态
- [x] 域本身：只有 4 段路径算 delta；未跟踪与 active change 不进域；嵌套主规格不算

## 4. 注入验证（逐次 sha256 还原，基线 `c8109b2c…b4cd`）

- [x] W1 顺序重放改回集合并集（`REMOVED` 分支失效）⇒ **2 红**（`#2` `#3`）
- [x] W2 去掉 `delta ⇒ 主规格` 方向 ⇒ **2 红**（`#3` `#4`）
- [x] W3 去掉 `主规格 ⇒ delta` 方向 ⇒ **2 红**（`#3` `#4`）
- [x] W4 放宽段标记闭世界（不认识的标题静默接受）⇒ **只红 `#4`**
- [x] W5 `REMOVED` 走通用措辞 ⇒ **只红 `#3`**（`doesNotMatch` 那一句）

## 5. 接线与收口

- [x] `xtask/src/lib.rs` 的 `engineering_checks` 增加一步（注释写明它补的是哪条零信号）
- [x] `TEST_REGISTRATION_SITES` 增加 `xtask-spec-merge`（`tier: portable`）
- [x] `tests/registry.test.mjs` 逐字清单「十八个」→「十九个」并加 id
- [x] ADR 0124；轮次报告；`docs/development-plan.md` W150 段；`docs/engineering.md` 一段
- [x] 门控 `cargo xtask check --profile full`；两条 validate（含 `--archived`）在推送前绿
- [x] 一次 commit + push；CI 双平台观测；回填 §六/§七
