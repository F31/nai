# 知识热发布任务闭环（2026-10-21）

## 交付

运行中可以编辑/保存商品草稿并显式热发布，不再要求停止引擎。正常发布保持同一进程：prepare先独立静音、推进epoch并持久化撤权、准备新快照；宿主CAS保存配置和草稿并刷盘导出，再activate核对完整知识内容和当前代次，切换运行态知识绑定。旧候选、审批与迟到生成/TTS均不能继续外发。激活只允许新的人工辅助任务，不恢复自动模式。

失败分开处理：准备/保存失败保持暂停，不自动恢复旧授权；持久配置已提交但激活未确认时保守停止引擎并提示重连加载新知识，禁止静默回滚事实。离线发布仍保留为维护入口。UI清理旧审批/预览并刷新状态；已保存但未确认激活时核对数据库草稿后允许重连。

先改OpenAPI并生成消费者：新增KnowledgePublishCommand、受权限保护的POST knowledge；复用Protobuf IPC，不增加JSON媒体通路。只接受数据根内受控暂存引用，检查目录/文件类型与场景schema；新文档命名空间不可复用当前事实。prepare/activate单容量、25秒准备任务预算、固定历史上限；请求重放由共享命令账本处理，并把操作类型加入指纹，防止手工生成ACK被当作知识发布ACK。[ADR0136](../adr/0136-controlled-knowledge-hot-publication.md)明确普通冻结绑定与显式撤权边界的区别，不改变Provider或能力绑定的维护语义。

发布期间宿主不持有P0需要的长锁；存储仍由运行引擎唯一owner操作，不另起引擎争抢数据库。新知识作用域由运行态持有，生成捕获一致版本，不再读取启动时旧knowledge配置。没有改Provider密文、默认local_only或registry状态。

## 验证与证据范围

- 真实engine进程/local协议：相关operator_flow20项通过。覆盖同PID热更新、准备与激活间拒绝生成、旧审批拒绝、重复prepare、激活前缺持久化拒绝、插入静音后旧prepared失效、准备失败/在途LLM取消、跨操作重放隔离。
- control-api库50项通过；engine库13项通过；相关engine/control-api全目标clippy通过。
- Windows独立宿主lib25项通过；最终真实宿主→运行引擎集成1项通过，含运行中草稿保存、热发布、同一ControlProcess保留、数据库触发器注入保存失败、失败后HTTP生成503和显式重试恢复。原生全目标clippy通过。
- 相关界面17项通过（包括热发布历史状态解析和实际组件渲染）、最终生产构建通过；37项契约注册检查通过，schema --check无漂移。
- 仅相关模块/消费者检查，没有全仓测试、付费请求、OBS录制或真实设备音轨认证，也未更新安装版。旧已派发请求的计费语义不变。

日志：WSL `/tmp/nai-hot-{integration,api,engine,clippy,schema-check,contracts,ui,frontend}.log`；Windows `.nai-diagnostics/hot-{native-tests,native-final,native-clippy,engine-windows}.log`。过程中的Windows瞬时LNK1104同命令复跑处理；测试失败不记Passed。

过程订正：契约严格拒绝手工请求混入知识命令字段，跨操作重放测试改为分别构造两份有效命令；OpenSpec要求保留既有场景名，已保留Concurrent or running edits并明确热发布新语义。自动审批服务曾连接中断，相关调用未执行，随后同一授权操作重试成功，无遗留阻塞。

## 剩余边界

当前为全会话知识切换时暂停播报，不是逐文档精细撤销或句间无感切换。持续讲品、插问归位、常驻音画与真人麦克风仍待后续任务；不因热发布完成升级B01/O01等验收状态。操作见 [商品工作台](../product-workbench.md)。

OpenSpec已归档为 `2026-10-21-r1-knowledge-hot-publish`；增量严格校验、归档后的desktop-shell/knowledge-pipeline主规格严格校验与归档一致性检查通过。diff空白检查通过；变更未提交。
