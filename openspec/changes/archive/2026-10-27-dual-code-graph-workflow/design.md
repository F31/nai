# Design
Graphify使用独立版本/虚拟环境，缓存/输出在target-windows/tool-cache，保留旧graphify-out。通过受控CLI明确本地AST模式，无模型网络后端。更新两个图的排除规则，避免扫描对方产物、构建/生成目录、凭据与诊断。
按需刷新并查询Graphify总体关系，CodeGraph按符号/调用图查具体位置；不自动合并两种边或要求每次双全量。源码和正式契约优先于图，EXTRACTED/INFERRED需区分，分析任务不能升级ACC。现有CodeGraph项目MCP保持其职责，Graphify先提供独立按需CLI以减少重复常驻成本。
