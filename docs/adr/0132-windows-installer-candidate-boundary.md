# ADR 0132：Windows NSIS 安装候选与资源边界

- 状态：接受
- 日期：2026-10-20
- 依据：V3.11 §15、§29.5；ADR 0129、0130、0131

## 决策

首个 Windows 安装产物采用 Tauri 原生 NSIS/currentUser，不自建安装器；随包分发真实目标侧车与有许可的演示资源。
宿主通过 Tauri resource_dir 解析默认场景，已保存设置优先。安装目录只读，数据仍位于 app_data/runtime，
不把源码绝对路径嵌成 release 默认值，也不随包存储 API 凭据。

未签名产物仅为显式请求的工程候选；哈希用于内容标识，不构成来源或签名认证。正式签名链、漏洞与许可
审计及完整发布流程仍需独立证据。NSIS 构建成功、Windows 安装成功与设备播放/真实 API 成功是不同事实。
工程包使用 `nai Engineering` 与独立 identifier `cn.nai.desktop-shell.engineering`，隔离正式安装和数据目录。

该入口是首次安装候选，不授予维护窗口外升级的权利；生产升级仍必须经 nai-upgrade-core 的 journal、备份、
兼容范围与恢复流程，本轮未接该流程，不宣称可用于生产升级。
安装 hook 在维护选择页前拒绝已有安装；静默模式在文件写入前重复检查。活动 engine/keeper/壳或进程检查
不可用时拒绝，而非采用 NSIS 默认的强杀。只有显式退出并卸载工程候选后才能重新首次安装，用户数据应保留。

## 成本

增加 NSIS/Tauri CLI 构建工具维护，无新增产品常驻服务或运行时依赖。WebView2 缺失时安装器需要下载引导，
断网且无 WebView2 的机器不能被描述为已可用。独立壳仍不进入 portable/full 原生构建闭包。
