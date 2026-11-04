import {useEffect,useRef,useState} from 'react';
import {MicStatus,MicControl,LiveTakeoverControl,FoundationResponse} from '@nai/contracts';
import {newId} from './api';
import type {Workbench} from './workbench';
const invoke=(name:string,args?:Record<string,unknown>)=>{const n=window as Window&{__TAURI__?:{core:{invoke:(name:string,args?:Record<string,unknown>)=>Promise<number[]>}}};if(!n.__TAURI__)throw Error('请使用原生工作台。');return n.__TAURI__.core.invoke(name,args);};
export function useHumanMic(v:Workbench){
 const [status,setStatus]=useState<MicStatus|null>(null),[notice,setNotice]=useState('');const live=useRef(v);live.current=v;
 function accept(bytes:number[]){const s=MicStatus.decode(Uint8Array.from(bytes));if(!s.instanceId||s.inputs.length>8||s.outputs.length>8||s.peakQ16>65535)throw Error('麦克风状态无法核对。');setStatus(s);return s;}
 async function devices(){accept(await invoke('workbench_mic_devices'));setNotice('麦克风仍关闭。请选择输入和耳机或虚拟音频输出，再明确接管。');}
 useEffect(()=>{if(!status)return;let active=true;const timer=setInterval(()=>{void invoke('workbench_mic_poll').then(b=>{if(active)accept(b);}).catch(e=>{if(active&&!String(e).includes('BUSY')){setStatus(s=>s?{...s,active:false,reason:'STATUS_UNAVAILABLE'}:s);}});},200);return()=>{active=false;clearInterval(timer);};},[status?.instanceId]);
 async function current(){const c=live.current.liveRun.context;if(!c)throw Error('请先建立讲品队列。');const r=await live.current.api.call({$case:'readLiveRun',readLiveRun:{workspaceId:live.current.ws,runId:c.runId}});return r.liveContext??c;}
 async function start(inputId:number,outputId:number,ack:boolean){if(!ack||!status||status.active||!inputId||!outputId)throw Error('请核对麦克风和耳机／虚拟输出并确认路由。');const c=await current();const x=LiveTakeoverControl.fromPartial({run:{workspaceId:v.ws,runId:c.runId},takeoverId:c.humanTakeover?c.takeoverId:newId(),expectedEpoch:c.epoch,acknowledge:true,resume:false});
  await invoke('workbench_public_open');try{await invoke('workbench_human_start',{takeover:Array.from(LiveTakeoverControl.encode(x).finish()),route:Array.from(MicControl.encode(MicControl.fromPartial({instanceId:status.instanceId,inputId,outputId,acknowledgeRoute:true})).finish())});accept(await invoke('workbench_mic_poll'));setNotice('已开启独立真人音轨；OBS需要采集 nai-human-mic-worker 的应用音频。提交计数不代表OBS已收声。');}finally{const updated=await current();v.session.current={id:updated.sessionId,epoch:updated.epoch};await live.current.liveRun.refresh();}
 }
 async function resume(ack:boolean){if(!ack)throw Error('请先确认恢复位置及被打断句的处理。');const c=await current();if(!c.humanTakeover)throw Error('当前没有待结束的接管。');const before=await v.api.call({$case:'readLiveAuto',readLiveAuto:{workspaceId:v.ws,runId:c.runId}});const old=before.liveAuto;
  const sourceCheck=(await v.api.call({$case:'readLiveSpeechPlan',readLiveSpeechPlan:{workspaceId:v.ws,runId:c.runId}})).liveSpeechPlan;
  if(!sourceCheck)throw Error('当前资料不可用，真人音轨保持，请先核对资料。');
  if(old&&BigInt(old.maxCostMinor)<=BigInt(old.reservedMinor))throw Error('原自动额度已用完，真人音轨保持；请核对额度后再恢复。');
  const response=FoundationResponse.decode(Uint8Array.from(await invoke('workbench_human_resume',{takeover:Array.from(LiveTakeoverControl.encode(LiveTakeoverControl.fromPartial({run:{workspaceId:v.ws,runId:c.runId},takeoverId:c.takeoverId,expectedEpoch:c.epoch,resume:true,acknowledge:true,skipPartialSentence:c.partialSentence})).finish())})));
  if(response.outcome?.$case!=='result')throw Error('恢复未完成，请刷新进度。');const restored=response.outcome.result.liveContext;if(!restored)throw Error('恢复状态缺失。');v.session.current={id:restored.sessionId,epoch:restored.epoch};setStatus(s=>s?{...s,active:false,reason:'STOPPED'}:s);setNotice('真人音轨已停止，已恢复原稿位置。');
  try{if(old&&restored.state===1){const remaining=BigInt(old.maxCostMinor)-BigInt(old.reservedMinor);if(remaining<=0n)throw Error('已结束接管；原自动额度已用完，请核对额度后重新启动。');
   const p=(await v.api.call({$case:'readLiveSpeechPlan',readLiveSpeechPlan:{workspaceId:v.ws,runId:c.runId}})).liveSpeechPlan;if(!p)throw Error('请重新核对当前资料。');
   await v.api.call({$case:'controlLiveBatch',controlLiveBatch:{run:{workspaceId:v.ws,runId:c.runId},expectedRevision:p.revision,expectedEpoch:p.epoch,approvalId:newId(),planSha256:p.planSha256,acknowledgeRights:true,revoke:false}});
   const fresh=(await v.api.call({$case:'readLiveRun',readLiveRun:{workspaceId:v.ws,runId:c.runId}})).liveContext;if(!fresh)throw Error('请刷新进度。');
   await invoke('workbench_public_open');await v.api.call({$case:'startLiveAuto',startLiveAuto:{run:{workspaceId:v.ws,runId:c.runId},grantId:newId(),expectedRevision:fresh.revision,expectedEpoch:fresh.epoch,planSha256:p.planSha256,modelId:old.modelId,modelRevision:old.modelRevision,maxCostMinor:remaining.toString(),outputId:'',visualSha256:'',licenseSha256:'',stopNonce:'',acknowledgeRights:true}});setNotice('真人音轨已停止，已用原模型和剩余额度重新授权数字人，从恢复点继续。');}
  }finally{await live.current.liveRun.refresh();}
 }
 async function stop(all=false){await invoke(all?'workbench_all_mute':'workbench_mic_stop');setNotice(all?'已请求工作台全部静音，请核对实际状态和OBS外部音轨。':'已请求真人音轨静音，人工模式保留；不会自动恢复数字人。');}
 return{status,notice,devices,start,resume,stop};
}
