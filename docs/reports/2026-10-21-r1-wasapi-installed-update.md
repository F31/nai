# 2026-10-21 安装版更新与人工音画体验准备

用户授权后已更新原目录 `D:\nai Engineering`，保留工程版应用身份与凭据绑定；未调用付费 API。

- 安装包 SHA256：`8fdc29ee34be6e817deb87a9e3b2a67f7326649b0ed41a9d4bf9cbd9e43c1082`，Debug、未签名工程版，不代表正式发布认证。
- 更新前完整备份安装文件和用户目录至 `target-windows/installation-backups/before-wasapi-20261021-224127`，经卸载/原目录安装完成更新。
- 首次核验因壳哈希不同停下；逐字节确认唯一差异为 Tauri 包类型标记 `__TAURI_BUNDLE_TYPE_VAR_UNK` → `__TAURI_BUNDLE_TYPE_VAR_NSS`。规范化此唯一标记后完全一致；引擎和 keeper 与打包源哈希完全一致。
- GUI subsystem=2；首次启动前六个用户数据文件数量及 SHA256 全部保留。启动时允许既有数据库迁移逻辑正常执行，不能将启动前哈希证据表述为启动后数据库永不变化。
- 安装版 `--smoke` 退出 0 且实际报告 `SHELL_UI_READY`，随后正常打开安装版供人工体验。

组合验证采用已通过的 full 前置检查，加失败点修复后的等价剩余步骤：fmt、workspace all-target clippy、workspace 测试及 xtask lib、前端类型/构建/测试。最后 UI 权限检查发现已有商品工作台命令漏登记于精确测试清单，补登记后 UI 167/167 通过。协议 fixture 补齐新增状态字段。full 包装命令此前失败，不记为一次 EXIT=0；日志保留在 `.nai-diagnostics/wasapi-full-continuation.log` 和 `wasapi-ui-recheck.log`，安装结果见 `wasapi-update-result.json`。

人工体验：连接引擎，商品知识准备/发布，准备直播输出，再显式生成、审批、本机播放。有限节目每段听完后使用“我已听完本段，提交引擎确认”。实际音频设备进度已接线；完整音画完成 ACK、自动连续讲品/插问归位、真人麦克风和 OBS 实录仍未验收。registry 状态不升级。
