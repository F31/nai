# ADR 0100：`tested` 的证据可核验（验收台账状态回填的第一条硬约束）

- 状态：Accepted
- 日期：2026-10-16
- 归属：W122（首轮「从改 registry 状态入手」的验收推进；同时也是把既有散文声明补上载体的守卫轮）
- 关联：ADR 0099（§Operation 有界预算）、`admission-registry` spec §Honest backfill mapping with traceable evidence、ADR 0015（基线切换与勘误）

## 背景
W121 之前的 9 轮（W113–W121）产出集中在判定面守卫，**`tests/registry.yaml` 自 W112 起未再变动**
（`git log W113..HEAD -- tests/registry.yaml` 为空）⇒ 判据密度上升但**验收状态零推进**。按裁定转入
「从改 registry 状态入手」后，第一步必须回答：**升级 `implemented → tested` 凭什么**。

实测结论：`admission-registry` 早已声明「`tested` 仅可用于**已有归档实测证据**的项目，且证据 MUST 指向
**真实报告**与测试计数」，但 `tests/registry.test.mjs` 中 `grep evidence` 只命中三处 `landing` 的内容锚点
（W93）——**`acceptance[].evidence` 零判据**。于是 `evidence` 可以被写空、写成不存在的路径、或指向
**工作区存在但仓库未跟踪**的文件，而**门控全绿**：`tested` 标签退化为无法复核的声明。

## 决策
1. **把该声明变成可执行判据**：遍历 registry **自己的键**、覆盖**全部在册 `tested` 项**（MUST NOT 扫手写清单
   ——清单的盲区恰是它的补集），三方向各一条断言：①非空且落在归档报告目录（相对 `docs/` 记 `reports/…`）；
   ②是仓库成员（`.git/index` 派生集）；③磁盘存在。
2. **判定顺序由可判定性决定**：先「形状」→ 再「仓库成员」→ 最后「磁盘存在」。**先判 tracked 再判磁盘**是硬要求：
   反序会让断言随本机磁盘状态变化，而宿主瞬态 MUST NOT 改变被测系统的结论（铁律二）。
3. **路径基准 MUST 显式**：registry 的 `evidence` 相对 `docs/`，`.git/index` 成员集相对**仓库根** ⇒ 判据必须先落
   `docs/` 再判归属。（首跑即因漏此而 11/11 假阳性 `not-tracked`，修正后 56/56——实测教训。）
4. **判据返回具名清单而非布尔**（`id:path:原因`），使失败可定位；每条守卫配注入反例（空 / 目录外 / 未跟踪）。
5. **闭世界 + 非空洞下限**：逐项断言扫描覆盖全部 `tested`；再设具名下限常量（A13 升级后实测 11），防「扫描恒空而恒真」。
6. **registry 回填只升真有证据者**：本轮 **A13 → `tested`**（静态结构事实，22 项断言在 portable+full 门控内实跑）；
   **A03 维持 `implemented`**（结构守卫虽通过，但剩余缺口是外部真值 ⇒ 按 spec 不具备可核验证据）。

## 后果
- 正向：`tested` 从「一句声明」变成「可复核的事实」；「守卫通过 ≠ 本项达标」被写进回填判据（A03 即反例）。
- **语义不变量**：`MET_STATUSES = ['implemented','tested']` ⇒ 本次升级**不改变** met/unmet 归类，
  对账类守卫（待裁定项的 precondition 判定）不受影响 —— 这也是「只升认证强度」风险可控的原因。
- 状态分布：`implemented` 28 → 27、`tested` 10 → 11；`pending/blocked/not_applicable` 不变。
- 零生产代码改动、零契约字段改动、零迁移、零 schema 改动。
- 如实披露：
  - 本轮**只升 1 条**。R1 的 15 条 `implemented` 中，8 条缺口在功能面（未建）、3 条本仓可闭合（接线/补覆盖/建语料）、
    3 条属外部真值、1 条待裁定 —— 这些**不得**因「补个守卫」而升级。
  - `tested` 的「测试计数」未进 registry（易漂移的数字不进活文档），其载体是证据报告本身。
