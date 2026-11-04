# CodeGraph 本地开发工具接入

日期：2026-10-27。用户明确要求安装并用于后续代码开发。

## 交付

官方v1.6.2 Windows x64独立包安装到E盘，核对官方SHA256SUMS成功。未执行远程安装脚本、未写入产品依赖、未修改全局PATH或其他开发工具。新增项目CLI入口 `scripts/codegraph.ps1`、扫描规则 `codegraph.json` 和仅本机项目级MCP配置；未覆盖既有AGENTS，另补简短开发约定。

索引目录通过 `.codegraph` 联接到 `target-windows/tool-cache/codegraph/index-nai`。遥测、自动更新检查关闭，MCP直接进程模式由客户端管理；连接期间增量同步，无脱离客户端daemon。全量产品测试没有执行：本轮只涉及开发工具安装和配置。

## 实测

- 官方归档SHA256：`7455e817ee8032666867a1badeb799b8a884c5f4fea9b444e088a02069b6deea`；CLI版本1.6.2。
- 最终首次项目索引：616文件、10,682节点、39,887边，约3.8秒，读取/解析错误0，SQLite约52.35MB。后续增量数字会自然变化。
- 初始59个ENOENT均为已删除未提交的退役UI/测试/脚本路径，明确排除后重建。保留初始日志，没有恢复旧UI或修改Git索引。
- CLI真实查询 `ReviewPage` 返回 `apps/workbench-desktop/ui/review-page.tsx`；`affected` 返回候选测试，实测可能较宽，不能直接当完整或最小测试集。
- `codex mcp get codegraph_nai` 解析成功、enabled=true、stdio、项目cwd与E盘路径正确。
- MCP协议2024-11-05 initialize及tools/list通过。该版默认公开只读 `codegraph_explore`（并非所有CLI子命令分别作为MCP工具）。
- MCP实际查询返回nai源码。新增独立临时TS夹具后被增量索引，删除后查询不再返回；夹具已清理。日志记录新增同步约6309ms、删除约3ms，不据此承诺固定延迟。
- `mcp-verification.json` 明确sourceQuery、incrementalAdd、incrementalRemove=true，telemetry及detachedDaemon=false。

可复核的本机证据位于 `target-windows/tool-cache/codegraph/` 的SHA256SUMS、index-final.log、mcp-verification.json、mcp-stderr.log。客户端CLI曾提示无法创建其C盘临时别名，此为已有Codex进程环境；项目CodeGraph程序/索引/编译缓存/临时文件均在E盘，不清理Codex工具链或用户数据。

## 边界和后续

已安装且CLI立即可用；当前会话不宣称新MCP工具已热加载，重新打开Codex或新建nai会话后使用。后续开发结合调用图与源码缩小检查范围，不将静态图的缺边当作无影响，不扩大实际产品认证状态。164项ACC保持不变，产品下一主线仍为V48-08问题池处理结果关联及第二场真实复用。

详见 [使用说明](../codegraph.md)。

OpenSpec增量已归档为2026-10-27-codegraph-local-tooling。
新主规格strict通过，264份OpenSpec归档校验通过。
