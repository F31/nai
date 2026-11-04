# ADR 0154：退役旧桌面 UI

日期：2026-10-24；状态：采纳。

新备播前端由 apps/workbench-desktop 自有 package、ui、Vite 配置及 index.html 装配。删除 apps/desktop 的旧 React 页面、旧桌面壳与专用安装/测试脚本，工程门控只引用在用前端。历史底层引擎保留，旧控制协议仅保留无 UI 的测试夹具，不构成兼容产品入口。

删除前核验本地恢复快照 .nai-diagnostics/legacy-ui-retirement/source.zip（125 文件），逐文件校验 SHA256；未提交的旧源码和差异补丁可恢复。旧缓存、图标和二进制也保存在诊断目录。快照不提交仓库。

本决策不卸载开始菜单旧安装，不删除或迁移数据库、Key、许可、素材，不改变 bootstrap-v43 数据根与凭据 scope。后续开发只维护新 UI。历史报告保持原样，不能据旧验收状态授予 V4.8 通过。

验证范围和遗留工程审计缺口见 ../reports/2026-10-24-retire-legacy-ui.md。
