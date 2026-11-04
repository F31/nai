## Why
新备播工作台仍借用旧desktop目录的前端和依赖，导致旧UI无法安全退役且构建门槛仍混入旧页面。需要完成单一前端归属和旧入口清理，并保留用户未提交成果的可恢复快照。
## What Changes
- 将在用新前端迁至workbench-desktop/ui，独立package/tsconfig/Vite和index.html。
- 删除旧UI与旧Tauri壳专用源码、测试、打包/浏览器fixture脚本，清理有效工程引用。
- 底层引擎smoke仅使用明确的legacy测试客户端，不保留旧UI运行入口。
- 调整npm脚本/消费/测试可达性台账，验证新UI独立构建和新桌面启动。
## Impact
不删除或卸载已安装旧程序，不修改数据库/Key/许可/素材。旧未提交源码先保存至ignored本地恢复快照并校验；旧构建缓存移动至ignored保留目录。历史报告/验收语料不改写为当前通过。
