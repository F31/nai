# Tasks: W138 — O02 的另一半：作业状态接上真实 journal

## 1. 勘定（先量，再动手）

- [x] 1.1 读基线 §30.1 第 4 条「升级与回滚提示」与 registry O02 原文（W137 已收窄为「只缺 journal/执行状态」）
- [x] 1.2 现测 journal：`upgrade_journal` 表、`UpgradeStep` 五态、`record_upgrade`/`upgrade_record` 均在
      `crates/nai-storage/src/upgrade.rs`；`Store` 是 `UpgradeJournal` 的生产实现
- [x] 1.3 现测编排：`UpgradeTransaction`、`Composition::stage_upgrade`/`activate_staged` 已有（带 `SessionGate`）
- [x] 1.4 现测安装器：`impl UpgradeHost for` 全仓只有 `rehearsal.rs:153` 的 `RehearsalHost`（不碰磁盘）⇒ 生产实现为零
- [x] 1.5 现测控制面：`/jobs/{id}` **零分支**（落 404 `CapabilityAbsent`）；execute/rollback **零分支**
- [x] 1.6 现测契约：`*jobResponses` 被 **14 条**路由复用，其中 12 条与 upgrades 无关且同样无状态来源
- [x] 1.7 现测状态值域：`operationErrors` 的 `expected = declared ? IMPLEMENTED_STATUS : undefined`
      ⇒ 新增第三个状态值必须放宽既有闭世界，否决
- [x] 1.8 现测错误码：`ErrorCode` 16 个值里**没有**「所命名的对象不存在」；`descriptor.test.mjs:48`
      有「openapi 错误枚举 == proto 枚举」的耦合断言
- [x] 1.9 现测 `Access::new` 的 `permissions.len() > 8` 是**单令牌**条数上限，但全局名集已有 9 个，
      且引擎 `CONTROL_PERMISSIONS` 用满 8 条、**不含** `runtime.upgrade` ⇒ W137 的 plan 对唯一令牌恒 403

## 2. 契约与生成物

- [x] 2.1 `schemas/core/error.proto` 增 `NOT_FOUND = 17`；`control.yaml` 的 `ErrorResponse.code.enum` 同步
- [x] 2.2 新增 `JobView`（`additionalProperties: false`，`step` 五值闭集），`/jobs/{id}` 改 200+404+503
      并标 `x-implementation-status: implemented-loopback`；补 `NotFound`/`ServiceUnavailable` 两个响应组件
- [x] 2.3 execute/rollback 增 `503` 声明与描述（点名平台安装器未注入），**仍不**标 `implemented-loopback`
- [x] 2.4 `scripts/contracts/openapi-ops.mjs` 的 `implemented` 增 `'get /jobs/{id}'`（9 → 10 条）
- [x] 2.5 `PROTOC=<仓库外 protoc 3.21.12>` 跑 `cargo xtask schema` 再生，再跑 `schema --check` ⇒ no drift
- [x] 2.6 `node --test tests/contracts/descriptor.test.mjs` 2/2、`schema.test.mjs` 30/30 绿

## 3. 执行体与调用方

- [x] 3.1 `UpgradeStep::ALL`（五态单一列举，供「契约值域 == 枚举」判据取用）
- [x] 3.2 `JobBackend`（持 `Arc<Mutex<Store>>`，照 `ApprovalBackend` 同形）+ `with_jobs` 装配
- [x] 3.3 权限表：`get /jobs/{id} => job.read`；execute/rollback 的 `runtime.upgrade`（clippy 要求合并同值分支）
- [x] 3.4 `jobs_get` 三态：200 `JobView` / 404 `NOT_FOUND` / 503 `CAPABILITY_NOT_READY`（持锁 409）
- [x] 3.5 execute/rollback 具名拒绝（503，`message` 点名缺平台安装器），**不**铸 `job_id`
- [x] 3.6 `apps/engine` 装配接上 `with_jobs`；`CONTROL_PERMISSIONS` 补两条（并修掉 plan 恒 403 的既有缺口）
- [x] 3.7 `Access::new` 的条数上限改为从 `PERMISSION_VOCABULARY` 派生，并拒绝表外权限名

## 4. 判据与注入

- [x] 4.1 控制面 **10** 条（三态各一条、五态逐条、`terminal` 由枚举派生、权限 403、契约值域==枚举、
      拒绝含内容锚点、响应满足 `JobView`、超长 id 400、持锁 409）；39 → **49**
- [x] 4.2 `tests/ui/upgrade-job.test.mjs` **9** 条（信封严格、未知步骤呈名为缺口、可选项只认缺省、客户端请求形状）
- [x] 4.3 注入 **7** 组全部真红：I1 未知 id 当 200｜I2 无后端当 404｜I3 execute 铸 job_id｜
      I4 拒绝消息去掉锚点（非空仍红）｜I5 `terminal` 另写一份｜I6 删 `job.read` 权限行（多条同红，如实记录）｜
      I7 只调换契约枚举顺序（集合不变仍红）
- [x] 4.4 注入基线 fmt-clean；7 组还原后 `sha256sum -c` 五个文件逐字节 `OK`；`schema --check` 仍 no drift

## 5. 文档

- [x] 5.1 ADR 0113（三态、为何加 `NOT_FOUND`、为何 execute 不能铸 id、12 条同族缺口具名、残余风险）
- [x] 5.2 `docs/control.md` 路由表新增 `/jobs/{id}` 与两条拒绝；`docs/upgrade.md` 作业状态段 + 未做边界订正
- [x] 5.3 `docs/ui.md` §30.1 第 4 条就地订正（读者已有、写者仍缺）；测试清单加 `upgrade-job.test.mjs`
- [x] 5.4 dev-plan W138 小节 + `docs/knowledge.md` 结论（三条可复用判据）
- [x] 5.5 报告 `docs/reports/2026-10-17-w138-upgrade-job-state.md`
- [x] 5.6 `tests/registry.yaml` O02 notes 收窄（**保持 `pending`**）；`node --test tests/registry.test.mjs` 57/57

## 6. 收口

- [x] 6.1 门控 `check --profile full`（`CARGO_INCREMENTAL=0` + `CARGO_BUILD_JOBS=1`）取 `GATE_EXIT=0` 与计数
- [x] 6.2 归档 openspec change + `validate --all --strict` + `validate --archived`
- [x] 6.3 一次 commit + push（SSH 443）
- [x] 6.4 CI 观测三件齐（`cb=` 纳秒 + `no-cache` + 回读 `head_sha`），逐平台完整步骤序列
- [x] 6.5 回填报告 §四/§六与 dev-plan、memory，清理临时文件
