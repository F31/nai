import {useEffect,useState} from 'react';
import {MicStatus} from '@nai/contracts';
export function HumanSafety(){const [status,setStatus]=useState<MicStatus|null>(null),[error,setError]=useState('');const native=(window as Window&{__TAURI__?:{core:{invoke:(name:string)=>Promise<number[]>}}}).__TAURI__;
 useEffect(()=>{if(!native)return;let active=true;const poll=()=>{void native.core.invoke('workbench_mic_poll').then(b=>{if(active)setStatus(MicStatus.decode(Uint8Array.from(b)));}).catch(()=>{if(active)setStatus(null);});};poll();const t=setInterval(poll,500);return()=>{active=false;clearInterval(t);};},[]);
 if(!native)return null;return <><span role="status">{status?.active?'● 真人音轨开启':status?'真人音轨关闭':''}</span><button className="danger" title="停止本工作台数字人、真人麦克风和私有试听，不控制OBS其他来源" onClick={()=>void native.core.invoke('workbench_all_mute').then(()=>setError('')).catch(()=>setError('静音请求未确认，请核对设备和OBS'))}>工作台全静音</button>{error&&<span role="alert">{error}</span>}</>;
}
