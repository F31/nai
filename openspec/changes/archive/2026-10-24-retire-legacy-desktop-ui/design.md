# Design
apps/workbench-desktop拥有唯一在用前端（ui/*、index.html、package.json、tsconfig.json、Vite），保留React/ReactDOM去重，native窗口/devUrl使用index.html。pnpm importer改为新包，同一能力不复制第三套页面；旧apps/desktop源码和src-tauri退出workspace。

移除前保存旧目录源码/专用脚本与测试，ZIP和逐文件SHA256清单验证后才删除源文件；暂存/未暂存Git补丁另存ignored路径。缓存/构建产物只移动至同workspace内保留目录，不递归删除未知路径。用户的独立数据库、密钥、许可、素材及旧安装不在操作域。

保留所有新线UI状态测试并改引用；旧UI专用测试退出当前门控，不以删测试宣称产品ACC通过。control/engine smoke的纯协议客户端从旧UI解耦到tests/legacy/v311，明确只服务历史后端演练。有效npm/配置消费台账撤销已退役UI接线，未知字段不得伪装已消费。历史docs/reports与archive只追溯，不批量修改历史路径。

验证新包类型/生产构建、全在用UI测试、受影响工程台账/契约检查、无旧目录构建依赖；维护窗口再构建新壳并做Windows原生启动。无业务契约变更，不手改生成代码；配置消费变更需要正常重新生成catalog并检查。
