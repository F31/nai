# r1-generation-readiness-reasons（2026-10-20）

实机事件3f0f10d6-0d09-45e7-9f32-603a0cf2e33e对应输入heelo。当前场景仅有演示品牌一年保修知识，文档local_only=true；ASCII问题heelo无知识命中，生成在知识准入阶段即拒绝。当前为知识约束问答，普通闲聊并未实现；不能把无命中自动改成通用聊天，也不能自动翻转local_only。

原generate将检索未就绪/无命中/空正文压成CAPABILITY_NOT_READY，手工事件只保留枚举名称。现使用内部GenerationFailure保留ErrorCode及静态受控阶段说明，在既有事件display_text中输出。区分RetrievalEmpty（及完整但空证据）、检索未就绪、命中知识未获外发许可、路由未就绪、模型调用失败与空正文。不会输出原始Provider错误、响应体、用户凭据，也不改错误码/控制守门/候选提交语义。

UI说明知识约束与外发条件，接收提示改为“正在检查知识与生成条件”，避免无命中时误报正在调用API。历史旧事件不能补造原因，重启后新事件使用新说明。未变更任何外部字段/协议/数据库格式，未修改现有知识、权限或密钥。

验证：UI135/135、typecheck/build、生产Chromium导航/启动回归通过。真实engine本地协议定向4项：无命中/local_only/输出守门（扩充heelo、空正文共5场景）、正常生成/审批、截止与陈旧命令、主备替换下一请求。无命中和外发拒绝断言Provider未收到请求，空正文保留明确拒因。一次测试将RetrievalEmpty误归为未就绪，按现有枚举改正后通过；旧截止断言更新为严格错误码前缀+受控说明。Windows短暂LNK1104独立重试后通过。

无真实付费调用、无设备播放、无全量复跑，registry验收状态不变。知识管理界面的导入/编辑/发布及外发授权尚未实现，列为下一产品主线，不称外部Blocked。

引擎lib/bin clippy -D warnings通过。Windows工程包SHA256 cf68b2b288a31eaf88834c2e14b110b769621f7571adc6f2e002f39193a73a53（未签名），构建壳--smoke退出0并输出SHELL_UI_READY。2026-10-21用户确认保存退出后已更新D:\nai Engineering；维护替换前后6个用户数据文件逐一SHA256一致，GUI Subsystem=2，安装版--smoke退出0并输出SHELL_UI_READY，随后启动新版。没有修改用户知识/凭据或执行付费调用。
