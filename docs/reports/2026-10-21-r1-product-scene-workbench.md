# 真人实物直播场景：商品事实与讲品准备（2026-10-21）

## 本轮交付

开始落实目标场景的第一张完整实施卡P0-1：新增“商品与知识”工作台，可编辑事实来源/正文、逐条明确外发许可、保存数据库草稿、通过真实引擎发布新快照并切换下次会话，提纲可填入提问框。新增事实默认local_only；已有Provider和加密Key保留，不默认更改演示知识许可。

先新增并生成ProductDraft JSON Schema/TS契约，再实现原生消费者。原生命令复用StorageRequest/StorageResult Protobuf，有界读取与数据库CAS；没有手改生成代码。配置DB迁移v2含一致备份、刷盘与SQLite事务恢复。发布在停止引擎后进行；新scope/文档/快照不覆盖旧事实。新scope发布成功但配置CAS失败时不接入旧会话。临时场景自动清理。

修正冻结知识绑定带来的真实接线缺口：重连首先静音并持久化撤权/旧审批失效，再通过带代次检查的专用存储方法更新绑定；普通绑定接口不放宽。新增可信CLI `--publish-knowledge <scene>` 不创建工作区/会话、不授予新权限、不调用Provider。

## 验证

- Windows独立桌面壳：25项lib通过，含v1一致备份/密文保留、草稿重开、并发冲突与Provider变动冲突；真实Windows宿主→引擎商品发布集成1项通过，验证草稿不发布、显式发布、配置绑定与暂存清理。
- 真实engine进程/本地协议：operator_flow 17项通过，包含新商品发布→重启→新快照问答、旧审批拒绝；现有生成/审批/TTS/取消相关回归通过。测试显式去掉代理环境，未访问付费服务。
- 存储知识发布相关13项通过；engine lib13项通过；engine/storage全目标clippy通过；Windows宿主全目标clippy通过，包含新增集成目标。
- 界面与导航/启动定向13项通过（含实际组件渲染）；前端typecheck及生产构建通过；schema --check无漂移。
- 契约注册套件首次36/37，发现新config未进入必填键消费台账；补真实消费者与三个键后，原失败的必填键消费测试1/1通过，不无意义重跑已通过的其余36项。
- 中途定位并修正拆分发布时重复建会话、SQLite整数类型、测试残留halt文件、测试工作区不匹配；Windows间歇LNK1104在无占用后同命令复跑通过。不将未执行/失败记通过。

日志：`.nai-diagnostics/product-native-tests.log`、`product-native-integration.log`、`product-native-clippy.log`、`product-engine-windows.log`；WSL `/tmp/nai-product-{integration,storage,engine,clippy,schema,ui,frontend,contract-tests,consumption}.log`。

## 尚未完成与下一任务

本轮不是整个直播场景完成：常驻输出窗口/独立音频、连续讲品编排、检查点插问与恢复、真人麦克风混音、OBS真实录制均待开发/实测。没有把这些缺仓内实现的部分写成外部Blocked。runtime已有PitchRunner执行体与kernel规则，后续复用并补真实五项恢复复检产生者，不能无条件调用ResumeRecheck::fresh。

P0-1尚缺文件导入、事实有效期、多商品目录和形象绑定，明确保留后续子任务。本轮已闭合的是“编辑→保存→发布→重连使用新知识”路径；registry的B01/O01等状态不升级。下一实施卡优先P0-2常驻音画与OBS录制，随后P0-3讲品/插问归位，禁止用提纲按钮冒充自动节目执行。

未更新安装版，未调用付费API，未运行全仓测试。

OpenSpec已归档为 `2026-10-21-r1-product-scene-workbench`；实施前/后增量严格校验、归档后desktop-shell主规格严格校验通过。最终错误资源拒绝与真实Windows发布集成1/1通过，diff空白检查通过。变更未提交。
