# Design

## Context

引擎拥有唯一 Store 与共享 Session；已有审批后台、API Provider、知识发布检索、装配与观测接口。保留上一切片尚未提交的工作台改动。

## Goals / Non-Goals

交付手工问题→本地实际检索→配置 API→最终文本候选→桌面批准/拒绝。暂不合成/播放、不启动 Auto、不依赖平台 SDK、不声称生产 protected IPC。

## Decisions

1. 在 app 层增加 operator 编排；control-api 仅持有领域接缝，不依赖 provider/runtime。复用 ManualEventCommand 与 ResourceView；ACK 只确认有界任务准入，事件查询给出进行中/候选哈希/具名失败。
2. 最多一个生成任务，无无界排队；事件历史最多 200，控制命令沿用 256 条幂等账本；同 ID 不重复计费。网络等待不持 Session、Composition 或 Store 锁。
3. `--operator-config` 显式加载 schema 校验过的本地场景；`--provision` 导入操作者确认的知识并发布快照。Provider 凭据仅从指定环境变量读入 Secret；清单与日志不含凭据。HTTP 仅允许 loopback，远端须 HTTPS。
4. 在实际检索后核验 local-only 外发政策；按已配置分段预算调用已有装配器和 runtime 日志入口，把实际存活的片段序列发给 API，不另造提示装配算法。预算是操作者给定的初值，不声称已标定。
5. Provider 通过 durable Store UsageGateway reserve→dispatch→finish；没有用量保留待对账，不编造金额。错误消息只公布稳定错误码，避免供应商原文泄漏凭据。
6. 输入/证据守门在网络前，输出守门在候选入库前；播放前守门仍须等审批与媒体阶段，本轮不伪造通过。记录使用最终 display_text/speech_text 的同一个 cue 哈希。
7. 每次外发前和提交时复检 epoch、TTL、权限、知识版本；静音/接管使在途结果失效。提交短临界区按 Session→Store 锁序避免撤权与候选写入竞态，P0 媒体撤权仍不等待网络。

## Risks / Trade-offs

loopback 控制面仍是工程适配。环境凭据是显式本机运行入口，不替代后续系统密钥库配置 UI。场景导入为可信 owner 的一次性 provision，不新增未经审核的远端知识编辑口。真实账户调用与浏览器交互另记，离线集成使用明确的本地 HTTP 协议测试服务。

## Verification

真实 engine 子进程与控制客户端/HTTP 交互验证文本产生、检索证据、预算账本、审批与拒绝；控制响应迟到/重复请求/TTL/静音换代/无知识或外发禁止均不生成可批准旧候选。前端测试请求契约与状态更新，运行生成检查、匹配 Rust 测试和 full，记录实际环境结果。
