# Design: W122 — `tested` 的证据可核验

## 缝（现测，不是推断）
`admission-registry` 的「Honest backfill mapping with traceable evidence」声明：`tested` 仅可用于**已有归档实测
证据**的项目，且**证据 MUST 指向真实报告与测试计数**。但 `tests/registry.test.mjs` 里 `grep evidence` 只命中
`landing` 的内容锚点（W93），**`acceptance[].evidence` 零判据** ⇒ 那句声明没有载体。

后果：一个 `tested` 项的 `evidence` 可以被写空、写成不存在的路径、或指向一个**未跟踪**的文件（在工作区里存在、
但不在仓库里），而**门控全绿**——`tested` 标签于是退化成一句无法复核的声明。

## 决策
1. **遍历 registry 自己的键，覆盖全部在册 `tested` 对象**（MUST NOT 扫一份手写清单——清单的盲区恰是它的补集）。
2. **三方向一起判，且顺序由可判定性决定**：先「形状」（非空 → 落在归档报告目录），再「仓库成员」（`.git/index` 派生集），
   最后才「磁盘存在」。**先判 tracked 再判磁盘**很关键：否则断言会随本机磁盘状态变化——
   而宿主瞬态 MUST NOT 改变被测系统的结论（铁律二）。
3. **路径基准 MUST 显式**：registry 的 `evidence` 是**相对 `docs/`** 的引用（与 `development-plan.md` 的相对报告
   链接同一约定），而 `.git/index` 派生的成员集是**仓库根**相对路径 ⇒ 判据必须先落 `docs/` 再判归属。
   首跑即因此假阳性（11/11 报 `not-tracked`），修正后 56/56 通过——这是本轮的实测教训，不是设计推测。
4. **判据返回具名清单，不是布尔**：`evidenceDefects(items)` 返回 `id:path:原因` 清单，使失败可定位。
5. **每条守卫配注入反例**（空 / 目录外 / 未跟踪各一条），证明它不是恒真。
6. **闭世界 + 非空洞下限**：逐项断言扫描覆盖全部 tested（不静默跳过）；再设一个具名下限常量
   `W122_TESTED_FLOOR = 11`（A13 升级后实测），防「扫描恒空而恒真」，并在注释里写明下调必须先说明理由。

## registry 回填的判据（为何只有一条升级）
- **A13（升 `tested`）**：命题为静态结构事实；`tests/platform/platform-matrix.test.mjs` 22 项经 `engineering_checks`
  在 portable 与 full 门控内**实跑**（本机实测 TAP `1..22 / # pass 22`），核心门控「全仓平台分支 ⊆ 矩阵登记路径」
  与 15 条非空洞注入守卫分别抓到；无外部依赖 ⇒ 具备完整认证证据。
- **A03（维持 `implemented`）**：两条结构方向（内核→连接器分层规则、连接器→内核）**确已**在门控内实跑通过
  （`deps::tests::a_platform_connector_never_depends_on_the_kernel`、`plugins/platform-unavailable/tests/unavailable.rs`），
  但 notes 的既有裁定指明剩余缺口是**外部真值**（真实官方接口 / §9.3 平台准入决定）⇒ 按 spec 维持，不因结构守卫
  通过而升级。**「守卫通过」≠「本项达标」**。
- 其余 13 条的缺口类别见报告 §三（本仓可闭合 3 条、待建功能 6 条、外部真值 4 条）。

## 明确不做
- 不改任何生产代码；不改 registry 的既有状态语义（`MET_STATUSES` 仍含 `implemented`，本次升级不改变 met/unmet 归类）。
- 不为「测试计数」另立判据（`tested` 的测试计数在报告里，本轮的载体是**证据报告本身**；把计数写进 registry 会让
  易漂移的数字进活文档）。
- 不批量升级其余条（外部真值/待建功能的项升级即为不实声明）。
