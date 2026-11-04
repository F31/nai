# 旧 UI 清理闭环（2026-10-24）

## 实现

新前端已迁至 apps/workbench-desktop/ui，独立包 @nai/workbench-desktop 持有依赖、配置和 index.html；原生壳使用新首页。apps/desktop 已退役，旧 UI 专用安装/浏览器脚本和旧页面测试删除；底层引擎 smoke 改用 tests/legacy/v311/desktop-protocol 的纯协议夹具。当前命令为 pnpm check:frontend、pnpm check:workbench-frontend。

删除前已核验125个文件恢复快照：.nai-diagnostics/legacy-ui-retirement/source.zip，SHA256 97c236134054137921ef6b89b4e4ba60906245c82d333f5b9e5d4d7cd9167391。manifest.json 记录逐文件哈希，补丁和旧缓存另存同目录；保留未提交源码恢复能力。主仓库暂存区未主动修改。未卸载旧开始菜单程序，未修改用户数据库、Key、许可或素材，未调用付费 API。

## 本轮实测

- 在用前端 typecheck/build 与50项 UI测试通过；新包布局和产物2项通过。
- npm脚本台账34项、workbench CI契约2项通过；依赖搬迁后离线冻结安装成功。
- Windows新壳构建、原生 authenticated owner protocol=2 启动 smoke通过；live_ready=false，不能据此宣称直播可用。
- Edge使用隔离测试 profile实际加载生产 index.html，React成功挂载；没有原生桥和付费调用。原生页面人工体验仍由用户确认。新版桌面壳和 foundation 引擎已重新打开并核对进程。
- 运行/构建路径残留审查通过，仅保留产物测试中禁止 v43.html 的断言。

## 未通过及边界

历史 tier-payment 工程契约仍有5项失败：引用已退役的 engineering_checks 和旧 CI 标记；未恢复旧门槛，也不计作通过。历史配置消费审计覆盖失败，遗漏 foundation-root、model-migration-backup、workbench-owner、workbench 四个新 Schema 的登记；旧 product-draft 消费点已明确标为 legacy-ui-retired。

生成无漂移检查未完成：catalog检查通过，OpenAPI生成依赖 @redocly/openapi-core 加载 js-yaml 时 types.merge 为 undefined。此次没有改业务 Schema 或生成代码，不能把检查未执行写成无漂移通过。相关日志保存在 .nai-diagnostics/retire-ui-*.log。额外 Windows xtask lib 检查有61项完成；全诊断产物凭据扫描超过5分钟（包含保留的旧缓存）后主动终止，整组不计通过。

本轮是旧 UI 工程清理，不是 V48-04 功能交付；164项 ACC 仍 pending。下一主线仍为 V48-04 备播首页与手动纵向路径，历史审计与工具链缺口单独跟踪。

