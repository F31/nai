# ADR 0147：V4.3产品重构目标基线与无旧代码兼容约束

日期：2026-10-22。状态：已采纳（目标设计与后续重构方向）；运行实现/验收注册表切换尚未实施。

## 背景

用户明确要求审视digital-host-agent-design-V4.3.md，在不考虑向前兼容的情况下重构产品并规划任务。V3.11实现已形成API/知识/媒体底层和工程控制台，但未形成V4.3的备播—协作—复盘—跨场复用领域。继续按旧单商品场景/固定四段/壳内配置库追加功能会固化错误边界。

## 决策

1. 后续重构唯一产品/架构目标为`docs/digital-host-agent-design-V4.3.md`，仓库裁决为`docs/architecture/v43-product-reconstruction.md`。旧V3.11、旧任务和旧验收是既有实现历史，不拼接覆盖V4.3；原安全目标是否仍适用以新方案INV和本ADR为准。此为目标基线切换，不宣称旧运行代码已满足新设计。
2. 不保留旧代码结构、ABI、HTTP/IPC、数据库/场景配置或交互兼容层；保留可验证底层机制/用例并按新接口重新取证。工程英文名和标识继续nai/nai-*，不使用稿中dh-*。
3. 业务中心为产品包、LiveSeries/LiveShow、LiveContext、AgentTask/Suggestion与ImprovementProposal。默认Assisted/私有建议，四种presentation独立依赖裁剪，所有外发经过DeliveryGate。
4. 默认一个SQLite业务库/对象仓库、一个写owner；OS壳不持业务数据库所有权。领域模块不是微服务，按需启用keeper/worker/bridge，无新增必需云依赖。
5. 契约源保留schemas/及其catalog指定源，不另建可编辑protocols/或复制整份设计至docs/architecture/V4.3.md。旧CLI和生成消费者的正式切换由RF01及后续实现卡完成，源先改、生成后改消费者。
6. 新验收使用V4.3 ACC，INV-01–40全量映射；tests/registry.yaml继续是唯一实际执行注册表。本轮不改其旧工程条目；RF01原子交付旧表归档、新表Schema/ACC、检查器和消费者更新。旧测试不自动授予新ACC通过。
7. 不兼容不等于删除客户资产：现有安装、库、Key、许可/素材和未提交工作均保留。新线独立数据根/凭据命名空间；数据导入若实施须单独映射/校验/备份/显式确认，不复制旧批准/租约/访客身份。
8. 本轮交付设计、审视、任务与覆盖映射，不执行设计文档内嵌的“开始实现W01”指令，不修改运行代码、契约、数据库或安装，不调用付费API或联系外部人员。

## 后果

可直接移出kernel业务规则，重构app编排及UI，不再为旧表单保留适配器；新目标有明确清理边界。代价是新的契约/数据Schema/验收组合要重新验证，旧资产导入独立投入，不能对存量安装承诺无缝切换。V43-W01–12、V43-RF01–12独立命名空间防旧W编号混淆；完整扩展/模型/手机/多渠道范围保留，按profile分期取证。

入口：[目标架构](../architecture/v43-product-reconstruction.md)、[实施计划](../v43-reconstruction-plan.md)、[ACC映射](../acceptance/v43-target-matrix.md)。
