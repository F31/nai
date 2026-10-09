# Design

## Context

D02 登记 DTO 缺少完整版本引用，D03 存储可保存快照但 kernel 尚无逻辑。
现无真实宿主/插件，Linux/WSL 仅验证纯逻辑；测试证据不作为真实准入。

## Goals / Non-Goals

**Goals:** 清单转换、有界注册、明确依赖/主备与会话快照、可解释准备结果。

**Non-Goals:** 启动进程、验签/资源实测、IPC 身份、模式租约执行与内容策略、持久接线。

## Decisions

1. Protobuf 增加 PluginRegistration、BoundProvider、BindingRequirement/Selection/Plan、PrepareReport/Issue。
   Binding/Descriptor 增加可选身份字段；保留旧 v1 字段，不提升未知旧快照的信任。
   JSON Schema 新建 binding-plan，配置默认不自动引入LLM/知识；按显式场景配置。
2. plugin-sdk 用同源 Schema、semver 与明确目标检查 manifest，产生注册 DTO。
   标记 validated_kernel_version 并非认证；trust/license/resource/host 证据由可信装配层独立提供。
   kernel 只依赖 contracts+std，不依赖 SDK/json/storage/Tauri/平台库。
3. Registry 为有界 BTreeMap，owner 行为 API 不是插件可调用的授权端口。
   每次操作带本地 CallContext，取消/到期及图规模/深度受限。
   安装检查全局能力声明图循环，保守拒绝包含循环的配置；未来需要可替代循环路径须独立规格。
4. 配置指定主备顺序；没有配置时仅允许唯一已登记实现，多实现即歧义。
   准入证据绑定完整 identity、chosen host、review/resource/cert/license 和 policy_scope。
   fallback 只按显式链且与 selection 的 scope 一致；健康/授权缺失不自动信任另一数据域。
5. 健康与权限由独立 owner 更新，不从manifest推断；TTL/来源/枚举严格复检。
   权限以 session/plugin/capability 保存；默认 Unknown。健康的 ready 只是有来源的观测，实际宿主检测在D05。
6. 外发需求隐式加入guard，avatar.render 加入avatar.profile；auto加入平台/知识必需项，
   平台快照必须明确 `auto.supervised` granted 与期限/来源，并有值守租约及预算准入引用。
   这些条件来自可信 root 的准备材料，D06/D07/D13实际签发/验证尚未实现。
7. Ready/Degraded 才可 freeze；快照保存所有闭包绑定和版本。冻结后更新/移除被引用包拒绝，
   更新健康/权限/撤销允许，live recheck 可失败且不改快照。release 仅可信 root 维护/结束使用。
8. 同一内核不限 edition；registry/session上限为可配置结构边界，不承诺Pro设备容量或最终开播。

## Risks / Trade-offs

- 独立材料尚无外部验证器 → 空生产 registry 默认NotReady；只有 tests 使用演练证据。
- 依赖闭包有组合选择 → 按明确链逐候选检查，失败可回退到显式备用；不随机选择。
- 字符串支持范围 → SDK标准semver解析，kernel对具体 validated target 和协议再检查。
- 冻结不等于永远可用 → 每次复检状态/权限，D06还要检查epoch/审批/租约。

## Verification

kernel suite（含SDK）、portable、生成无漂移、三语言新报告/快照互通、JSON配置正负样本、前端/OpenSpec。
额外 Windows GNU kernel 交叉检查；不作为 Windows进程/设备认证。
