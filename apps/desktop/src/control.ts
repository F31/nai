import type { components } from '@nai/contracts';

export type RuntimeView = components['schemas']['ResourceView'];
export type CommandAck = components['schemas']['CommandAck'];
type SessionView = components['schemas']['SessionView'];
const u64 = (value: unknown): value is string => typeof value === 'string'
  && /^(0|[1-9][0-9]*)$/.test(value) && value.length <= 20 && BigInt(value) <= 18446744073709551615n;
const object = (value: unknown): value is Record<string, unknown> => value !== null && typeof value === 'object' && !Array.isArray(value);

export function parseRuntime(value: unknown): RuntimeView & { session: SessionView; device_id: string; last_command_seq: string } {
  if (!object(value) || !object(value.session) || typeof value.resource_id !== 'string'
    || typeof value.revision !== 'string' || typeof value.status !== 'string' || typeof value.safe_message !== 'string'
    || typeof value.device_id !== 'string' || !value.device_id || !u64(value.last_command_seq)) throw new Error('运行时响应结构无效');
  const s = value.session;
  if (typeof s.session_id !== 'string' || !s.session_id || s.session_id !== value.resource_id
    || !u64(s.generation_epoch) || !u64(s.state_revision)
    || !['created','preparing','ready','ready_degraded','running','draining','stopped','faulted'].includes(String(s.lifecycle))
    || !['rehearsal','assisted','auto_supervised'].includes(String(s.mode))
    || !['standby','buffering','playing','muted','device_lost','failed'].includes(String(s.media_state))) throw new Error('会话响应结构无效');
  return value as ReturnType<typeof parseRuntime>;
}

export class ControlClient {
  private readonly base: string;
  private readonly token: string;
  private readonly fetcher: typeof fetch;

  constructor(base: string, token: string, fetcher: typeof fetch = fetch) {
    const url = new URL(base);
    if (url.protocol !== 'http:' || url.hostname !== '127.0.0.1' || url.username || url.password
      || url.search || url.hash || !['','/','/api/v1','/api/v1/'].includes(url.pathname)) throw new Error('本轮仅支持显式 127.0.0.1 本机控制地址');
    if (!/^[A-Za-z0-9_-]{32,256}$/.test(token)) throw new Error('请输入有效的临时控制令牌');
    this.base = `${url.origin}/api/v1`;
    this.token = token;
    this.fetcher = fetcher;
  }

  private async request(path: string, body?: components['schemas']['SessionCommand']): Promise<unknown> {
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

  async status(): Promise<ReturnType<typeof parseRuntime>> {
    return parseRuntime(await this.request('/runtime/status'));
  }

  async command(view: ReturnType<typeof parseRuntime>, action: 'mute' | 'takeover', id: string): Promise<CommandAck> {
    const seq = BigInt(view.last_command_seq) + 1n;
    if (seq > 18446744073709551615n) throw new Error('设备命令序号已耗尽');
    const body: components['schemas']['SessionCommand'] = {
      command_id:id,session_id:view.session.session_id,device_id:view.device_id,seq:seq.toString(),
      generation_epoch:view.session.generation_epoch,expected_state_revision:view.session.state_revision,ttl_ms:4000,
    };
    const value = await this.request(`/sessions/${encodeURIComponent(body.session_id)}/${action}`,body);
    if (!object(value) || value.command_id !== id || value.executed !== true
      || !u64(value.generation_epoch) || !u64(value.state_revision)) throw new Error('命令未取得有效的控制执行 ACK');
    return value as CommandAck;
  }
}
