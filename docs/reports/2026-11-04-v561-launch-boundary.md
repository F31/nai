# V5.6.1 启动装配边界

此前 `scripts/v561-start.ps1` 实际指定旧 `nai-foundation-engine` 和旧桌面程序，后者仍消费旧 Store 与协议；把这条链路称为 V5.6.1 并把新数据根交给它，与不迁移旧数据的基线冲突。

启动入口现返回 `NAI_V561_ASSEMBLY_NOT_READY`，不创建数据根、不启动程序；`-AllowDegraded`、`-NoProbe`、`-Build` 均不能绕过。`nai-v561-probe` 保持独立可执行，其打开新目录会初始化数据根，使用者应明确指定路径。

`pwsh -NoProfile -File tests/v561/launcher-boundary.ps1` 通过：五组参数和已有旧根均拒绝启动；现有文件哈希及目录内容不变。OpenSpec 使用 E 盘项目锁定工具链严格校验通过。没有调用模型、启动旧工作台或修改用户数据库。

新工作台启动器尚未实现；需新引擎装配 V5.6.1 Store、新协议与 UI，不能通过现有旧引擎替代。M0 FlashHead/Qwen/OBS 真实验收、模型完整性及许可仍未完成；本次安全修复不授予验收通过。
