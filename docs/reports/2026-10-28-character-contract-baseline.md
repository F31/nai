# 形象契约验证环境与协议基线（2026-10-28）

## 结果

`protocol.rs` 旧夹具使用已退出当前能力目录的 `nai.v1.ControlCommand`，使测试在期限和会话判断前因未知类型失败。夹具切换为当前冻结的 `nai.workbench.v1.FoundationRequest`，并明确断言旧类型仍被拒绝；当前目录无强制 session-scoped 类型，因此用非空空白会话检查缺陷输入，保留带会话时 epoch 必需的断言。原有 envelope 校验器的 v1 主版本语义未在本轮改变，也不把新目录版本作为该接口的互通证明。

新增形象动作状态的 Rust Protobuf 往返测试，覆盖中文、超过 JavaScript 安全整数的 epoch、最大序号、可选字段零值与缺失、未知枚举及未来字段。`cargo test -p nai-contracts --offline` 在 E 盘独立目录通过 12 项。TypeScript 契约编译通过；实时形象 OpenSpec 使用项目锁定的 pnpm 8.15.0、Node 22.23.1 和 OpenSpec 1.14.1 严格验证通过。Windows `node_modules/.bin/openspec.cmd` 缺失，已补本机生成的入口；未覆盖 Linux 安装或重装共享依赖。原构建目录存在链接器无法替换测试 exe 的情况，改用 `target-windows/avatar-baseline-check` 完成测试。

## 尚未完成

新形象消息尚无 Rust/TS/C++ 三方互通样本；许可复检、原子选择和撤权、独立素材传输与真实渲染接线未完成。原型的路径先检查再打开无法抵御父目录替换，不应启用；本机离线缓存中缺少可直接提供目录句柄相对打开的安全封装，不能将路径检查当作可信授权。需确定并评审可审计的 Windows 句柄方案、依赖边界和对抗测试后继续。未修改用户资料、密钥或运行版，未调用模型；验收注册表状态不变。
