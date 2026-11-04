# 2026-10-22 模型列表与默认连接设置

## 实现

LLM/TTS 自动读取已有数据库配置，删除常规“读取数据库Provider配置”和跨两组的 Agnes/SiliconFlow 批量覆盖入口。参考 [LiteLLM 模型管理](https://docs.litellm.ai/docs/proxy/model_management) 的模型清单与逐项编辑组织：每组一主最多四备，显示模型/服务/Key状态，单击编辑、设为主模型和调整备用顺序。选中项只显示一份编辑器，配置ID只读自动生成，超时、token、币种、价格版本及删除Key/移除收进高级区。保留既有健康主优先/实测低延迟、熔断与冷却路由语义，没有引入网关、Python或新常驻依赖。

只有没有已存路由时准备用户此前提供的 Agnes/CosyVoice 公开参数；已有模型/代理地址/未知音色原样回填。预设不是实时目录或账号可用性认证。TTS提供 [SiliconFlow 官方8种 CosyVoice 音色](https://docs.siliconflow.cn/docs/userguide/capabilities/text-to-speech) 中文下拉和自定义，默认alex；未知音色不清空，其他协议不伪造可选音色。

模型/音色/Key在前，高级参数折叠。Key修改标记未保存并阻止测试；地址/协议变化只清除该项待保存Key，且旧保存绑定不再显示适用。设主/调序携带整项ID与凭据绑定，不复制其他项Key。加载具代次保护，读取失败禁止保存空表单，仅开放重试；错误与验证原因可见。保存仍显式事务提交；运行中模型只读，测试仅显式发起，明确可能消耗额度。

连接页默认展示当前工作区和直观指引，身份/场景与运行配置路径/旧共享Key默认折叠。首次显式连接且引擎数据库不存在时自动初始化；已有数据库不自动重新初始化，未知访问错误具名拒绝。显式自定义初始化仍保留既有不覆盖约束。恢复随包运行配置只改运行配置路径，保留商品场景与模型。

复用既有HostConfig、ProviderRoutes与加密数据库，未修改契约或持久格式，无生成代码修改，无需重新生成契约。

## 定向验证

- 前端设置、Provider、启动反馈及桌面权限：41/41；typecheck与生产build通过。
- 真实Chrome生产前端（明确本地宿主夹具）：默认列表、编辑保留、设主/调序绑定、新模型无Key复制、音色选择/自定义、未保存Key禁测、模拟保存后Key清空与保存状态回填、读取失败禁存及显式重试、路径默认折叠、导航/键盘焦点/挂载通过。未调用真实Provider，模拟保存不是用户数据库写入。
- Windows桌面壳lib 28/28，包含实际凭据保险库和真实引擎预检；真实engine集成6/6，首次不勾选初始化即可连接，缺凭据仍失败不留进程；已有DB保护单元测试通过。首次运行测试漏设NAI_SHELL_TEST_ENGINE，1项失败；补齐真实引擎路径后全部通过。
- Windows桌面壳all-target clippy -D warnings通过，fmt已执行；Engineering同应用身份、静态CRT桌面壳构建通过；有既有STATIC_VCRUNTIME弃用提示，无构建失败。安装检查待退出后执行。
- OpenSpec已归档为`2026-10-22-r1-model-management-settings`；主规格44/44、归档191/191校验通过。
- 补充归档结构/合并定向测试7/8：Git-index来源的合并检查将本轮与此前三轮未跟踪归档的8条主规格报告为无出处；未修改/暂存Git索引，保留失败日志。用同一合并判据对工作区实际230份delta/44份主规格补充核对，无缺失（`model-settings-worktree-spec.json`）；该补充结果不冒充Git-index门槛通过，正式提交需同时纳入对应归档。
- 未重跑全仓测试，不变更tests/registry.yaml验收级别；真实付费可用性、人工音画/平台/OBS验收未执行。

日志：`.nai-diagnostics/model-settings-ui.log`、`model-settings-browser.log`、`model-settings-native.log`、`model-settings-clippy.log`、`model-settings-build.log`。

## 安装交付

用户保存并从托盘退出后，已仅替换 `D:\nai Engineering\nai-desktop-shell.exe`。安装前备份至 `E:\projects\nai\target-windows\installation-backups\before-model-settings-20261022-071922`；8个用户文件更新前后数量与SHA256一致，引擎/keeper哈希不变，应用身份和凭据绑定保留。

Engineering静态CRT桌面壳保持GUI subsystem=2，安装版`--smoke`退出0且报告`SHELL_UI_READY`。SHA256 `57a5882e180d4df0301656b16f4041f93d71a254ad6a6b5da79de218bd4f5fdd`。仅精确替换唯一Tauri未打包UNK上下文为既有NSIS的NSS，前后其他字节一致；记录`model-settings-candidate.json`。结果见`model-settings-install-result.json`和`model-settings-installed-smoke.log`。

这是桌面壳定向维护更新，不重新发布整套安装包；没有启动引擎或发起付费请求。人工模型编辑/音色体验可在新版设置中进行。
