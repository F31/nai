# Graphify 与 CodeGraph 协同开发

这两个开发工具已独立配置。CodeGraph v1.6.2用于日常符号、调用链和改动影响查询；Graphify v0.9.96按需查看代码整体结构、Cargo依赖、SQL结构和当前设计文档的显式引用。后者不另开持续监听，不要求每次编码都做双全量分析。

## 实际用法

### 开始一条主线或做架构调整

```powershell
.\scripts\graphify.ps1 status
.\scripts\graphify.ps1 refresh
.\scripts\graphify.ps1 docs "digital-host-agent-design-V4.8" --budget 1200
.\scripts\graphify.ps1 path nai-foundation-sqlite nai-live
.\scripts\graphify.ps1 query ReviewPage --budget 900
```

`refresh` 使用固定本地 `--code-only --cargo --max-workers 2 --no-dedup`，再执行 `cluster-only --no-label --no-viz`。每阶段最多300秒，失败不自动重试。SQL解析可选依赖已安装。`report` 显示Graphify原生结构报告，不生成大图页面。

另用Graphify原生Markdown提取/解析/构建/导出接口，为8份指定的当前设计入口、V4.8方案、计划、架构边界和相关ADR/报告生成独立 `architecture-docs.json`。只提取标题、显式链接及代码符号提及，按原生规则保留EXTRACTED/INFERRED区别，不做LLM语义推断。

仅这些文档变化时运行 `refresh-docs`，无需重新提取代码。`docs` 查询会核对8份来源及代码图的SHA256；不匹配则拒绝查询并提示刷新，避免把旧文档结构当作当前证据。代码也发生变化时先 `refresh`。

### 日常编码与选定向测试

```powershell
.\scripts\codegraph.ps1 query ReviewPage --limit 5
.\scripts\codegraph.ps1 callers ReviewPage
.\scripts\codegraph.ps1 impact ReviewPage
.\scripts\codegraph.ps1 affected adapters/storage-sqlite/src/review.rs --json
```

CodeGraph项目MCP `codegraph_nai` 在客户端连接期间增量同步；当前会话若未加载工具，可直接使用上述CLI。MCP配置和已验证限制见 [CodeGraph说明](codegraph.md)。

分析结果指向源码后，继续核对正式schema、调用边界及现有测试，按影响闭包验证。Graphify做总体定位，CodeGraph查具体符号；小修改通常只需后者。两者不一致时核对当前源码、版本与排除规则；不把图合并为“唯一事实”，不因缺边跳过IPC、迁移、取消、媒体故障测试，也不机械执行候选中的全部测试。

## 安装与数据位置

Graphify的独立Python 3.12.12、虚拟环境和uv缓存均在E盘 `target-windows/tool-cache`：

- `graphify/venv-0.9.96/`：固定 `graphifyy[sql]==0.9.96`。
- `graphify/index-nai/`：代码图、报告、文档结构图、缓存和来源哈希。
- `codegraph/index-nai/`：另一份独立SQLite索引，由根目录 `.codegraph` 联接。
- `uv-python/`、`uv-cache/`：本地开发分析工具链和包缓存。

复现安装时先执行 `scripts/use-project-build-cache.ps1`，将 `UV_CACHE_DIR`、`UV_PYTHON_INSTALL_DIR` 设置到上述E盘路径，再创建虚拟环境并安装固定包。Graphify官方包名是 `graphifyy`；使用PyPI官方包，不运行其批量智能体安装器覆盖本项目说明。

WSL中旧0.8.32及根目录 `graphify-out` 的历史中间文件保留，日常入口不使用它们。不跨Windows/WSL共享SQLite。本次Python是用户明确要求的开发分析工具，不成为nai产品运行依赖。

两个工具互相排除产物，并排除构建、依赖、生成代码、凭据、诊断和退役UI。CodeGraph遥测关闭；Graphify入口固定本地提取并关闭本机查询日志。模型调用0。深层设计语义提取仍需另行配置受控后端，当前不会自动把文档发送给模型。
