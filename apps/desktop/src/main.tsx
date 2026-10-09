import { StrictMode, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { ControlClient, type parseRuntime } from './control';
import './style.css';

function App() {
  const [address,setAddress] = useState('http://127.0.0.1:4318');
  const [token,setToken] = useState('');
  const [view,setView] = useState<ReturnType<typeof parseRuntime>>();
  const [message,setMessage] = useState('运行时未连接');
  const [busy,setBusy] = useState(false);
  const client = useRef<ControlClient | undefined>(undefined);
  const generation = useRef(0);
  async function connect() {
    const current = ++generation.current;
    client.current = undefined; setView(undefined); setBusy(true); setMessage('正在连接');
    const credential = token; setToken('');
    try {
      const next = new ControlClient(address,credential);
      const status = await next.status();
      if (current !== generation.current) return;
      client.current = next; setView(status); setMessage('控制状态已连接');
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
      setView(status);setMessage(action ? '控制状态已执行；未取得硬件 ACK' : '状态已刷新');
    } catch (error) {
      if (current === generation.current) {setView(undefined);setMessage(error instanceof Error ? error.message : '请求失败');}
    } finally {if (current === generation.current) setBusy(false);}
  }
  function disconnect() {++generation.current;client.current = undefined;setView(undefined);setToken('');setBusy(false);setMessage('已断开控制界面；未向运行时发送 Stop');}
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
        <p>本轮不提供开播、恢复自动或审批按钮。真实设备、平台 SDK 和 Tauri/受保护 IPC 尚未接线。</p>
        <p>设计基线：V3.5 · 统一内核 · 按需插件 · LLM / TTS API</p>
      </section>
    </main>
  );
}

const root = document.getElementById('root');
if (!root) throw new Error('Missing root element');
createRoot(root).render(<StrictMode><App /></StrictMode>);
