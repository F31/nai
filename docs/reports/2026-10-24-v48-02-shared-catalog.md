# V48-02 共享模型完整链路

2026-10-24。当前状态：用户关闭后，新版引擎和桌面壳均已构建；Windows真实宿主及隔离启动验证通过。未操作用户配置或发起付费请求。不能把本报告写成安装交付或真实迁移成功。

## 已实现

- 共享目录独立SQL表、版本和hash；LLM/TTS增删改和调用路由统一使用共享目录，预算/任务留在业务工作区。全局模型修改仅root owner。
- 迁移Protobuf命令、状态和可恢复映射，原生host allow/read接线。
- 包含回收站的完整来源映射，冲突独立保留新ID，拒绝遗漏、重复、超限和源版本变化。
- 准备journal、绑定已验证一致备份、逐项作用域重加密及回读、最终单事务发布。重启跳过已完成项；发布前原目录继续可读。原配置/历史引用保留。
- 新任务只允许共享ID；历史读取仍能解析原模型。工作区切换共享配置但不共享预算。
- 界面读取预览/恢复状态、明确勾选确认、有限自动步进和错误停止；可明确放弃未提交迁移恢复原配置管理，保留备份和审计。

## 验证证据

Windows nai-foundation-sqlite --lib：87 passed/0 failed，其中6项共享目录用例涵盖跨重启续办、来源保留、回收区与同名配置、共享CRUD/权限、预算隔离、两种worker准备路径、坏备份/凭据失败关闭、显式放弃、空目录和容量超限。

Windows真实NativeVault重加密测试：1 passed（循环覆盖LLM/TTS，测试专属临时条目，源仍可读，目标作用域回读及错误作用域拒绝，测试条目清理完成）；不是用户密钥迁移，也没有网络调用。

前端迁移状态测试5/5，相邻工作区测试16/16，专用工作台TypeScript/生产构建通过。源契约生成及schema --check无漂移。相关storage/host/engine all-targets clippy -D warnings通过。

真实host集成8项通过，包括新增共享迁移IPC/owner重启续办与跨工作区模型/预算断言；新版engine bins与桌面壳构建通过，隔离启动smoke EXIT=0，输出WORKBENCH_PREPARATION_READY，PE GUI子系统、原生权限探针及认证owner protocol=2通过。宿主测试初次链接遇到LNK1104，既有脚本重试后8/8成功，日志为model-catalog-native-workspace-management-host.log.retry。

## 剩余交付

1. 已完成engine构建及8项真实host测试。
2. 已完成桌面壳及隔离启动/权限smoke；已重新打开新工作台，并确认新版桌面及foundation引擎进程存活；未自动执行共享迁移。
3. 用户在新界面核对来源和确认后执行实际迁移，再人工确认跨工作区列表及LLM/TTS配置。

系统Keyring和SQLite不具跨介质事务，seal后中断可能留下有界孤立目标条目；未做真实断电认证或自动清理。备份不导出Keyring，不以旧快照覆盖活动库回滚。164ACC仍pending。V48-03及直播音画主线尚未在本轮实施。


