# ADR 0022：手工事件编排链路的落位边界——编排留在 app 层，不新建核心 crate

- 日期：2026-10-09
- 状态：Accepted
- 基线：V3.6 §1.4、§17.2、§31.2；承接 ADR 0001（内核/分层）、ADR 0019（装配根）、ADR 0020/0021（引擎装配与媒体接线）
- 变更：`openspec/changes/engine-manual-event-pipeline/`

## 背景

Slice-0（V3.6 §1.4、§31.2）要求可验证的「手工事件 → 模板/LLM → 守门 → TTS API → 2D → 录制」演练闭环，
验收含 R07（决策日志哈希链校验通过 + 用桩 Provider 回放与原决策一致）。逐层构件均已实现：
`nai-kernel::evaluate::{agent,guard}`（纯决策/四阶段守门）、`nai-knowledge-core`（向量检索）、
`nai-provider-core`（HTTP 传输 + usage 记账）、`nai-media-core`、`nai-storage`（决策日志哈希链）。

但全仓检索表明**不存在任何编排部件**：`apps/engine` 此前只装配控制面与媒体演练，没有任何一处把
「事件 → 检索 → 决策 → 守门 → 生成 → 日志」串起来。结果是 R07 取不到判据，A01/R01/R03/S01 也缺少
真实进程链路证据。R07 依赖的桩回放原计划推到 W17（R1），与本项 `release_line: slice_0` 冲突。

## 决策

**编排位于 app 层（`apps/engine/src/pipeline.rs`），不新建核心 crate，不改 kernel。** 依据：

1. V3.6 §17.2 的核心 crate 清单中**没有** pipeline/orchestrator 部件；装配与编排属 composition root 职责
   （ADR 0019/0020 已确立 `apps/engine` 为装配可执行）。
2. kernel 只承载**纯决策路由与守门判定**，不得含业务策略或 I/O（ADR 0001）。把编排塞进 kernel 会引入
   kernel→runtime/knowledge/provider 依赖，破坏 `check-deps` 强制的层级方向。
3. plugin 之间禁止直接依赖；编排所需的跨层协调只能发生在 app 层。

具体落位：

1. **编排链**：归一化/去重 → 知识检索 → 智能体决策（`kernel::evaluate::agent`）→ 守门四阶段
   （`kernel::evaluate::guard`）→ 文本生成 → 决策日志落库（哈希链）。每一步在进入下一次对外调用前
   复检 session、generation epoch、值守租约与审批对象哈希。
2. **Provider 接缝**：`nai-provider-core::text` 新增 `TextGenerator` 接缝与 **rehearsal-only 确定性桩**
   （离线、可复现、无网络）。既有 HTTP 适配器与 `UsageGateway` 行为不变；接缝不绕过预算、取消或 usage 记账。
3. **检索实现**：复用 `nai-knowledge-core::VectorSpace` 的真实余弦排序，但嵌入为**精确词表索引**而非
   哈希技巧——碰撞不得把「未命中」变成「命中」，`knowledge_no_hit` 必须是可核验事实。
4. **失败顺序固定**：链路任一阶段失败依次执行「P0 静音 → 撤销输出授权（epoch 递增）→ critical 决策日志
   → 有界退出」；决策日志写入失败**不回退**已执行的静音。
5. **诚实边界**：结果为 rehearsal，显式标注 `delivery: "none"`、`real_provider: false`、`devices: false`、
   `playback_ack: false`。桩不得用于真实集成、付费调用或任何认证结论。

## 后果

- R07 取得**桩回放判据**（同一事件 + 同一确定性桩重放，比对决策日志逐项一致），S01 取得真实进程链路证据；
  **是否升级验收状态按 registry 判据逐条判定，本轮不自动升级**。
- 依赖方向不变：`check-deps` 通过（17 包），无 kernel→runtime、无 plugin→plugin。
- 残余缺口（不得掩盖）：真实付费 Provider 调用、TTS→2D→播放 ACK 段（下一轮 `engine-text-to-playback-ack`）、
  真实设备/窗口/音频、Windows 认证、protected IPC/信号停机。
