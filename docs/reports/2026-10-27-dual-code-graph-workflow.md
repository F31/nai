# Graphify + CodeGraph 联合工作流

日期：2026-10-27。用户授权按分工建议配置并使用两种工具。产品代码/数据库/Key和164ACC状态不因开发工具接入改变。

## 实施

核对发现旧Graphify为WSL `/usr/local/bin/graphify` 0.8.32，根目录graphify-out有18个10月8日中间文件/目录，无成品graph.json/报告。本轮保留旧安装与产物，另建Windows Graphify 0.9.96（Python 3.12.12）独立环境，包/解释器/缓存/输出均在E盘target-windows/tool-cache。

安装官方 `graphifyy[sql]==0.9.96`，实际环境清单保存在工具缓存。官方wheel下载的PyPI SHA256核对通过：`bbef4c79b5905f45e384a7ca49417cd6c131d4774f280037159dd6cf4c425e29`。不安装全部可选依赖、不修改其他智能体或覆盖AGENTS。

新增scripts/graphify.ps1按需入口和.graphifyignore，CodeGraph增加graphify-out显式排除。Graphify使用固定本地AST/Cargo模式、最多2提取进程，各阶段5分钟期限，保留日志并不自动重试；不启动第二套持续监听。原生报告禁用LLM命名及大型图页面。补装SQL解析后，20份SQL不再因缺依赖而缺失。

用Graphify原生接口单独提取8份当前设计/计划/ADR的Markdown结构，结合代码上下文生成独立architecture-docs.json。只含标题、显式引用和符号提及，不做LLM语义理解；来源与代码图均有SHA256。docs查询来源失配时拒绝并提示refresh-docs；代码变化需refresh。

## 实测与边界

- 一次完成的代码图：10,767节点、27,845边、359社区；Cargo 30个节点/82条依赖边。8文档提取381节点/927条结构关系，组合图11,148节点/28,307边。数字随后续工具源码/文档更新会变化，不作固定性能或完整性承诺。
- Graphify与CodeGraph都定位ReviewPage到 `apps/workbench-desktop/ui/review-page.tsx:14`，并与实际源码核对。
- Graphify查询确认 `nai-foundation-sqlite --depends_on [EXTRACTED]--> nai-live`，与当前Cargo.toml一致。
- 组合图包含设计入口到V4.8方案的EXTRACTED显式引用，并核对8份来源哈希。
- 两图所有已索引source路径中未发现构建/另一个图产物/凭据目录/退役UI；CodeGraph同步成功。历史graphify-out的18个条目仍保持10月8日时间戳，没有移动、删除或写入。
- Graphify原生报告记录模型token 0/0；本轮模型调用0。初次SQL缺依赖警告保留，安装解析器后刷新补齐；日期检查脚本第一次使用错误epoch阈值，改为明确日历日期后复核通过，未改历史文件凑结果。

不跑全量产品测试：本轮验证范围为实际工具刷新、同模块查询、排除规则、来源/失效检查。静态图可能漏边、同名歧义或过宽，不能当编译/验收结果；未经深层语义提取的文档图不授予方案覆盖或设计符合性结论。

本机证据：`target-windows/tool-cache/graphify/` 中的package-verification.json、requirements-installed.txt、refresh-*.log、joint-verification.json，以及index-nai下的图/报告/文档来源清单。CodeGraph仍通过原项目MCP和CLI使用；Graphify先采用按需CLI，减少重复工具与常驻成本。说明见 [双图工作流](../code-intelligence.md)。

交付复核：freshness-verification.json中旧来源拒绝、当前来源查询与缓存恢复三项通过；joint-verification.json中两图源码行、Cargo关系、设计引用、8文档哈希与产物隔离均通过。Graphify短词design.md曾优先命中YAML同名节点，采用完整V4.8文件名后正确返回目标，保留这一检索局限，不据此扩大结论。

OpenSpec已归档为2026-10-27-dual-code-graph-workflow，更新开发工具主规格；未升级任何产品ACC。
