# Design
使用已校验的v1.6.2 Windows x64独立包，自带Node，不引入产品常驻服务。保持E盘缓存要求；通过项目启动脚本设置TEMP、Node编译缓存、遥测环境并固定项目根。检查项目扫描忽略规则，生成目录及本机凭据不入图。
MCP只配置nai项目，不擅自修改Claude/Cursor/OpenCode等工具。使用安装包提供的MCP stdio协议，做initialize/tools/list与至少一次真实查询；不能声称当前会话立即热加载。保留CLI入口和后续开发工作约定。
