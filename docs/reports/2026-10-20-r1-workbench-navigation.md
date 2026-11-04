# r1-workbench-navigation（2026-10-20）

已实现：默认直播工作台；右上设置入口；连接与工作区、LLM、TTS、主备与智能路由、预检与诊断、升级查询、关于七个菜单。常用六个大图标按钮对应启动/连接、手工提问、审批候选、本机播放、立即静音、人工接管。状态详情折叠，提问与候选并列，批准后输出独立。

ProviderSettings 单实例挂载，按菜单隐藏，切换/关闭保留未保存模型和Key输入；导航不自动保存或调用Provider。保存仍为两组配置整体校验/显式落库；运行中须先停止引擎。设置内保留停止引擎与紧急静音。背景inert，Tab焦点约束，Escape关闭并回到入口。图标为本地SVG，无新依赖、无配置/IPC/数据库格式变化。审批两次确认与未批准播放门控保持；忙碌时已连接的静音入口可用。

实测：前端typecheck/Vite build通过；UI 121/121通过；真实生产dist Chromium通过七菜单、表单编辑跨菜单/关闭保留、Tab双向回环、Escape焦点返回、快捷提问聚焦与仅只读native调用断言。工作台/设置截图人工核验。Windows工程包构建通过，GUI Subsystem=2；真实WebView --smoke退出0并输出SHELL_UI_READY（退出时WebView/托盘清理诊断不影响结果）。

未执行全量Rust/契约测试：本轮仅变更UI呈现与浏览器测试，没有改契约或后端。无付费API请求，未修改用户数据库/密钥。registry验收状态不变。

产物：target-windows/shell/x86_64-pc-windows-msvc/debug/bundle/nsis/nai Engineering_0.1.0_x64-setup.exe；SHA256 14F90589008994467E4F8088A93F38CC106A2FFF56D9F0900D7008FD3045E838。Unsigned Engineering，非正式签名发布。经用户授权已更新 D:\nai Engineering 原安装目录：停机维护时备份旧程序后卸载/安装，6个AppData文件逐一SHA256相同；已安装程序GUI Subsystem=2，--smoke退出0并输出SHELL_UI_READY，随后打开新版。首次安装包本身仍不支持覆盖已有安装。
