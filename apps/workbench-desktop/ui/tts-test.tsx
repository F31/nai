import {AuditionControls} from './audition-controls';
import {useEffect,useRef,useState} from 'react';
import {AgentTaskState,type FoundationResult,type TtsModelView} from '@nai/contracts';
import type {Workbench} from './workbench';
import {explain,newId} from './api';
import {isRunning} from './llm';
import {providerFailureMessage} from './provider-failure';
import {submitTtsTest,ttsDeadline,TtsActionGate} from './tts-test-flow';
export function TtsTest({v,model}:{v:Workbench;model:TtsModelView}){
 const [result,setResult]=useState<FoundationResult|null>(null),[error,setError]=useState(''),[released,setReleased]=useState(false);
 const [phase,setPhase]=useState(''),[unknown,setUnknown]=useState(false),[pollFailed,setPollFailed]=useState(false);
 const alive=useRef(true),action=useRef(new TtsActionGate()),task=useRef(''),audio=useRef<{id:string;sha:string}|null>(null);
 const ws=v.ws,api=v.api;
 function accept(r:FoundationResult){if(!alive.current)return;setResult(r);setUnknown(false);setPollFailed(false);setError('');if(r.taskState===AgentTaskState.AGENT_TASK_STATE_COMPLETED&&r.body)audio.current={id:r.resourceId,sha:r.sha256};if(!isRunning(r))task.current='';}
 async function act(label:string,fn:()=>Promise<unknown>){await action.current.run(async()=>{setPhase(label);setError('');try{await fn();}catch(e){if(alive.current)setError(explain(e));}finally{if(alive.current)setPhase('');}});}
 async function release(){await ttsDeadline(v.priorityAuditionStop(),'stop');const a=audio.current;if(!a)return;await ttsDeadline(api.call({$case:'revokeTtsAudio',revokeTtsAudio:{workspaceId:ws,taskId:a.id,taskSha256:a.sha}}),'status');audio.current=null;if(alive.current)setReleased(true);}
 async function cancelId(id:string){return ttsDeadline(api.call({$case:'cancelTts',cancelTts:{workspaceId:ws,taskId:id}}),'status');}
 useEffect(()=>{alive.current=true;return()=>{alive.current=false;void v.priorityAuditionStop().catch(()=>{});if(task.current&&!api.pending)void cancelId(task.current).catch(()=>{});else if(audio.current&&!api.pending)void release().catch(()=>{});};},[ws,model.id]);
 useEffect(()=>{if(!result||!isRunning(result)||pollFailed)return;let active=true,inFlight=false;const timer=setInterval(()=>{if(api.pending||action.current.busy||inFlight)return;inFlight=true;void ttsDeadline(api.call({$case:'readTtsTask',readTtsTask:{workspaceId:ws,taskId:result.resourceId}}),'status').then(r=>{if(active&&alive.current)accept(r);}).catch(()=>{if(active&&alive.current){setPollFailed(true);setError('读取状态失败，请核对原任务；不要重复合成。');}}).finally(()=>{inFlight=false;});},500);return()=>{active=false;clearInterval(timer);};},[result?.resourceId,result?.taskState,ws,pollFailed]);
 async function start(){if(unknown||api.pending)throw Error('请先核对原任务。');
  await submitTtsTest(release,()=>alive.current,()=>{const id=newId();task.current=id;return api.call({$case:'testTts',testTts:{workspaceId:ws,taskId:id,modelId:model.id,modelRevision:model.revision}});},()=>{setPhase('正在提交测试请求…');setUnknown(true);setResult(null);setReleased(false);},r=>{if(alive.current)accept(r);else if(isRunning(r)&&!api.pending)void cancelId(r.resourceId).catch(()=>{});});
 }
 async function check(){if(api.pending)throw Error('原生请求仍在等待回执，暂不能核对；请稍后重试核对，勿再次合成。');const id=task.current||result?.resourceId;if(!id)throw Error('没有待核对的任务。');accept(await ttsDeadline(api.call({$case:'readTtsTask',readTtsTask:{workspaceId:ws,taskId:id}}),'status'));}
 let samples=0;if(result?.body)try{const b=JSON.parse(result.body);if(b.kind==='publication_tts_test'){if(Number.isInteger(b.sample_count)&&b.sample_count>0&&b.sample_count<=480000)samples=b.sample_count;}}catch{/* A malformed result never becomes playable. */}
 const running=!!result&&isRunning(result),busy=v.busy||!!phase,locked=busy||running||unknown;
 return <section className="model-editor"><h2>语音合成测试 · {model.name}</h2><p>仅合成固定短句“你好，这是语音合成测试。”，最长等待45秒、不重试。音频短期缓存，不自动播放；合成后可点击本机试听。</p>
 <div className="actions"><button disabled={locked||api.pending} onClick={()=>void act('正在停止旧试听，准备测试…',start)}>测试合成</button><button disabled={busy||(!running&&!unknown)||api.pending} onClick={()=>void act('正在请求取消…',async()=>{const id=task.current||result?.resourceId;if(id)accept(await cancelId(id));})}>取消合成</button><button disabled={busy||(!result&&!unknown)} onClick={()=>void act('正在核对原任务…',check)}>核对任务状态</button><button disabled={locked||samples===0||released} onClick={()=>void act('正在释放音频…',release)}>释放本次音频</button></div>
 {phase&&<p role="status">{phase}</p>}{unknown&&!phase&&<p role="status">请求结果尚未确认，未重试。请核对原任务；没有成功音频时无法试听。</p>}
 {released&&<p role="status">本次音频已释放；历史任务和费用保留。</p>}{error&&<p role="alert">{error}</p>}{result&&<div role="status"><p>请求编号：<code>{result.resourceId}</code></p><p>{running?'正在等待供应商合成（最长45秒），可取消；成功后点击“试听（本机）”。':result.taskState===AgentTaskState.AGENT_TASK_STATE_COMPLETED&&samples>0?`合成并校验成功：24kHz / 单声道 / PCM16，${(samples/24000).toFixed(2)}秒。尚未试听。`:result.taskState===AgentTaskState.AGENT_TASK_STATE_CANCELLED?'已取消；已派发部分不假退款。':`合成失败或结果失效（${providerFailureMessage(result.taskReason)}），没有可试听音频，未重试。`}</p></div>}
 {result&&samples>0&&!released&&<AuditionControls key={result.resourceId} v={v} task={result}/>}
 </section>;
}
