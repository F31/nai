# V48-01：基线、验收映射与物理边界

状态：Verified（2026-10-24，本卡工程范围）。来自[V4.8计划](../../v48-development-plan.md)，执行证据见[报告](../../reports/2026-10-24-v48-01-baseline-cutover.md)；不是164项产品ACC通过证明。

goal：将V4.8规划目标变成工程实际执行基线，保留现有实现/证据与全部既有编号，避免再次从空工程起步。

depends_on：无；现有RF01生成器、RF02 owner和新工作台作为已存在消费者复用。

allowed_paths：AGENTS.md、openspec/config.yaml、docs/design.md、docs/adr/0151-*、docs/architecture/、docs/acceptance/、docs/development/tasks/、schemas/catalog/、schemas/tests/或实际registry Schema路径、tests/registry.yaml、xtask/及相应注册表消费者。先定位实际Schema，不新增平行注册表。

contracts：CapabilityStatus、注册表结果/证据类型、catalog源指针；本卡只冻结被真实消费者覆盖的闭包，不把所有能力批量标Frozen。

migrations：业务数据无迁移；验收注册表保持旧只读快照/ID与证据引用，新增项pending。报告格式变化有明确版本和消费者测试。

invariants：INV-01–62全量映射，优先核验原01–40含义未被覆盖；INV-51/52/55–62新增义务具名记录。

ACC：ACC-CONTRACT-01/02、ACC-ASM-03、ACC-RUN-01的本卡局部前置证据；不提升整项通过。

fixtures：旧90项、完整164项、重复ID、未知INV、未知证据类型、缺必需闭包、声明能力被排除、Blocked误计Pass、条件未知误当NotApplicable。

commands（现有入口，执行前按实际CLI参数核验）：

- pnpm exec openspec validate <本卡变更名> --strict --no-interactive
- cargo xtask schema --check
- cargo xtask check-deps
- cargo xtask check --registry
- 注册表检查器的定向测试、受影响生成/消费者编译及新桌面bootstrap检查

evidence_kind：确定性契约/注册表检查、依赖静态检查、真实消费者构建。不是Windows媒体/平台/GPU认证。

external_prerequisites：项目锁定工具链；本卡不需付费Key、直播账号、GPU或用户数据库关闭。

done_when：

1. 实际注册表164个唯一ACC，旧90个ID及证据可追溯；62条INV逐项有来源，无自动通过。
2. ADR0151变Accepted并同步AGENTS/OpenSpec/设计入口/catalog/registry引用，不存在不同文件自称不同唯一基线。
3. 新版状态的缺安装、缺认证、缺权限、缺实现、运行故障可区分；未知枚举拒绝，Blocked不计通过。
4. 默认nai-* crate/模块/进程映射与现有消费者迁出顺序明确；无空服务、无kernel反依赖adapter。
5. 实际/目标工程命令清单区分；未实现命令不写成功占位，不将工具缺失伪称外部平台Blocked。
6. 修改来源契约则生成并检查无漂移，实际消费者编译通过；报告记录命令/环境/退出码/限制，预算和用户数据未改。

下一卡：V48-02共享模型与凭据安全迁移。该卡先形成可审查的来源映射、备份与冲突预览，再实施真实迁移；禁止单纯改workspace_id导致旧密文不可解。
