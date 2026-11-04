# V48-06：LLM/TTS共同源上下文与网络子任务

goal: 业务文本生成和语音准备使用同一发布源身份；父取消/期限传播到网络子调用，迟到结果不能恢复正文/音频，已派发费用仍结算。
depends_on: V48-06中文引用FAQ、ADR0176父子问题池，既有LLM/TTS私有任务与预算。
allowed_paths: schemas/workbench/foundation.proto, schemas/content/publication.schema.json, runtime-core/foundation及task_context, storage-sqlite模型/预演路径, llm-openai传输, workbench-engine任务lane, workbench-desktop/ui/api.ts, tests, docs, openspec。
contracts: FoundationResult.agent_context、SpeechPlanView.agent_context及业务工件task_context；先生成再改消费者。
migrations: 无表迁移，不改用户模型、Key及历史费用；不为旧候选补执行权威。
invariants: INV-11、INV-35、INV-40、INV-54；取消不是退款，网络子期限不能扩展父期限。
ACC: KB/AG相关局部证据，164项全部保持pending。
fixtures: 临时Store、合成测试凭据、回环HTTP服务；无真实Provider。
commands: 受影响5包clippy；storage/runtime/provider组件；模型lane与6种Windows原生用例；UI/Schema定向与前端构建；78条互通和生成无漂移。
evidence_kind: 逻辑/临时存储/实际HTTP取消/Windows真实IPC；不授真实付费调用或完整直播验收。
external_prerequisites: 更新前相关Windows程序必须不运行；本轮已只读确认，无真实供应商前置。
done_when: 同发布资料的LLM/TTS/问题池源身份相同；旧/篡改/变化来源不能派发或采用；父取消及短期限使网络退出；取消后known/unknown费用不丢、无自动重试；新引擎/壳通过启动验证。

状态：实现及定向实测完成，交付/归档结果见docs/reports/2026-10-26-v48-06-model-task-context.md。
