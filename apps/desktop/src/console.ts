// 控制台的可测纯逻辑：新鲜度判据、批准的两步确认、命令结果消息与 UI 层断言声明。
//
// 这里**不碰 DOM**。§30.1 为 UI 层指定的四条断言（审批失效、模式切换条件、静音 ACK、升级与回滚提示）
// 要能被机器检查，而渲染本身不是本轮的判据；断言在 `tests/ui`（Node + `--experimental-strip-types`）跑。
// 显式 `.ts` 扩展名：§30.1 的 UI 层由 Node 直接加载这两个模块（`--experimental-strip-types`），
// 而 Node 的 ESM 解析不做扩展名补全。对应 tsconfig 的 `allowImportingTsExtensions`。
import { asU64, type ApprovalItem, type ApprovalsView, type CommandAck, type RuntimeStatus } from './control.ts';

/** 面板载入时看到的队列快照。列表接口不返回会话代次，故代次取自与列表同一次的 status 读取。 */
export type QueueSnapshot = { epoch: string; ids: readonly string[] };

export type FreshnessReason = 'session_changed' | 'not_listed' | 'unknown_expiry' | 'expired';
export type Freshness = { fresh: true } | { fresh: false; reason: FreshnessReason };

/** 每条拒绝理由都要让操作员知道**下一步做什么**，而不是只说「失败了」。 */
export const FRESHNESS_REASON: Record<FreshnessReason, string> = {
  session_changed: '会话已换代（generation_epoch 与列出候选时不同）；请重新列出后再决定',
  not_listed: '候选已不在最新队列中（已被处理或不再有效）；请重新列出',
  unknown_expiry: '候选未给出有效期，按不可用处理；请重新列出',
  expired: '候选已过期（expires_at_utc_ms 已过）',
};

/**
 * 审批只在候选**仍然有效**时可用。判据比较「面板载入时的快照」与「动作前的实时快照」：
 * 代次变了 ⇒ 整份快照作废；候选不在新队列里 ⇒ 已被处理或不再有效；有效期过了 ⇒ 已经失效。
 * 任一不满足都**不发出请求**——把无效动作呈现为可点，本身就是错误的反馈。
 */
export function approvalFreshness(
  item: ApprovalItem, listed: QueueSnapshot, live: QueueSnapshot, nowMs: number,
): Freshness {
  if (listed.epoch !== live.epoch) return { fresh: false, reason: 'session_changed' };
  if (!live.ids.includes(item.id)) return { fresh: false, reason: 'not_listed' };
  const expiry = item.expires_at_utc_ms === undefined ? undefined : asU64(item.expires_at_utc_ms);
  if (expiry === undefined) return { fresh: false, reason: 'unknown_expiry' };
  if (expiry <= BigInt(Math.floor(nowMs))) return { fresh: false, reason: 'expired' };
  return { fresh: true };
}

/** 批准的两步确认状态机：批准是**高危方向**（放行将要播报的内容），拒绝是安全方向。 */
export type ApprovalFlow = { armed?: string };
export const IDLE_APPROVAL_FLOW: ApprovalFlow = {};

export function nextApprovalStep(
  flow: ApprovalFlow, action: 'approve' | 'reject', candidateId: string,
): { flow: ApprovalFlow; submit: boolean } {
  if (action === 'reject') return { flow: IDLE_APPROVAL_FLOW, submit: true };
  if (flow.armed === candidateId) return { flow: IDLE_APPROVAL_FLOW, submit: true };
  return { flow: { armed: candidateId }, submit: false };
}

export const HARDWARE_ACK_NOTE = '控制 ACK 仅表示控制状态已执行，未取得硬件静音/播放 ACK';

/**
 * 候选的展示文本随列表载荷下发（`ResourceItem.display_text`），供操作员核对**将要播报的内容**。
 * 身份始终是 `item.id`（候选哈希）——批准命令也只用哈希，展示文本不参与构造命令。
 */
export type CandidateText = { id: string; present: boolean; text?: string; label: string };

/**
 * 缺文本的两种来源是同一件事：载荷没有该字段，或存储行早于该字段（迁移 0006 的默认空串）。
 * 两者都必须**具名说明缺失**，绝不显示哈希冒充文本，也绝不用空串冒充「内容为空」。
 */
export const FINAL_TEXT_GAP =
  '该候选未随载荷提供展示文本（该行早于文本保存字段，或载荷未含该字段）；身份是候选哈希，请勿据哈希推断将要播报的内容。';

export function candidateText(item: ApprovalItem): CandidateText {
  const raw = item.display_text;
  if (typeof raw !== 'string' || raw.length === 0) {
    return { id: item.id, present: false, label: FINAL_TEXT_GAP };
  }
  return { id: item.id, present: true, text: raw, label: raw };
}

export const APPROVE_CONFIRM_PROMPT = `${FINAL_TEXT_GAP} 再次点击同一候选才提交批准。`;

/** 二次确认里要么给**文本本身**，要么给**具名缺失**；两者都不给就等于让操作员盲批。 */
export function approveConfirmPrompt(item: ApprovalItem): string {
  const shown = candidateText(item);
  return shown.present
    ? `将批准播报：「${shown.text}」；身份为候选哈希。再次点击同一候选才提交批准。`
    : APPROVE_CONFIRM_PROMPT;
}

export type ControlAction = 'mute' | 'takeover' | 'approve' | 'reject';

export function describeCommandResult(action: ControlAction): string {
  switch (action) {
    case 'mute': return `已执行静音命令；${HARDWARE_ACK_NOTE}`;
    case 'takeover': return `已执行接管命令；${HARDWARE_ACK_NOTE}`;
    case 'approve': return `审批已提交；${HARDWARE_ACK_NOTE}`;
    case 'reject': return '拒绝已提交；未发送播报';
  }
}

/** 审批动作的传输面。抽成结构类型是为了让编排可被 Node 直接驱动（`tests/ui` 用替身计数）。 */
export type ApprovalTransport = {
  status(): Promise<RuntimeStatus>;
  approvals(): Promise<ApprovalsView>;
  decideApproval(
    view: RuntimeStatus, item: ApprovalItem, action: 'approve' | 'reject', id: string,
  ): Promise<CommandAck>;
};

export type ApprovalOutcome = {
  kind: 'refused' | 'armed' | 'submitted';
  message: string;
  flow: ApprovalFlow;
  status: RuntimeStatus;
  list: ApprovalsView;
  live: QueueSnapshot;
};

/**
 * 审批动作的完整编排：先重读状态与队列，再判新鲜度，再走两步确认，最后才发命令。
 * 放在这里而不是 React 组件里，是为了让「失效时不发请求」成为一条**可断言**的性质——组件只把结果贴回界面。
 */
export async function runApprovalDecision(
  transport: ApprovalTransport, item: ApprovalItem, action: 'approve' | 'reject',
  listed: QueueSnapshot, flow: ApprovalFlow, commandId: string, nowMs: number,
): Promise<ApprovalOutcome> {
  const status = await transport.status();
  const list = await transport.approvals();
  const live: QueueSnapshot = { epoch: status.session.generation_epoch, ids: list.items.map((entry) => entry.id) };
  const verdict = approvalFreshness(item, listed, live, nowMs);
  if (!verdict.fresh) {
    return {
      kind: 'refused', flow: IDLE_APPROVAL_FLOW, status, list, live,
      message: `${FRESHNESS_REASON[verdict.reason]}；未发送请求。`,
    };
  }
  const step = nextApprovalStep(flow, action, item.id);
  if (!step.submit) {
    return { kind: 'armed', flow: step.flow, status, list, live, message: approveConfirmPrompt(item) };
  }
  await transport.decideApproval(status, item, action, commandId);
  const after = await transport.approvals();
  return {
    kind: 'submitted', flow: IDLE_APPROVAL_FLOW, status, list: after,
    live: { epoch: status.session.generation_epoch, ids: after.items.map((entry) => entry.id) },
    message: describeCommandResult(action),
  };
}

/** §30.1 UI 层的四条断言，顺序与设计文档一致。 */
export const UI_ASSERTION_LABELS = ['审批失效', '模式切换条件', '静音 ACK', '升级与回滚提示'] as const;
export type UiAssertionLabel = (typeof UI_ASSERTION_LABELS)[number];
export type UiAssertionId =
  | 'approval_invalidation' | 'mode_switch_conditions' | 'mute_ack_honesty' | 'upgrade_rollback_prompt';

export type UiAssertion = {
  id: UiAssertionId;
  label: UiAssertionLabel;
  status: 'implemented' | 'unimplemented';
  /** 未实现的断言必须点名缺什么前置，而不是含糊的「以后再说」。 */
  missing?: string;
};

export const UI_ASSERTIONS: readonly UiAssertion[] = [
  { id: 'approval_invalidation', label: '审批失效', status: 'implemented' },
  {
    id: 'mode_switch_conditions', label: '模式切换条件', status: 'unimplemented',
    missing: 'POST /sessions/{id}/mode 的契约已定义、控制面未实现（schemas/openapi/control.yaml:66），界面无从表达切换条件',
  },
  { id: 'mute_ack_honesty', label: '静音 ACK', status: 'implemented' },
  {
    id: 'upgrade_rollback_prompt', label: '升级与回滚提示', status: 'unimplemented',
    missing: '控制面无升级状态来源（未暴露 nai-upgrade-core 的 plan/journal），界面无从提示升级与回滚',
  },
];

export function assertionById(id: UiAssertionId): UiAssertion | undefined {
  return UI_ASSERTIONS.find((assertion) => assertion.id === id);
}
