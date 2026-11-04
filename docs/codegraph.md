# nai 本地 CodeGraph

与Graphify的联合分工、独立索引和按需刷新见 [双图工作流](code-intelligence.md)。CodeGraph日常使用，Graphify整体分析按需运行。

2026-10-27安装上游 [colbymchenry/codegraph](https://github.com/colbymchenry/codegraph) v1.6.2 Windows x64独立包。仅作为开发工具，不进入产品运行依赖。官方SHA256SUMS核对值：`7455e817ee8032666867a1badeb799b8a884c5f4fea9b444e088a02069b6deea`。

## 位置和隐私

- 程序：`target-windows/tool-cache/codegraph/v1.6.2/codegraph-win32-x64/`，自带Node。
- 图谱：`.codegraph/` 为目录联接，实际数据位于 `target-windows/tool-cache/codegraph/index-nai/`。
- 临时文件及Node编译缓存均在E盘 `target-windows/tool-cache/`。
- CLI和MCP设置 `CODEGRAPH_TELEMETRY=0`、`DO_NOT_TRACK=1`、`CODEGRAPH_NO_UPDATE_CHECK=1`；不调用LLM/TTS，不需要Key。
- `CODEGRAPH_NO_DAEMON=1`：MCP进程由客户端持有，连接期间监听变化并增量同步，不创建独立脱离客户端的常驻daemon。
- `codegraph.json` 排除生成代码、构建/依赖、诊断、智能体配置、演示和凭据文件。已退役且从磁盘删除、但尚未提交删除的旧UI/测试路径显式排除，未恢复或删除原文件。
- 索引及机器相关 `.codex/config.toml` 已忽略；未修改全局PATH或其他智能体配置。

Windows和WSL不得同时打开同一个SQLite图谱。这里配置Windows版；WSL若另装须用独立索引，不复用 `.codegraph/`。

## 使用

在项目PowerShell中：

```powershell
.\scripts\codegraph.ps1 status
.\scripts\codegraph.ps1 query ReviewPage --limit 5
.\scripts\codegraph.ps1 context "ReviewPage review_draft_job"
.\scripts\codegraph.ps1 callers ReviewPage
.\scripts\codegraph.ps1 impact ReviewPage
.\scripts\codegraph.ps1 affected adapters/storage-sqlite/src/review.rs --json
.\scripts\codegraph.ps1 sync
```

MCP为项目级 `.codex/config.toml` 中的 `codegraph_nai`。Codex配置解析已验证。v1.6.2默认公开一个只读工具 `codegraph_explore`，可按符号、文件和流程获取上下文；其他细粒度命令可从CLI执行。

当前会话不保证热加载新工具目录。重新打开Codex或在nai新建会话后加载MCP；CLI现在即可使用。若工具未出现，先检查项目受信任和 `codex mcp get codegraph_nai`，不重复安装或覆盖用户配置。

## 后续开发约定

非平凡功能改动先查询相关符号/调用链，结合契约和源码确定影响闭包。只运行相应组件及相关功能检查；图谱返回的是候选，可能过宽、漏边或受动态调用/跨语言/宏影响。不得因为图中没有依赖就跳过状态机、IPC、迁移、取消或媒体故障测试。

查不到符号时先 `sync` 并核对排除规则，再用 `rg`/直接源码阅读。生成代码和退役源码不在本图，契约变更仍须先改schema和正式生成；不以图谱结果升级ACC或替代真实Windows/设备/OBS验收。

## 安装复现

从官方v1.6.2 Release下载 `codegraph-win32-x64.zip` 与 `SHA256SUMS`，先核对哈希再解压至上述路径，不运行远程脚本管道。安装包、清单、初始化/互通日志保存在 `target-windows/tool-cache/codegraph/`；按用户缓存要求保留在E盘。改变安装版本需同时更新CLI脚本和本机MCP中的固定路径，重新校验并查询验证。
