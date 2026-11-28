import { useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { Event, Kind } from '../../../packages/contracts-ts/src/generated/v561/live_session';
import './v561-voice.css';
type Native = {
    core: {
        invoke<T>(name: string, args?: Record<string, unknown>): Promise<T>;
    };
};
const native = (window as Window & {
    __TAURI__?: Native;
}).__TAURI__;
const idle = Event.create({ version: 1, kind: Kind.KIND_UNPREPARED });
const errors: Record<string, string> = { LLM_ANSWER_TRUNCATED: '文本模型响应被截断，本次未播放或恢复；未自动重试。', LLM_NOT_CONFIGURED: '请在音画配置中设置文本模型后再提问。', LLM_CREDENTIAL_UNAVAILABLE: '无法读取文本模型凭据，请检查系统凭据配置。', LLM_AUTH_FAILED: '文本模型认证失败，请检查密钥。', LLM_PAYMENT_REQUIRED: '文本模型余额不足或需要付费。', LLM_RATE_LIMITED: '文本模型请求受限，请稍后再试；未自动重试。', LLM_TIMEOUT: '文本回答超时，本次未播放或恢复；未自动重试。', LLM_ANSWER_INVALID: '文本回答格式或长度不符合要求，本次未播放；未自动重试。', LLM_INPUT_TOO_LARGE: '原稿与问题过长，请缩短后重新准备。', LLM_REQUEST_FAILED: '文本模型请求失败，本次未播放或恢复；未自动重试。', REPLY_NOT_AVAILABLE: '请在播放已备内容时提问。', VISUAL_UNAVAILABLE: '数字人进程已断开，公共画面已清空；请重新准备。', CONTENT_NOT_PREPARED: '已备内容不匹配或已过期，请重新准备；未自动生成。', CONTENT_PREPARATION_FAILED: '内容准备失败；首版请使用一个简短完整句，检查语音服务与缓存容量后重试。', PREPARED_PLAY_FAILED: '已备内容播放未完成；已停止，不会自动重试。', CONFIG_REQUIRED: '请在设置中选择本机音画配置。', CONFIG_INVALID: '音画配置无效，请检查所选文件。', PROGRAMS_MISSING: '缺少音画控制或音频程序，请使用完整开发组合。', NOT_PREPARED: '请先准备数字人。', BUSY: '正在处理，请稍候；需要中断可点击停止。', TEXT_INVALID: '请输入1—1024个字符。', PREPARE_FAILED: '准备未完成，请检查本机模型、形象和运行环境。', CUDA_PREPARATION_FAILED: 'CUDA 模型预热失败，数字人尚未就绪；请检查运行日志后重新准备，未自动重试。', CUDA_MEMORY_EXHAUSTED: '显存不足，数字人准备失败；请关闭其他占用显存的程序后重试。', HOST_MEMORY_EXHAUSTED: '系统内存或页面文件空间不足，数字人准备失败；请释放内存后重新准备。', TTS_PREPARATION_FAILED: '语音预热失败，请检查本地语音服务和参考音频后重新准备；未自动重试。', PREPARATION_CANCELLED: '数字人准备已停止。', MODEL_PREPARATION_FAILED: '模型加载或预热失败，请检查模型文件和运行环境。', PREPARATION_TIMEOUT: '数字人准备超时，尚未就绪；未自动重试。', PLAY_FAILED: '本段音画播放未完成，请检查语音服务和设备；未自动重试。', SESSION_EXITED: '音画会话已退出，请检查配置后重新选择；不会自动恢复。' };
errors.OBS_NOT_CONFIGURED = '请先选择包含独立 OBS 的本机音画配置。';
errors.DEFAULT_ASSETS_MISSING = '默认配置所需模型或素材缺失，请检查本机装配目录。';
errors.PYTHON_MISSING = '未找到本机 Python 解释器，无法启动形象 worker。';
errors.MODEL_MISSING = '默认 FlashHead 模型缺失，未生成配置。';
errors.WAV2VEC_MISSING = '默认 wav2vec 模型缺失，未生成配置。';
errors.PORTRAIT_MISSING = '默认授权肖像缺失，未生成配置。';
errors.WORKER_MISSING = '默认形象 worker 脚本缺失，未生成配置。';
errors.WORKER_DEPS_MISSING = '默认 worker 依赖目录缺失，未生成配置。';
errors.CONTRACTS_MISSING = '默认契约生成物缺失，未生成配置。';
errors.DEFAULT_WRITE_FAILED = '默认配置写入失败，请检查运行时目录权限。';
errors.OBS_NOT_READY = '未收到本次隔离 OBS 的有效启录回执，请检查接收端后重试。';
errors.OBS_REPEAT_LIMIT = '本轮 OBS 播放超过场次句数或语音时长上限，请缩短讲稿或减少播放次数。';
errors.OUTPUT_WINDOW_UNAVAILABLE = '公共输出窗口已最小化或没有有效显示区域，音画任务已停止。请恢复窗口后重新准备内容并播放；不会自动恢复。';
export function message(value: unknown) { return errors[String(value)] ?? '操作未完成，请检查本机服务和音画配置。'; }
export function prepared(view: Pick<Event, 'visualPrepared' | 'ttsPrepared'>) { return view.visualPrepared && view.ttsPrepared; }
export function repeatDecision(after: bigint | null, next: Event) {
    if (after === null || next.busy) return 'wait';
    if (!next.contentPrepared || !prepared(next) || next.kind === Kind.KIND_FAILED || next.kind === Kind.KIND_STOPPED) return 'stop';
    if (BigInt(next.requestSequence) <= after) return 'wait';
    return next.kind === Kind.KIND_COMPLETED && ['COMPLETED', 'REPLY_COMPLETED'].includes(next.code) ? 'play' : 'stop';
}
export function localGenerationActive(view: Pick<Event, 'kind' | 'code' | 'busy'>) {
    return view.busy && view.kind === Kind.KIND_STARTING && ['PLAYING', 'GENERATING_ANSWER'].includes(view.code);
}
export function elapsedSeconds(started: number, now: number) {
    return Number.isFinite(started) && Number.isFinite(now) ? Math.floor(Math.max(0, now - started) / 1000) : 0;
}
const monotonicNow = () => performance.now();
export function LocalElapsed({ now = monotonicNow }: { now?: () => number }) {
    const started = useRef<number | null>(null);
    if (started.current === null) started.current = now();
    const [seconds, setSeconds] = useState(0);
    useEffect(() => {
        const timer = setInterval(() => setSeconds(elapsedSeconds(started.current!, now())), 500);
        return () => clearInterval(timer);
    }, [now]);
    return <p aria-live="off">本次已用 {seconds} 秒（含生成与播放），不是剩余时间。需要结束可随时点击停止。</p>;
}
export function Voice() {
    const [view, setView] = useState(idle), [text, setText] = useState('你好，这是数字人直播。'), [error, setError] = useState(''), [settings, setSettings] = useState(false), [choosing, setChoosing] = useState(false), [preparedText, setPreparedText] = useState(''), [question, setQuestion] = useState('');
    const alive = useRef(true), polling = useRef(false), ticket = useRef(0);
    const [obs, setObs] = useState(false);
    const [repeat, setRepeat] = useState(false), [looping, setLooping] = useState(false);
    const loop = useRef<{ text: string; after: bigint } | null>(null);
    function endLoop() { loop.current = null; setLooping(false); }
    async function refresh() { if (!native || polling.current)
        return; polling.current = true; const own = ticket.current; try {
        const bytes = await native.core.invoke<number[]>('v561_live_status');
        const next = Event.decode(Uint8Array.from(bytes));
        if (next.version !== 1 || next.kind < 1 || next.kind > 10 || next.code.length > 64)
            throw Error('PROTOCOL');
        if (alive.current && own === ticket.current) {
            setView(next);
            const current = loop.current;
            const decision = repeatDecision(current?.after ?? null, next);
            if (decision === 'stop') {
                endLoop();
            } else if (current && decision === 'play') {
                current.after = BigInt(next.requestSequence);
                void act('v561_live_play_prepared', { text: current.text });
            }
        }
    }
    catch (e) {
        if (alive.current && own === ticket.current) {
            endLoop();
            setError(message(e));
        }
    }
    finally {
        polling.current = false;
    } }
    useEffect(() => { alive.current = true; void refresh(); const timer = setInterval(() => void refresh(), 200); return () => { alive.current = false; clearInterval(timer); void native?.core.invoke('v561_live_stop').catch(() => { }); }; }, []);
    async function act(name: string, args?: Record<string, unknown>) { if (!native)
        return;
        if (name !== 'v561_live_play_prepared' && name !== 'v561_live_ask') endLoop();
        const own = ++ticket.current;
        setError(''); try {
        await native.core.invoke(name, args);
    }
    catch (e) {
        if (alive.current && own === ticket.current) {
            endLoop();
            setError(message(e));
        }
    }
    finally {
        await refresh();
    } }
    async function choose() { setChoosing(true); setView(idle); await act('v561_live_config'); setChoosing(false); }
    async function useDefault() { setChoosing(true); setView(idle); await act('v561_live_default_config'); setChoosing(false); }
    const ready = prepared(view);
    const contentReady = view.contentPrepared && preparedText === text;
    function playPrepared() {
        if (loop.current || !contentReady || view.busy) return;
        if (repeat && !obs) {
            loop.current = { text, after: BigInt(view.requestSequence) };
            setLooping(true);
        }
        void act('v561_live_play_prepared', { text, obs: obs && view.obsPort !== undefined });
    }
    const status = view.code === 'VISUAL_UNAVAILABLE' ? message(view.code) : view.kind === Kind.KIND_CONTENT_PREPARING ? '正在准备完整音画，准备期间不会播放声音；可随时停止。' : view.kind === Kind.KIND_CONTENT_PREPARED ? '内容已准备，可重复播放；修改正文或参考素材后需要重新准备。' : view.kind === Kind.KIND_PREPARING ? (view.code === 'TTS_PREPARING' ? '正在静音预热语音服务，不会播放声音；可随时停止。' : '语音已预热，正在加载并准备画面；可随时停止。') : view.kind === Kind.KIND_STARTING ? (view.code === 'GENERATING_ANSWER' ? '正在生成回答或播放回答，完成后自动回到被打断的句子；可随时停止。' : view.code === 'PLAYING_PREPARED' ? '正在播放已备内容，本次不会调用模型；可随时停止。' : '正在本机生成或播放音画，首次声音需要等待数秒；可随时停止，不会自动重试。') : view.kind === Kind.KIND_STOPPING ? '正在清退当前操作…' : view.kind === Kind.KIND_COMPLETED ? '本段讲解已完成，设备样本已核对。' : view.kind === Kind.KIND_STOPPED ? '当前操作已停止，不会自动继续。' : view.kind === Kind.KIND_FAILED ? message(view.code) : ready ? '语音和画面已准备，可开始讲解。' : '尚未准备数字人。';
    const obsAddress = view.obsPort === undefined ? '' : 'udp://127.0.0.1:' + view.obsPort;
    const obsStatus = view.code === 'PLAYING_PREPARED_OBS' ? '正在发送本机音画。' : view.code === 'OBS_OUTPUT_COMPLETED' ? '发送已结束。' : view.code === 'OBS_OUTPUT_FAILED' ? '独立输出未完成，本机讲解已结束。' : '等待播放。';
    return <main><header><div><span className="brand">nai</span><span className="version">V5.6.1 · 音画工作台</span></div><button onClick={() => setSettings(!settings)}>{settings ? '返回工作台' : '设置'}</button></header>{settings ? <section className="card"><h1>本机音画设置</h1><p>一键使用本机默认装配：已有 FlashHead 模型、授权肖像、worker 与本地语音服务地址；也可自行选择已有 JSON 配置。默认配置只是可运行的起点，不表示模型、语音服务或设备已经就绪。</p><button disabled={!native || view.busy || choosing} onClick={() => void useDefault()}>{choosing ? '正在应用…' : '使用默认配置'}</button><button disabled={!native || view.busy || choosing} onClick={() => void choose()}>{choosing ? '正在选择…' : '选择音画配置'}</button><p>准备会先静音预热语音，再准备画面；输出设备在实际播放时检查，不代表平台已开播。</p></section> : <><section className="intro"><span className="eyebrow">本机数字人</span><h1>直播工作台</h1><p>输入原稿后先准备数字人和讲解音画，完成后即可播放已备内容。按句号或分号自动分句。</p></section><section className="card"><label htmlFor="speech">讲解内容</label><textarea id="speech" value={text} maxLength={1024} disabled={view.busy || looping} onChange={e => { endLoop(); setText(e.target.value); }}/><div className="actions"><button disabled={!native || view.busy || ready || choosing} onClick={() => void act('v561_live_prepare')}>{view.kind === Kind.KIND_PREPARING ? '准备中…' : ready ? '已准备' : '准备数字人'}</button><button disabled={!native || view.busy || !ready || !text.trim()} onClick={() => { setPreparedText(text); void act('v561_live_prepare_content', { text }); }}>准备讲解内容</button><button className="primary" disabled={!native || view.busy || choosing || !ready || !contentReady || looping} onClick={playPrepared}>播放已备内容</button><button className="stop" disabled={!native || (!view.busy && !view.contentPrepared && !looping)} onClick={() => void act('v561_live_stop')}>停止</button></div>{obsAddress && <section aria-label="独立 OBS 输出"><label><input type="checkbox" checked={obs} disabled={view.busy || looping} onChange={e => { setObs(e.target.checked); if (e.target.checked) setRepeat(false); }}/>本次发送至独立 OBS</label><div><input aria-label="OBS 接收地址" readOnly value={obsAddress} onFocus={e => e.target.select()}/><button onClick={() => void navigator.clipboard.writeText(obsAddress).catch(() => setError('复制失败，可选择地址后手动复制。'))}>复制</button></div><p role="status">{obsStatus}操作前请确认本次隔离 OBS 已开始录制。</p></section>}<label><input type="checkbox" checked={repeat} disabled={view.busy || looping || (obs && !!obsAddress)} onChange={e => setRepeat(e.target.checked)}/>循环播放已备内容</label><div className="status" role="status" aria-live="polite">{looping && '循环播放已启用。'}{status}</div><p>语音：{view.ttsPrepared ? '已预热' : '未预热'} · 画面：{view.visualPrepared ? '已准备' : '未准备'}</p><p>“准备讲解内容”会逐句准备整段内容，全部完成后可重复播放，避免重复生成。整段长度和缓存容量由当前配置限制。停止会取消当前操作。已备内容到期或关闭工作台后会清除。已备讲解结束后显示静态待机图，点击停止可清空画面。OBS捕获时请保持公共输出窗口展开。</p><details><summary>即时生成讲解</summary><p>现场生成整段音画，开口前需要等待。</p><button disabled={!native || view.busy || choosing || !ready || !text.trim()} onClick={() => void act('v561_live_start', { text })}>生成并播放</button></details></section><section className="card"><label htmlFor="question">提问</label><input id="question" value={question} maxLength={1024} onChange={e => setQuestion(e.target.value)}/><button disabled={!native || !question.trim() || !view.busy || view.code !== 'PLAYING_PREPARED'} onClick={() => void act('v561_live_ask', { question })}>AI回答</button><p>播放已备内容时可插问；回答后自动继续原稿。停止将同时取消回答和恢复。</p></section></>}{localGenerationActive(view) && <LocalElapsed key={String(view.requestSequence)}/>}{error && <p role="alert" className="error">{error}</p>}{view.code === 'CONFIG_REQUIRED' && <p role="alert" className="error">{errors.CONFIG_REQUIRED}</p>}{!native && <p role="alert" className="error">网页预览不会调用模型，请使用原生工作台。</p>}<footer>单个数字人、单个语音服务；不自动重试。准备成功不代表持续实时或平台开播通过。</footer></main>;
}
createRoot(document.getElementById('root')!).render(<Voice />);


