# ADR 0005：内核登记与冻结绑定边界

- 日期：2026-10-07
- 状态：Accepted
- 对应：D04；V3.5 第3/4/21.2章

## 决策

1. kernel 只依赖 nai-contracts 与 std；不引入 concrete plugin、SDK、storage、Tauri 或平台库。
   plugin-sdk 负责同源 JSON Schema/semver/目标检查，返回生成 PluginRegistration；不创建运行实例。
   SDK 的 semver 1.0.28 与 Schema/SHA 工具为明确职责依赖，沿用既有锁定版本。
2. Binding/CapabilityDescriptor 保留原v1字段，加完整 provider/version/artifact/manifest/host/policy_scope。
   新增注册、计划/需求/选择、自动前置材料与报告 DTO。旧快照缺versioned selected不得视为有效授权。
   能力目录仍27条，编译生成 CAPABILITY_IDS，不在kernel手写另一份能力目录。
3. Admission 是可信 root 的本地输入，不从插件 JSON 反序列化。必须匹配 manifest 与 artifact 等身份，
   有review/license/resource/cert/host-boundary引用；inproc还要求可信root确认编译期实现。
   本轮消费验证结果，不实现验签、签名信任库、资源实测或 OS 宿主约束；生产没有这些证据时拒绝登记。
4. 注册图包含 manifest requires 和不可撤销硬依赖。能力声明图全局循环保守拒绝，即使将来有替代路径；
   此取舍简化安装保证，需复杂条件图时独立变更。失败不覆盖原注册表。
5. 未显式选择时只允许唯一已登记 provider；多实现不能按健康状态随机挑一个。
   备用只按显式顺序；注册的任何备用与 selection scope 不一致则拒绝，避免跨KB/上传政策边界。
   遍历候选时使用暂存闭包，失败候选不留下部分依赖。
6. 健康与权限独立输入；健康期限/来源revision、权限authority/revision/TTL逐次检查。
   权限以session/plugin隔离，声明 network/media_output 等不是call grant。
   CapabilityView 中仅完成闭包且当前可用的选中实例标Ready；未选中不伪报已激活。
7. output/tts硬依赖guard，avatar.render硬依赖profile/guard，emit_output要求output/guard。
   Auto额外要求平台三能力、知识/守门、`auto.supervised`明确授权及监督期限/预算引用。
   自动材料必须由可信root汇总；D06/D07/D13负责真实签发和执行，D04不替代这些认证。
8. ReadyDegraded表示可选缺失或显式备用；NotReady没有可冻结快照。
   Ready仅绑定预检。Registry不启动宿主、模型、网络，也不执行内容/播放授权。
9. 冻结所有闭包绑定的selected完整身份。active使用期间拒绝移除/换版，允许健康/权限/准入撤销。
   recheck只验证原选择，不静默切备用；切换须重新准备及遵循句边界/维护窗口规则。
   approved_fallbacks是独立policy准入身份引用，不表示其闭包、健康或租约已获准启动。
10. Registry规模默认256插件/32会话、依赖64、深度32，结构上限可配置到受限范围。
    这些是内存/算法界限，不是Pro运行容量认证。CallContext最长30s，取消/期限在有界遍历中复检。
    boot_id由可信root提供且每次进程启动唯一；重启不恢复Registry旧快照。

## 验证与范围

测试使用明确 test-only 准入/健康/授权材料，未把这些引用当真实认证。
Linux/WSL kernel/SDK与契约测试、Windows GNU kernel代码/测试检查；Windows宿主/设备仍未测。
暂无数据库迁移，D03持久化接线及D05/06流程后续实施，无新增常驻服务或API调用。
