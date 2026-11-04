import type { components } from '@nai/contracts';
import { parseCostEstimate, type CostEstimate } from './upgrade-wire.ts';

export type RuntimeView = components['schemas']['ResourceView'];
export type CommandAck = components['schemas']['CommandAck'];
export type ProgramView = components['schemas']['ProgramView'];
export type ProgramAction = Pick<components['schemas']['ProgramCommand'],'action'|'program_id'|'snapshot'|'prompts'|'candidate_hash'>;

export function parseProgram(value:unknown):ProgramView {
  const keys=['program_id','snapshot','generation_epoch','phase','entered','confirmed','max_attempts','request_id','candidate_hash','playback_ack'];
  if(!object(value)||Object.keys(value).some(k=>!keys.includes(k)&&k!=='last_confirmation')||keys.some(k=>!(k in value))
    || !u64(value.generation_epoch)||!['absent','ready','generating','candidate','failed','paused','done'].includes(String(value.phase))
    || !Number.isInteger(value.entered)||!Number.isInteger(value.confirmed)
    || Number(value.confirmed)<0||Number(value.entered)>4||Number(value.confirmed)>Number(value.entered)
    || value.max_attempts!==4||value.playback_ack!==false
    || typeof value.program_id!=='string'||typeof value.snapshot!=='string'
    || typeof value.request_id!=='string'||value.request_id.length>128
    || typeof value.candidate_hash!=='string'||(value.candidate_hash!==''&&!/^[a-f0-9]{64}$/.test(value.candidate_hash)))throw new Error('引擎节目响应无效');
  if(value.phase==='absent') {
    if(value.program_id||value.snapshot||value.request_id||value.candidate_hash||value.entered!==0||value.confirmed!==0)throw new Error('空节目响应不一致');
  } else if(!/^[A-Za-z0-9_-]{1,64}$/.test(value.program_id)||!value.snapshot)throw new Error('节目身份无效');
  if((value.phase==='ready'&&value.entered!==value.confirmed)
    || (value.phase==='done'&&(value.entered!==4||value.confirmed!==4))
    || (['generating','candidate'].includes(String(value.phase))&&(!value.request_id||Number(value.entered)!==Number(value.confirmed)+1))
    || (value.phase==='candidate'&&!value.candidate_hash))throw new Error('节目进度不一致');
  if('last_confirmation' in value) {
    const r=value.last_confirmation;
    const fields=['event','program_id','session_id','generation_epoch','snapshot','segment_number','candidate_hash','operator_id','device_played_samples','report_sha256','media_sha256','window_updates','confirmed_at_utc_ms','human_confirmed','playback_ack','confirmation_ref'];
    if(!object(r)||fields.some(k=>!(k in r))||Object.keys(r).some(k=>!fields.includes(k))
      ||r.event!=='program_human_confirmation'||r.human_confirmed!==true||r.playback_ack!==false
      ||typeof r.program_id!=='string'||!/^[A-Za-z0-9_-]{1,64}$/.test(r.program_id)
      ||['session_id','snapshot','operator_id'].some(k=>typeof r[k]!=='string'||!(r[k] as string).length||(r[k] as string).length>256)
      ||['candidate_hash','report_sha256','media_sha256','confirmation_ref'].some(k=>typeof r[k]!=='string'||!/^[a-f0-9]{64}$/.test(r[k] as string))
      ||['generation_epoch','device_played_samples','window_updates','confirmed_at_utc_ms'].some(k=>!u64(r[k]))
      ||r.device_played_samples==='0'||r.window_updates==='0'||!Number.isInteger(r.segment_number)||Number(r.segment_number)<1||Number(r.segment_number)>4)throw new Error('节目人工复核记录无效');
    if(value.phase!=='absent'&&(r.program_id!==value.program_id||r.snapshot!==value.snapshot||r.generation_epoch!==value.generation_epoch))throw new Error('节目人工复核身份不匹配');
  }
  return value as ProgramView;
}
type SessionView = components['schemas']['SessionView'];
/** uint64 的唯一判据：十进制、无前导零、不超界。`u64` 是它的类型谓词形态。 */
export const asU64 = (value: unknown): bigint | undefined => typeof value === 'string'
  && /^(0|[1-9][0-9]*)$/.test(value) && value.length <= 20 && BigInt(value) <= 18446744073709551615n
  ? BigInt(value) : undefined;
const u64 = (value: unknown): value is string => asU64(value) !== undefined;
const object = (value: unknown): value is Record<string, unknown> => value !== null && typeof value === 'object' && !Array.isArray(value);

export type ApprovalItem = components['schemas']['ResourceItem'];
export type ApprovalsView = {
  resource_id: string; revision: string; status: string; safe_message: string; items: ApprovalItem[];
};

export type PreviewItem = ApprovalItem & {
  sample_count?: string; frame_count?: string; audio_sha256?: string;
};
export type PreviewsView = {
  resource_id: string; revision: string; status: string; safe_message: string; items: PreviewItem[];
};

/** 预览状态是闭集；`rendered` 必须带真实样本数、帧数、音频摘要与产物引用。
 *  缺任一项的「已渲染」不能作为可核对的结果呈现。 */
export function parsePreviews(value: unknown): PreviewsView {
  if (!object(value) || typeof value.resource_id !== 'string' || !value.resource_id
    || typeof value.revision !== 'string' || typeof value.status !== 'string' || typeof value.safe_message !== 'string'
    || !Array.isArray(value.items) || value.items.length > 100) throw new Error('预览响应结构无效');
  for (const item of value.items) {
    if (!object(item) || typeof item.id !== 'string' || !/^[a-f0-9]{64}$/.test(item.id)
      || typeof item.revision !== 'string' || !['rendering','playing','rendered','submitted','failed'].includes(String(item.status))
      || typeof item.display_text !== 'string'
      || (item.encoded_contract_ref !== undefined && typeof item.encoded_contract_ref !== 'string')
      || (item.sample_count !== undefined && !u64(item.sample_count))
      || (item.frame_count !== undefined && !u64(item.frame_count))
      || (item.audio_sha256 !== undefined && (typeof item.audio_sha256 !== 'string' || !/^[a-f0-9]{64}$/.test(item.audio_sha256)))) throw new Error('预览项结构无效');
    if (['rendered','submitted'].includes(String(item.status)) && (item.sample_count === undefined || item.frame_count === undefined
      || item.audio_sha256 === undefined || !item.encoded_contract_ref)) throw new Error('已渲染预览缺少可核对字段');
  }
  return value as PreviewsView;
}

export type PreflightAxis = { name: string; status: 'passed' | 'failed' | 'not_ready'; reason: string };
export type PreflightReport = {
  generated_at_utc_ms: string; all_passed: boolean; playback_ack: boolean; note: string;
  checks: PreflightAxis[];
};
export type PreflightView = {
  resource_id: string; revision: string; status: string; safe_message: string; report: PreflightReport;
};

/** 预检报告是闭世界：轴、三态与 `playback_ack` 逐字核验。成功状态 MUST NOT 缺
 *  `playback_ack=false` 或 note，否则「已预检」会被读成「可播放」。 */
export function parsePreflight(value: unknown): PreflightView {
  if (!object(value) || typeof value.resource_id !== 'string' || !value.resource_id
    || value.revision !== 'preflight' || typeof value.status !== 'string' || typeof value.safe_message !== 'string'
    || !object(value.report)) throw new Error('预检响应结构无效');
  const r = value.report;
  if (typeof r.generated_at_utc_ms !== 'string' || !u64(r.generated_at_utc_ms)
    || typeof r.all_passed !== 'boolean' || typeof r.playback_ack !== 'boolean'
    || typeof r.note !== 'string' || !Array.isArray(r.checks) || r.checks.length > 32) throw new Error('预检报告结构无效');
  for (const c of r.checks) {
    if (!object(c) || typeof c.name !== 'string' || !c.name
      || !['passed','failed','not_ready'].includes(String(c.status))
      || typeof c.reason !== 'string') throw new Error('预检轴结构无效');
  }
  return value as PreflightView;
}

/** 待审批载荷就是契约的 `ResourceView` + `ResourceItem[]`（`schemas/openapi/control.yaml:279`）。
 *  `id` 是候选哈希：`ApprovalCommand.candidate_hash` 的 pattern 为 `^[a-f0-9]{64}$`，界面拿不到
 *  合法身份就无法构造命令，故在此**直接拒绝**，而不是让面板退化成一排点不动的按钮。 */
export function parseApprovals(value: unknown): ApprovalsView {
  if (!object(value) || typeof value.resource_id !== 'string' || !value.resource_id
    || typeof value.revision !== 'string' || typeof value.status !== 'string' || typeof value.safe_message !== 'string'
    || !Array.isArray(value.items) || value.items.length > 1024) throw new Error('待审批响应结构无效');
  for (const item of value.items) {
    if (!object(item) || typeof item.id !== 'string' || !/^[a-f0-9]{64}$/.test(item.id)
      || typeof item.revision !== 'string' || typeof item.status !== 'string'
      || (item.encoded_contract_ref !== undefined && typeof item.encoded_contract_ref !== 'string')
      || (item.display_text !== undefined && typeof item.display_text !== 'string')
      || (item.expires_at_utc_ms !== undefined && !u64(item.expires_at_utc_ms))
      || (item.evidence_count !== undefined && !u64(item.evidence_count))) throw new Error('待审批项结构无效');
  }
  return value as ApprovalsView;
}

export type RuntimeStatus = RuntimeView & { session: SessionView; device_id: string; last_command_seq: string };

export function parseRuntime(value: unknown): RuntimeStatus {
  if (!object(value) || !object(value.session) || typeof value.resource_id !== 'string'
    || typeof value.revision !== 'string' || typeof value.status !== 'string' || typeof value.safe_message !== 'string'
    || typeof value.device_id !== 'string' || !value.device_id || !u64(value.last_command_seq)) throw new Error('运行时响应结构无效');
  const s = value.session;
  if (typeof s.session_id !== 'string' || !s.session_id || s.session_id !== value.resource_id
    || !u64(s.generation_epoch) || !u64(s.state_revision)
    || !['created','preparing','ready','ready_degraded','running','draining','stopped','faulted'].includes(String(s.lifecycle))
    || !['rehearsal','assisted','auto_supervised'].includes(String(s.mode))
    || !['standby','buffering','playing','muted','device_lost','failed'].includes(String(s.media_state))) throw new Error('会话响应结构无效');
  return value as RuntimeStatus;
}

/** 升级计划：包身份 + 成本。成本载荷的解析**只有一处**（`parseCostEstimate`），
 *  这里不重复判一遍金额——两处判同一件事，迟早给出两个答案。 */
export type UpgradePlan = {
  package_id: string; package_version: string; cost: CostEstimate;
};

const PLAN_KEYS = ['package_id', 'package_version', 'cost'] as const;

/**
 * 严格解析升级计划信封：只认这三个键。
 *
 * 契约是 `additionalProperties: false`，于是「多一个 `total`」这种**看起来方便**的字段
 * 会被服务端拒绝；客户端同样 MUST NOT 默默容忍——容忍的后果是前后端对形状的理解悄悄分家。
 * 成本交给 `parseCostEstimate`，它已经拒绝缺分项、未知分项、非法金额与空原因。
 */
export function parseUpgradePlan(value: unknown): UpgradePlan {
  if (!object(value)) throw new Error('升级计划结构无效：不是对象');
  const extra = Object.keys(value).filter(
    (key) => !PLAN_KEYS.includes(key as (typeof PLAN_KEYS)[number]),
  );
  if (extra.length > 0) throw new Error(`升级计划结构无效：含未知字段 ${extra.join(',')}`);
  const missing = PLAN_KEYS.filter((key) => !(key in value));
  if (missing.length > 0) throw new Error(`升级计划结构无效：缺少字段 ${missing.join(',')}`);
  if (typeof value.package_id !== 'string' || value.package_id.length === 0) {
    throw new Error('升级计划结构无效：package_id 必须是非空字符串');
  }
  if (typeof value.package_version !== 'string' || value.package_version.length === 0) {
    throw new Error('升级计划结构无效：package_version 必须是非空字符串');
  }
  return {
    package_id: value.package_id,
    package_version: value.package_version,
    cost: parseCostEstimate(value.cost),
  };
}

/** 作业步骤的值域。与存储的 `UpgradeStep` 五个值逐字相同（控制面侧的判据把契约与枚举钉住，
 *  这里钉住呈现侧）。`JOB_STEP_LABELS` 的键集 MUST 等于它——两边各写一份，
 *  就会在「契约加了一态」时继续显示旧的五态而不自知。 */
export const JOB_STEPS = ['prepared', 'migrated', 'checked', 'committed', 'rolled_back'] as const;
export type JobStep = (typeof JOB_STEPS)[number];

/** 一条作业在 journal 里的样子。`backup_ref`/`error_code` 是可选的**缺省**，
 *  不是空串：没备份过就没有这个键，填占位值等于替 journal 编一个它没有的备份。 */
export type JobView = {
  job_id: string; kind: 'upgrade'; step: JobStep; terminal: boolean;
  from_manifest: string; to_manifest: string; backup_ref?: string; error_code?: string;
};

const JOB_KEYS = ['job_id', 'kind', 'step', 'terminal', 'from_manifest', 'to_manifest', 'backup_ref', 'error_code'] as const;
const JOB_REQUIRED = ['job_id', 'kind', 'step', 'terminal', 'from_manifest', 'to_manifest'] as const;

/**
 * 严格解析作业状态信封。
 *
 * 未知步骤 MUST 被**具名拒绝**，MUST NOT 被猜成某个已知步骤：猜出来的「进行到哪一步」
 * 比「不知道」更坏，因为它看起来像事实。
 *
 * `terminal` **不在客户端重算**。它是 journal 对步骤词汇派生的结论，客户端再算一遍就是
 * 同一件事的第二处定义——词汇一改，两处就会不一致，而分歧是静默的。这里只校验它是布尔值，
 * 然后**原样呈现服务端说的**。
 */
export function parseJobView(value: unknown): JobView {
  if (!object(value)) throw new Error('作业状态结构无效：不是对象');
  const extra = Object.keys(value).filter((key) => !JOB_KEYS.includes(key as (typeof JOB_KEYS)[number]));
  if (extra.length > 0) throw new Error(`作业状态结构无效：含未知字段 ${extra.join(',')}`);
  const missing = JOB_REQUIRED.filter((key) => !(key in value));
  if (missing.length > 0) throw new Error(`作业状态结构无效：缺少字段 ${missing.join(',')}`);
  if (typeof value.job_id !== 'string' || value.job_id.length === 0) {
    throw new Error('作业状态结构无效：job_id 必须是非空字符串');
  }
  if (value.kind !== 'upgrade') throw new Error(`作业状态结构无效：未知作业类别 ${String(value.kind)}`);
  const step = JOB_STEPS.find((candidate) => candidate === value.step);
  if (step === undefined) throw new Error(`作业状态结构无效：未知步骤 ${String(value.step)}`);
  if (typeof value.terminal !== 'boolean') throw new Error('作业状态结构无效：terminal 必须是布尔值');
  for (const key of ['from_manifest', 'to_manifest'] as const) {
    if (typeof value[key] !== 'string' || value[key].length === 0) {
      throw new Error(`作业状态结构无效：${key} 必须是非空字符串`);
    }
  }
  for (const key of ['backup_ref', 'error_code'] as const) {
    if (value[key] !== undefined && (typeof value[key] !== 'string' || value[key].length === 0)) {
      throw new Error(`作业状态结构无效：${key} 若出现必须是非空字符串`);
    }
  }
  return value as JobView;
}

/**
 * Requests go through exactly one of two adaptations: the explicit loopback
 * HTTP test client, or the native host's private channel. The native path has
 * no address or token because the WebView must not hold either.
 */
export interface ControlTransport {
  request(path: string, body?: unknown): Promise<unknown>;
}

export class ControlClient {
  private readonly base: string;
  private readonly token: string;
  private readonly fetcher: typeof fetch;
  private readonly transport?: ControlTransport;

  constructor(base: string | ControlTransport, token?: string, fetcher: typeof fetch = fetch) {
    this.fetcher = fetcher;
    if (typeof base !== 'string') {
      this.transport = base;
      this.base = '';
      this.token = '';
      return;
    }
    const url = new URL(base);
    if (url.protocol !== 'http:' || url.hostname !== '127.0.0.1' || url.username || url.password
      || url.search || url.hash || !['','/','/api/v1','/api/v1/'].includes(url.pathname)) throw new Error('本轮仅支持显式 127.0.0.1 本机控制地址');
    if (typeof token !== 'string' || !/^[A-Za-z0-9_-]{32,256}$/.test(token)) throw new Error('请输入有效的临时控制令牌');
    this.base = `${url.origin}/api/v1`;
    this.token = token;
  }

  private async request(path: string, body?: unknown): Promise<unknown> {
    if (this.transport) return this.transport.request(path, body);
    const response = await this.fetcher(`${this.base}${path}`, {
      method: body ? 'POST' : 'GET', cache: 'no-store', credentials: 'omit', redirect: 'error',
      headers: { Authorization: `Bearer ${this.token}`, ...(body ? { 'Content-Type': 'application/json' } : {}) },
      body: body ? JSON.stringify(body) : undefined, signal: AbortSignal.timeout(4000),
    });
    const reader = response.body?.getReader();
    if (!reader) throw new Error('控制服务未返回响应');
    const chunks: Uint8Array[] = []; let length = 0;
    try {
      for (;;) {
        const {value,done} = await reader.read();
        if (done) break;
        length += value.length;
        if (length > 65536) { await reader.cancel(); throw new Error('控制响应超过上限'); }
        chunks.push(value);
      }
    } finally { reader.releaseLock(); }
    const bytes = new Uint8Array(length); let offset = 0;
    for (const chunk of chunks) {bytes.set(chunk,offset);offset += chunk.length;}
    const value: unknown = JSON.parse(new TextDecoder().decode(bytes));
    if (!response.ok) {
      const code = object(value) && object(value.error) && typeof value.error.code === 'string' ? value.error.code : 'UNKNOWN';
      throw new Error(`控制请求失败（${response.status} / ${code}）；请刷新真实状态，未自动重试`);
    }
    return value;
  }

  async status(): Promise<RuntimeStatus> {
    return parseRuntime(await this.request('/runtime/status'));
  }

  /** 命令序号由真实视图单调推进；耗尽即拒绝，不静默回绕。 */
  private nextSeq(view: RuntimeStatus): string {
    const seq = BigInt(view.last_command_seq) + 1n;
    if (seq > 18446744073709551615n) throw new Error('设备命令序号已耗尽');
    return seq.toString();
  }

  /** ACK 只在 `executed === true` 且 id/epoch/revision 自洽时被接受；false 或结构异常一律算失败。 */
  private parseAck(value: unknown, id: string): CommandAck {
    if (!object(value) || value.command_id !== id || value.executed !== true
      || !u64(value.generation_epoch) || !u64(value.state_revision)) throw new Error('命令未取得有效的控制执行 ACK');
    return value as CommandAck;
  }

  async command(view: RuntimeStatus, action: 'mute' | 'takeover', id: string): Promise<CommandAck> {
    const body: components['schemas']['SessionCommand'] = {
      command_id:id,session_id:view.session.session_id,device_id:view.device_id,seq:this.nextSeq(view),
      generation_epoch:view.session.generation_epoch,expected_state_revision:view.session.state_revision,ttl_ms:4000,
    };
    const value = await this.request(`/sessions/${encodeURIComponent(body.session_id)}/${action}`,body);
    return this.parseAck(value,id);
  }

  async approvals(): Promise<ApprovalsView> {
    return parseApprovals(await this.request('/approvals'));
  }

  /** 预览一个已批准候选。身份/批准范围都由候选哈希表达，正文不随命令发送。 */
  async preview(view: RuntimeStatus, candidateHash: string, id: string, localPlayback = false): Promise<CommandAck> {
    if (!/^[a-f0-9]{64}$/.test(candidateHash)) throw new Error('预览候选哈希无效');
    const body: components['schemas']['PreviewCommand'] = {
      command_id:id,session_id:view.session.session_id,device_id:view.device_id,seq:this.nextSeq(view),
      generation_epoch:view.session.generation_epoch,expected_state_revision:view.session.state_revision,ttl_ms:localPlayback ? 30000 : 4000,
      candidate_hash:candidateHash,hash_algorithm:'sha256-canonical-v1',
      local_playback:localPlayback,
    };
    const value = await this.request(`/sessions/${encodeURIComponent(body.session_id)}/previews`,body);
    return this.parseAck(value,id);
  }

  async previews(view: RuntimeStatus): Promise<PreviewsView> {
    return parsePreviews(await this.request(`/sessions/${encodeURIComponent(view.session.session_id)}/previews`));
  }

  /** 读取最近一次一键预检报告；`CAPABILITY_NOT_READY` 表示尚未预检。 */
  async preflight(view: RuntimeStatus): Promise<PreflightView> {
    return parsePreflight(await this.request(`/sessions/${encodeURIComponent(view.session.session_id)}/preflight`));
  }

  async outputPrewarm(view:RuntimeStatus,id:string):Promise<CommandAck> {
    const body:components['schemas']['SessionCommand']={command_id:id,session_id:view.session.session_id,device_id:view.device_id,seq:this.nextSeq(view),generation_epoch:view.session.generation_epoch,expected_state_revision:view.session.state_revision,ttl_ms:10000};
    return this.parseAck(await this.request(`/sessions/${encodeURIComponent(body.session_id)}/output-prewarm`,body),id);
  }

  async manualEvent(view: RuntimeStatus, text: string, id: string): Promise<CommandAck> {
    if (!text.trim() || text.length > 4096) throw new Error('请输入不超过4096字符的问题');
    const body: components['schemas']['ManualEventCommand'] = {
      command_id:id,session_id:view.session.session_id,device_id:view.device_id,seq:this.nextSeq(view),
      generation_epoch:view.session.generation_epoch,expected_state_revision:view.session.state_revision,
      ttl_ms:30000,source:'manual',text:text.trim(),
    };
    if (new TextEncoder().encode(JSON.stringify(body)).length > 4096) throw new Error('问题与命令元数据超过4096字节，请缩短问题');
    return this.parseAck(await this.request(`/sessions/${encodeURIComponent(body.session_id)}/manual-events`,body),id);
  }

  async program(view:RuntimeStatus):Promise<ProgramView> {
    return parseProgram(await this.request(`/sessions/${encodeURIComponent(view.session.session_id)}/program`));
  }
  async programCommand(view:RuntimeStatus,action:ProgramAction,id:string):Promise<CommandAck> {
    if(!/^[A-Za-z0-9_-]{1,64}$/.test(action.program_id)||!action.snapshot)throw new Error('节目身份无效');
    if(action.action==='start'&&(!action.prompts||action.prompts.length!==4||action.prompts.some(p=>!p.trim()||Array.from(p).length>512)))throw new Error('需要四段非空提纲，每段最多512字');
    if(action.action==='confirm'&&!/^[a-f0-9]{64}$/.test(action.candidate_hash??''))throw new Error('确认须绑定本段精确候选');
    const body:components['schemas']['ProgramCommand']={...action,command_id:id,session_id:view.session.session_id,device_id:view.device_id,seq:this.nextSeq(view),generation_epoch:view.session.generation_epoch,expected_state_revision:view.session.state_revision,ttl_ms:30000};
    if(new TextEncoder().encode(JSON.stringify(body)).length>4096)throw new Error('四段提纲与元数据超过4096字节，请缩短后发布');
    return this.parseAck(await this.request(`/sessions/${encodeURIComponent(body.session_id)}/program`,body),id);
  }

  async manualEvents(view: RuntimeStatus): Promise<ApprovalsView> {
    const value = await this.request(`/sessions/${encodeURIComponent(view.session.session_id)}/events`);
    if (!object(value) || value.resource_id !== view.session.session_id || typeof value.revision !== 'string'
      || typeof value.status !== 'string' || typeof value.safe_message !== 'string'
      || !Array.isArray(value.items) || value.items.length > 200) throw new Error('手工事件响应结构无效');
    for (const item of value.items) {
      if (!object(item) || typeof item.id !== 'string' || !item.id || typeof item.revision !== 'string'
        || !['generating','awaiting_approval','failed','knowledge_preparing','knowledge_prepared','knowledge_activated','output_preparing','output_ready'].includes(String(item.status))
        || typeof item.display_text !== 'string' || typeof item.encoded_contract_ref !== 'string'
        || (['awaiting_approval','output_ready'].includes(String(item.status)) && !/^[a-f0-9]{64}$/.test(item.encoded_contract_ref))) throw new Error('手工事件项结构无效');
    }
    return value as ApprovalsView;
  }

  /**
   * 升级成本：把**包清单**交给控制面去算。
   *
   * 前端不算成本：四类分项的算法只有一处（`nai-upgrade-core::cost`），
   * 在这里重算一遍会让「同一条清单两个答案」。本路径**不验签**，
   * 界面必须同时呈现 `COST_DECLARED_BASIS_NOTE`。
   */
  async planUpgrade(manifest: unknown): Promise<UpgradePlan> {
    return parseUpgradePlan(await this.request('/upgrades/plan', { manifest }));
  }

  /**
   * 作业状态：读的是控制面从 `upgrade_journal` 取出的当前行。
   *
   * 三种结果由状态码分开，客户端**不猜**：200 有记录、404 该 id 不命名任何事务、
   * 503 作业面根本没接。把它们合并成「查询失败」会让运维分不清「记录丢了」与「功能没上」。
   */
  async fetchJob(jobId: string): Promise<JobView> {
    if (typeof jobId !== 'string' || jobId.length === 0) throw new Error('作业 id 必须是非空字符串');
    return parseJobView(await this.request(`/jobs/${encodeURIComponent(jobId)}`));
  }

  /** 审批与命令共用同一 `SessionCommand` 骨架（`ApprovalCommand = SessionCommand & {candidate_hash, hash_algorithm}`）。 */
  async decideApproval(
    view: RuntimeStatus, item: ApprovalItem, action: 'approve' | 'reject', id: string,
  ): Promise<CommandAck> {
    const body: components['schemas']['ApprovalCommand'] = {
      command_id:id,session_id:view.session.session_id,device_id:view.device_id,seq:this.nextSeq(view),
      generation_epoch:view.session.generation_epoch,expected_state_revision:view.session.state_revision,ttl_ms:30000,
      candidate_hash:item.id,hash_algorithm:'sha256-canonical-v1',
    };
    const value = await this.request(`/approvals/${encodeURIComponent(item.id)}/${action}`,body);
    return this.parseAck(value,id);
  }
}

