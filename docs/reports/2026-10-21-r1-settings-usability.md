# 2026-10-21 设置易用性

## 实现

连接与工作区的“场景文件”“运行配置文件”均可点击“浏览…”打开Windows原生JSON选择器，也可粘贴绝对路径。“profile”显示为“运行方案名称”。选择仅修改表单；取消保留旧值，错误有提示，不自动保存/启动。引擎运行中禁用路径编辑。宿主新增精确枚举目标的窄命令`pick_setup_file`，单实例互斥，不读文件内容，不开放通用文件系统或shell权限；非Windows显式不可用。

旧共享Key折叠到“旧版场景兼容（通常无需填写）”：独立LLM/TTS配置用户留空；仅旧场景`provider.credential_env`共享凭据时填对应服务商API Key，非工作区密码/激活码，临时且不入库。

每条主备服务提供模型选择，预设自动填协议/基础地址/模型，TTS带默认音色；只有“自定义服务 / 模型”展开地址/模型编辑。空配置先提示选择；旧未知模型或代理地址原样作为自定义展示。切换不保存、不测试、不改主备ID/预算/路由；改地址/协议仅清除该行未保存Key，数据库原有凭据失效规则保持。音色可编辑，批量覆盖组合收进折叠项。

当前预设为用户提供的Agnes `agnes-2.5-flash`，以及官方文档核对的 [OpenAI GPT-4.1 mini](https://developers.openai.com/api/docs/models/gpt-4.1-mini) 和 [SiliconFlow CosyVoice2](https://docs.siliconflow.cn/docs/api/audio-speech-post)。预设不是实时模型目录、付费账号可用性或价格认证；未新增API适配器。其余模型可用自定义。

## 检查与边界

- 前端定向36/36（设置、Provider、桌面权限及新增易用性）；typecheck/生产build通过。
- 实际Chrome生产bundle：模型预设隐藏编辑、自定义保留值、跨菜单保留编辑、文件选择成功/取消、旧Key折叠、导航/焦点/挂载通过。宿主使用明确测试替身，不能冒充系统选择器实点。
- Windows独立壳all-target clippy通过；选择器非法目标/并发拒绝/取消释放状态机1/1，实际原生壳与同身份工程版构建通过。
- OpenSpec已归档为`2026-10-21-r1-settings-usability`，主规格44/44、归档188/188通过。
- 新Windows依赖rfd固定0.15.4，仅宿主Windows目标，新增windows-sys0.59.0；无常驻服务/端口。直连下载超时，使用既有本机代理完成固定依赖下载，不改系统代理。平台矩阵初测发现新增Cargo目标分支漏登记，补独立选择器能力后23/23。
- 原生检查首次未指定侧车目录失败；补`NAI_SIDECAR_DIR`指向已有同目标二进制后通过。工程构建有既有STATIC_VCRUNTIME弃用提示，仍静态CRT成功构建。
- 未进行真实供应商调用、未重跑全仓测试，不修改知识/审批/输出状态或registry验收级别。原生选择器实际点击选择文件仍需人工确认。

日志：`.nai-diagnostics/settings-ui.log`、`settings-browser.log`、`settings-native-clippy.log`、`settings-native-test.log`、`settings-engineering-build.log`与`settings-platform.log`。

## 安装版更新

已在无nai进程时仅替换 `D:\nai Engineering\nai-desktop-shell.exe`，保持工程版应用身份、静态CRT及NSIS上下文标记（仅将Tauri未打包标记UNK替换为既有安装的NSS，其他字节与工程构建一致）。安装前备份至`target-windows/installation-backups/before-settings-20261021-233239`。

更新前后8个用户数据文件数量与SHA256一致，引擎/keeper哈希不变；安装壳GUI subsystem=2，SHA256为`7af527daa811ef6b63ff7c27d8817fd4e65443dba448cc439abfffd6b167de4e`。安装版`--smoke`退出0且报告`SHELL_UI_READY`。本轮是桌面壳定向维护更新，不重新发布全套安装包；旧安装包仍为前一轮产物。结果见`.nai-diagnostics/settings-install-result.json`。系统选择器交互仍待用户人工确认。
