# Tasks: W137 — O05 的成本来源

## 1. 勘定（先量，再动手）

- [x] 1.1 读基线 §1330 O05 原文：「升级成本显示：云资源、下载、存储与预估运行增量准确列示」
- [x] 1.2 读基线 §30.1 第 4 条（UI「升级与回滚提示」）与 §900 数据根子项清单——
      确认「在数据根下加一个清单收件箱」会与基线冲突（11 个子项里没有它），故不做
- [x] 1.3 现测四环：模型**已有**（`CostEstimate`）；控制面 `/upgrades/plan` **0 命中**；
      桌面 `control.ts` **无**升级方法；④包清单只来自外部厂商包
- [x] 1.4 现测契约：`/upgrades/plan` 复用 `*jobResponses`（202 + `job_id`），
      `OPENAPI_DOCUMENTS.implemented` 只有 8 条，均不含 upgrade 系列
- [x] 1.5 现测呈现口径对载荷的硬要求：`parseCostEstimate` 要求 `currency` 匹配 `^[A-Z]{3}$`、
      分项为 `{known: 非负安全整数}` 或 `{unknown: 非空原因}`、两层 `additionalProperties: false`
- [x] 1.6 现测清单 schema 的口径更宽：`currency` 为 1–8 字符、`amount` 为十进制字符串 ⇒
      两处口径**不合并**，差异在控制面以具名拒绝处置

## 2. 契约与生成物

- [x] 2.1 `/upgrades/plan` 改为 200 + `UpgradePlanView`，加 `x-implementation-status: implemented-loopback`
- [x] 2.2 新增 `UpgradePlanRequest`/`UpgradePlanView`/`CostView`/`CostComponents`/`CostLine`；
      `manifest` 只声明为 object，结构真相仍只有 `schemas/config/upgrade-package.schema.json` 一份
- [x] 2.3 `scripts/contracts/openapi-ops.mjs` 的 `OPENAPI_DOCUMENTS.implemented` 增 `post /upgrades/plan`
- [x] 2.4 用 `PROTOC=<仓库外 protoc 3.21.12>` 跑 `cargo xtask schema` 再生 Rust/TS 生成物
- [x] 2.5 `node --test tests/contracts/schema.test.mjs` 30/30 绿

## 3. 执行体与调用方

- [x] 3.1 `nai-control-api` 增依赖 `nai-upgrade-core`（shared→shared），并只让后者多导出 `MANIFEST_LIMIT`
- [x] 3.2 `upgrade_plan`：`plan_schema` 校验 → `UpgradePackage::parse` → `CostEstimate::from_package` → 四分量
- [x] 3.3 三道表示性闸门（币种 / 金额上界 / 清单不合规）各自具名拒绝，且都**不**产出成本面
- [x] 3.4 体上限按 `MANIFEST_LIMIT` 派生；命令口径的 4096 抽成 `COMMAND_BODY_LIMIT` 常量
- [x] 3.5 拒绝后改为**有界排空**（`body_limit × 4`）：否则带未读数据关连接给对端 RST，413 不可观测
- [x] 3.6 桌面 `parseUpgradePlan`（只认三键）+ `ControlClient.planUpgrade`；
      `COST_DECLARED_BASIS_NOTE` 说明未验签边界

## 4. 判据与注入

- [x] 4.1 控制面新增 9 条：权限 403（且无成本面）、响应满足 `UpgradePlanView`、未知带原因、
      显式 0、清单不合规具名、请求多键、币种、金额上界、体上限两侧
- [x] 4.2 `tests/ui/upgrade-plan.test.mjs` 新增 10 条（信封严格性、成本解析只有一处、来源边界内容锚点、客户端请求形状）
- [x] 4.3 注入 9 组全部真红：I1 未知当 0｜I2 去币种闸门｜I3 去金额上界｜I4 权限降为 read｜
      I5 体上限退回 4096｜I6 清单失败当成功｜I7 跳过体 schema｜I8 信封不判多余键｜I9 成本透传不解析
- [x] 4.4 注入基线 fmt-clean；`sha256sum -c` 四个文件逐字节 `OK`

## 5. 文档

- [x] 5.1 ADR 0112（plan 同步化、清单不重述、未验签边界、为何不开接缝、为何不读目录）
- [x] 5.2 `docs/control.md` 路由表新增 `/upgrades/plan`
- [x] 5.3 `docs/ui.md` 就地订正：第 4 条的前置从「plan/journal」收窄为「journal/执行状态」；
      O05 段补来源一侧；测试清单加 `upgrade-plan.test.mjs`
- [x] 5.4 dev-plan W137 小节 + `docs/knowledge.md` 结论
- [x] 5.5 5 处「本机缺/无 protoc」过期表述按「已发布记录只追加勘误」处置（dev-plan 3 + knowledge 2）
- [x] 5.6 报告 `docs/reports/2026-10-17-w137-upgrade-plan-cost-source.md`
- [x] 5.7 `tests/registry.yaml`：O05 notes 更新（**保持 `pending`**，逐项写清还差什么）；O02 的同名前提高同步订正

## 6. 收口

- [x] 6.1 门控 `check --profile full`（`CARGO_INCREMENTAL=0` + `CARGO_BUILD_JOBS=1`）取收口计数
- [x] 6.2 归档 openspec change + `validate --all --strict` + `validate --archived`
- [x] 6.3 一次 commit + push（SSH 443）
- [x] 6.4 CI 观测三件齐、逐平台完整步骤序列
- [x] 6.5 回填报告 §四/§六与 dev-plan、memory，清理临时文件
