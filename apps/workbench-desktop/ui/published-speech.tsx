import {ttsReady} from './tts';
import {useEffect,useRef,useState} from 'react';
import {AgentTaskState,type FoundationResult,type FoundationSpeechPlan,type SpeechPlanView} from '@nai/contracts';
import {ApiError,explain,newId} from './api';
import {minor,yuan,isRunning} from './llm';
import {AuditionControls} from './audition-controls';
import {PublicCandidate} from './public-candidate';
import type {Workbench} from './workbench';
function speechError(e:unknown){if(e instanceof ApiError&&e.code==='RESOURCE_EXHAUSTED')return '本次上限或工作区可用预算不足，请到设置核对预算。';if(e instanceof ApiError&&e.code==='BUSY')return '其他生成任务尚未结束，请稍后重新检查。';if(e instanceof ApiError&&e.code==='INVALID_ARGUMENT')return '请核对费用上限和本句长度；最多256字符、1024字节，超长不会自动截断。';return explain(e);}
export function PublishedSpeech({v}:{v:Workbench}){
 const [selected,setSelected]=useState(''),[limit,setLimit]=useState('0.10'),[confirm,setConfirm]=useState(false),[plan,setPlan]=useState<SpeechPlanView|null>(null),[result,setResult]=useState<FoundationResult|null>(null),[error,setError]=useState(''),[refresh,setRefresh]=useState(0);
 const [taskId]=useState(newId);const live=useRef(true),checked=useRef(''),inFlight=useRef(''),audio=useRef<{id:string;sha:string}|null>(null);
 const {ws,api,card}=v;const models=v.llm.ttsModels.filter(ttsReady),model=models.find(m=>m.id===selected)??models[0];
 let request:FoundationSpeechPlan|null=null;try{if(card&&model)request={workspaceId:ws,previewId:card.result.resourceId,previewSha256:card.result.sha256,modelId:model.id,modelRevision:model.revision,maxCostMinor:minor(limit),taskId};}catch{/* Invalid amount cannot obtain approval. */}
 const key=JSON.stringify(request)+refresh,running=!!result&&isRunning(result);
 useEffect(()=>{setConfirm(false);setPlan(null);setError('');},[key]);
 useEffect(()=>{live.current=true;return()=>{live.current=false;void v.priorityAuditionStop().catch(()=>{});if(inFlight.current&&!api.pending)void api.call({$case:'cancelTts',cancelTts:{workspaceId:ws,taskId:inFlight.current}}).catch(()=>{});else if(audio.current&&!api.pending)void api.call({$case:'revokeTtsAudio',revokeTtsAudio:{workspaceId:ws,taskId:audio.current.id,taskSha256:audio.current.sha}}).catch(()=>{});};},[ws,taskId]);
 useEffect(()=>{if(!request||result||v.busy||checked.current===key)return;const p=request;let active=true,finished=false;let timer:ReturnType<typeof setTimeout>;function check(){if(!active)return;if(api.pending){timer=setTimeout(check,150);return;}checked.current=key;void api.call({$case:'checkPreviewSpeech',checkPreviewSpeech:p}).then(r=>{if(active&&live.current){if(!r.speechPlan||r.speechPlan.text!==card?.value.text||r.speechPlan.voice!==model?.voice)throw Error('语音计划与当前本句或音色不符。');setPlan(r.speechPlan);}}).catch(e=>{if(active&&live.current)setError(speechError(e));}).finally(()=>{finished=true;});}timer=setTimeout(check,0);return()=>{active=false;clearTimeout(timer);if(!finished&&checked.current===key)checked.current='';};},[key,v.busy,!!result]);
 useEffect(()=>{if(!result||!isRunning(result))return;const timer=setInterval(()=>{if(api.pending)return;void api.call({$case:'readTtsTask',readTtsTask:{workspaceId:ws,taskId:result.resourceId}}).then(r=>{if(live.current){setResult(r);if(r.taskState===AgentTaskState.AGENT_TASK_STATE_COMPLETED&&r.body)audio.current={id:r.resourceId,sha:r.sha256};if(!isRunning(r))inFlight.current='';}}).catch(e=>{if(live.current)setError(speechError(e));});},500);return()=>clearInterval(timer);},[result?.resourceId,result?.taskState,ws]);
 async function start(){if(!request||!plan||!confirm)throw Error('请先核对本句、音色和费用，再勾选批准。');setConfirm(false);inFlight.current=taskId;try{const r=await api.call({$case:'synthesizePreviewSpeech',synthesizePreviewSpeech:{plan:request,taskId,approvalSha256:plan.approvalSha256}});if(live.current){setResult(r);if(!isRunning(r))inFlight.current='';}else if(isRunning(r)&&!api.pending)await api.call({$case:'cancelTts',cancelTts:{workspaceId:ws,taskId}});}catch(e){inFlight.current='';throw e;}}
 const complete=!!result&&result.taskState===AgentTaskState.AGENT_TASK_STATE_COMPLETED&&!!result.body;
 return <section className="model-editor"><h3>本句语音预演</h3><p>使用上方已发布话术，合成后手动播放。停止预演、撤销内容或更换音色后，旧音频不能继续使用。</p>
 {!model?<><p>尚无可用的语音模型，请配置远程凭据或本地服务。</p><button disabled={v.busy} onClick={()=>void v.run(()=>v.changePage('models'))}>设置语音模型</button></>:<>
 {models.length>1?<label>语音模型<select disabled={v.busy||!!result} value={model.id} onChange={e=>setSelected(e.target.value)}>{models.map(m=><option key={m.id} value={m.id}>{m.name}</option>)}</select></label>:<p>语音模型：{model.name}</p>}
 <details><summary>本次费用上限：¥{limit}</summary><label>上限（元）<input disabled={v.busy||!!result} inputMode="decimal" value={limit} onChange={e=>setLimit(e.target.value)}/></label></details>
 {plan&&!result&&<><p>音色：{plan.voice}；{plan.characters} 字符，{plan.estimateKnown?'按配置单价预留':'单价未知，按本次上限全额预留'} ¥{yuan(plan.reserveMinor)}。实际账单以供应商为准，无用量回执保持未知费用。</p><label className="check"><input type="checkbox" disabled={v.busy} checked={confirm} onChange={e=>setConfirm(e.target.checked)}/>我已核对上方本句话术，批准此音色的本机语音合成并承担本次 API 费用</label></>}
 <div className="actions"><button disabled={v.busy||!plan||!confirm||!!result} onClick={()=>void v.run(start)}>批准并合成本句</button>{!result&&<button disabled={v.busy} onClick={()=>{checked.current='';setRefresh(n=>n+1);}}>重新检查</button>}<button disabled={v.busy||!running} onClick={()=>void v.run(async()=>{await v.priorityAuditionStop();const r=await api.call({$case:'cancelTts',cancelTts:{workspaceId:ws,taskId}});setResult(r);inFlight.current='';})}>取消合成</button></div>
 {running&&<p role="status">正在合成本句，可取消；不会自动播放或重试。</p>}{result&&!running&&<p role="status">{complete?'本句合成成功，可手动播放。':'本句未合成成功或已失效；重新预演后再准备，未自动重试。'} 费用状态：{result.chargeStatus==='unknown'?'未知，继续占用额度':result.chargeStatus==='released'?'未派发，已释放':'已预留'}。</p>}{complete&&<><AuditionControls v={v} task={result} label="播放本句（本机）"/><PublicCandidate key={result.sha256} v={v} task={result}/></>}</>}
 {model&&!request&&<p role="alert">请展开费用上限并填写有效金额。</p>}{error&&<p role="alert">{error}</p>}</section>;
}

