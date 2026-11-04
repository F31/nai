# 数字人直播产品差距审视（2026-10-21）

## 结论

用户对控制台实际可用性不足的判断成立。已有安全与工程基础，但商品准备、连续讲解、稳定直播音画、插问恢复与真人音轨没有组成统一可操作路径。下一主线见 [R1交付计划](../r1-delivery-plan.md)，不再默认以零散提示修复为产品里程碑。

## 核对依据

- 唯一基线 V3.11：§8.4–§8.5 常驻输出与真人音轨、§9 直播工具与平台、§10/§15 工作台和首句、§51 R1边界、§52 三轨准入、§53 基础讲品、§54 商品指标、§56 活文档订正。
- `tests/registry.yaml`：A12/O01/O02/O05仍pending；B01/O03/O04仍blocked；B05/B06/B07为implemented，不等于完整界面已交付；P01/B02在R2，O03在R1；本轮未修改任何状态。
- `apps/engine/src/operator.rs` 已调用真实Provider与装配/观测记录；不能因 `pipeline.rs` 保留演练桩而宣称生产生成未接。
- `apps/output-keeper/src/local_playback.rs` 样本上限为采样率×5，单作业创建窗口，当前增益15%；这属于短句本机试播，尚非持续直播输出。
- `crates/nai-kernel/src/evaluate/show.rs` 有ShowPlan校验与版本检查点边界，不能据此宣称连续讲品编排已接UI。
- 真实平台插件当前为 `plugins/platform-unavailable/`；缺仓内生产者与官方外部证据须分别登记。
- Provider单项真实探针、配置加密数据库和Windows短句/静音证据见交付计划底部报告链接；本轮未新增实机或付费证据。

## 本轮交付与边界

重写交付计划，给出P0-1知识/场景、P0-2常驻音画录制、P0-3讲品归位，以及真人接管、复盘、发布收口与平台独立轨的落点、依赖和退出条件；更新开发计划入口。订正过期的“仅演练生成”“账号缺失”“无设备接线”等当前描述。没有改基线、发布线、代码、契约、数据库、知识许可或安装版，也未调用付费API。

首个验收演示：UI录入商品事实与明确许可→发布→分段讲解→人工插问→生成/审核→回答→恢复讲解→OBS录制。10分钟录制是新增实施验收目标，未执行；O01首句10分钟仍按基线前提独立计时。真实开播还须目标软件和平台条件，录制不等于自动互动准入。

## 检查

实施前与实施后 `pnpm exec openspec validate r1-live-product-roadmap --strict --no-interactive` 均通过（WSL项目锁定CLI）。已归档为 `2026-10-21-r1-live-product-roadmap`，归档后 `pnpm exec openspec validate spec-driven-delivery --type spec --strict --no-interactive` 通过。两份新规划文档的本地链接、计划中的registry编号均核对通过；`git diff --check` 初次发现开发计划尾部空行，修正后通过。文档任务未运行Rust/前端全量测试；归档表示本轮审视规划完成，不表示后续产品功能完成。
