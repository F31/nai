import { StrictMode, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { ControlClient, type ApprovalItem, type ApprovalsView, type RuntimeStatus } from './control';
import {
  APPROVE_CONFIRM_PROMPT, IDLE_APPROVAL_FLOW, UI_ASSERTIONS, describeCommandResult, runApprovalDecision,
  type ApprovalFlow, type QueueSnapshot,
} from './console';
import './style.css';

const shortHash = (id: string) => `${id.slice(0, 12)}…`;

function App() {
  const [address,setAddress] = useState('http://127.0.0.1:4318');
  const [token,setToken] = useState('');
  const [view,setView] = useState<RuntimeStatus>();
  const [approvals,setApprovals] = useState<ApprovalsView>();
  const [snapshot,setSnapshot] = useState<QueueSnapshot>();
  const [flow,setFlow] = useState<ApprovalFlow>(IDLE_APPROVAL_FLOW);
  const [message,setMessage] = useState('运行时未连接');
  const [busy,setBusy] = useState(false);
  const client = useRef<ControlClient | undefined>(undefined);
  const generation = useRef(0);
  function clear() {setView(undefined);setApprovals(undefined);setSnapshot(undefined);setFlow(IDLE_APPROVAL_FLOW);}
  async function connect() {
    const current = ++generation.current;
    client.current = undefined; clear(); setBusy(true); setMessage('正在连接');
    const credential = token; setToken('');
    try {
      const next = new ControlClient(address,credential);
      const status = await next.status();
      if (current !== generation.current) return;
      client.current = next; setView(status); setMessage('控制状态已连接');
      // 待审批是**可缺省**的一侧：engine 未配置审批后端时返回 CAPABILITY_NOT_READY，
      // 此时 status/mute/takeover 仍应可用，界面只需如实说明审批不可用。
      try {
        const list = await next.approvals();
        if (current !== generation.current) return;
        setApprovals(list);
        setSnapshot({epoch:status.session.generation_epoch,ids:list.items.map(item => item.id)});
        setMessage('控制状态已连接；待审批队列已列出');
      } catch (error) {
        if (current === generation.current) setMessage(`控制状态已连接；待审批不可用（${error instanceof Error ? error.message : '未知错误'}）`);
      }
    } catch (error) { if (current === generation.current) setMessage(error instanceof Error ? error.message : '连接失败'); }
    finally {if (current === generation.current) setBusy(false);}
  }
  async function run(action?: 'mute' | 'takeover') {
    const active = client.current;
    if (!active || (action && !view)) return;
    const current = ++generation.current; setBusy(true);
    try {
      if (action && view) await active.command(view,action,crypto.randomUUID());
      const status = await active.status();
      if (current !== generation.current) return;
      setView(status);setMessage(action ? describeCommandResult(action) : '状态已刷新');
    } catch (error) {
      if (current === generation.current) {setView(undefined);setMessage(error instanceof Error ? error.message : '请求失败');}
    } finally {if (current === generation.current) setBusy(false);}
  }
  // 批准前重读一次状态与队列：判据的两端分别是「面板载入时的快照」与「此刻的实时快照」。
  // 不满足就不发请求——服务端应该拒绝，但界面把无效动作呈现为可点本身就是错误反馈。
  async function decide(item: ApprovalItem, action: 'approve' | 'reject') {
    const active = client.current; const listed = snapshot;
    if (!active || !listed) return;
    const current = ++generation.current; setBusy(true);
    try {
      const outcome = await runApprovalDecision(active,item,action,listed,flow,crypto.randomUUID(),Date.now());
      if (current !== generation.current) return;
      setView(outcome.status);setApprovals(outcome.list);setSnapshot(outcome.live);
      setFlow(outcome.flow);setMessage(outcome.message);
    } catch (error) {
      if (current === generation.current) setMessage(error instanceof Error ? error.message : '请求失败');
    } finally {if (current === generation.current) setBusy(false);}
  }
  function disconnect() {++generation.current;client.current = undefined;clear();setToken('');setBusy(false);setMessage('已断开控制界面；未向运行时发送 Stop');}
  const capped = approvals?.status === 'approvals_limit_reached';
  return (
    <main>
      <p className="label">本机控制面 · D14 工程入口</p>
      <h1>nai</h1>
      <p>智能体结合的直播数字人系统</p>
      <section aria-labelledby="runtime-status">
        <h2 id="runtime-status">{view ? '控制状态已连接' : '运行时未连接'}</h2>
        <p role="status">{message}</p>
        <form onSubmit={event => {event.preventDefault();void connect();}}>
          <label>本机控制地址<input value={address} onChange={event => setAddress(event.target.value)} /></label>
          <label>临时令牌（仅内存）<input type="password" autoComplete="off" value={token} onChange={event => setToken(event.target.value)} /></label>
          <button type="submit" disabled={busy || !token}>连接</button>
        </form>
        <div className="actions">
          <button onClick={() => void run()} disabled={busy || !client.current}>刷新真实状态</button>
          <button className="emergency" onClick={() => void run('mute')} disabled={!view || !client.current}>立即静音（控制撤权）</button>
          <button onClick={() => void run('takeover')} disabled={!view || !client.current}>人工接管（撤权）</button>
          <button onClick={disconnect}>断开界面</button>
        </div>
        {view && <dl>
          <dt>会话</dt><dd>{view.session.session_id}</dd>
          <dt>生命周期 / 模式</dt><dd>{view.session.lifecycle} / {view.session.mode}</dd>
          <dt>媒体状态</dt><dd>{view.session.media_state}</dd>
          <dt>epoch / 状态版本</dt><dd>{view.session.generation_epoch} / {view.session.state_revision}</dd>
          <dt>能力说明</dt><dd>{view.safe_message}</dd>
        </dl>}
      </section>
      <section aria-labelledby="approvals">
        <h2 id="approvals">待审批候选</h2>
        {approvals === undefined ? <p>未连接或审批后端未配置；本界面不伪造候选。</p> : <>
          <p>{approvals.safe_message}{capped && '（已达 200 条上限，处理候选后刷新，不代表完整队列）'}</p>
          {approvals.items.length === 0 ? <p>当前没有待审批候选。</p> : <ul className="approvals">
            {approvals.items.map(item => <li key={item.id}>
              <code title={item.id}>{shortHash(item.id)}</code>
              <span>{item.status}</span>
              <span>证据 {item.evidence_count ?? '未给出'}</span>
              <span>到期 {item.expires_at_utc_ms ?? '未给出'}</span>
              <button onClick={() => void decide(item,'approve')} disabled={busy || !client.current}>
                {flow.armed === item.id ? '再次点击以确认批准' : '批准'}
              </button>
              <button onClick={() => void decide(item,'reject')} disabled={busy || !client.current}>拒绝</button>
            </li>)}
          </ul>}
          <p>{APPROVE_CONFIRM_PROMPT}</p>
          <p>动作前会重读状态与队列：候选已过期、会话已换代或已不在队列时<strong>不发送请求</strong>。</p>
        </>}
      </section>
      <section aria-labelledby="ui-layer">
        <h2 id="ui-layer">UI 层断言（§30.1）</h2>
        <ul>{UI_ASSERTIONS.map(assertion => <li key={assertion.id}>
          {assertion.label}：{assertion.status === 'implemented' ? '已实现' : `未实现——${assertion.missing}`}
        </li>)}</ul>
      </section>
      <p>本轮不提供开播、恢复自动或模式切换。真实设备、平台 SDK 和 Tauri/受保护 IPC 尚未接线。</p>
      <p>设计基线：V3.6 · 统一内核 · 按需插件 · LLM / TTS API</p>
    </main>
  );
}

const root = document.getElementById('root');
if (!root) throw new Error('Missing root element');
createRoot(root).render(<StrictMode><App /></StrictMode>);
