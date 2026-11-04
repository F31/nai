import {useEffect,useRef,useState} from 'react';
import {FaqDecision,type FoundationResult} from '@nai/contracts';
import {Api,ApiError,newId,type Resource} from './api';
import {FaqVisibility,faqDocument,canAdopt,type FaqDocument} from './faq';
type Task={result:FoundationResult;document:FaqDocument|null;context:string};
export function useFaq({api,ws,snapshot,snapshotMeta,questions,enabled,busy,onError,onAdopted}:{api:Api;ws:string;snapshot:FoundationResult|null;snapshotMeta:Resource|null;questions:string[];enabled:boolean;busy:boolean;onError:(e:unknown)=>void;onAdopted:(id:string)=>Promise<void>}){
 const [question,setQuestion]=useState(''),[task,setTask]=useState<Task|null>(null),[edit,setEdit]=useState(''),[editing,setEditing]=useState(false),[ack,setAck]=useState(false),[notice,setNotice]=useState(''),[cancelling,setCancelling]=useState(false);
 const [visibility]=useState(()=>new FaqVisibility()),pendingTask=useRef(false),taskRef=useRef<Task|null>(null);
 const context=enabled&&ws&&snapshotMeta&&snapshot?`${ws}:${snapshotMeta.id}:${snapshot.sha256}`:'';
 const contextRef=useRef(context);contextRef.current=context;
 const visible=task?.context===context?task:null;
 function clear(){visibility.invalidate();taskRef.current=null;setTask(null);setEdit('');setEditing(false);setAck(false);}
 useEffect(()=>{clear();setNotice('');setCancelling(false);setQuestion(questions[0]??'');},[context,questions]);
 async function resolveRaw(value:Task,decision:FaqDecision){return api.call({$case:'resolveFaq',resolveFaq:{workspaceId:ws,taskId:value.result.resourceId,taskSha256:value.result.sha256,decision,editedText:decision===FaqDecision.FAQ_DECISION_EDIT?edit:'',acknowledgeEdit:decision===FaqDecision.FAQ_DECISION_EDIT&&ack}});}
 async function ask(){if(!context||!snapshotMeta||!snapshot||!question.trim())throw Error('请先选择已发布版本并填写问题。');if(pendingTask.current)throw new ApiError('BUSY');clear();setNotice('');const marker=visibility.begin(),selected=context;pendingTask.current=true;
  try{const result=await api.call({$case:'askFaq',askFaq:{workspaceId:ws,taskId:newId(),snapshotId:snapshotMeta.id,snapshotSha256:snapshot.sha256,question:question.trim()}});const document=await faqDocument(result,snapshotMeta.id,snapshot.sha256);const next={result,document,context:selected};
   if(!visibility.active(marker)||contextRef.current!==selected){try{await resolveRaw(next,FaqDecision.FAQ_DECISION_CANCEL);if(contextRef.current===selected)setNotice('迟到结果已取消，不会采用或出声。');}catch(e){setNotice('查询已隐藏，取消结果未确认；不会采用或重放。');onError(e);}return;}
   taskRef.current=next;setTask(next);setEdit(document?.text??'');
  }catch(e){if(e instanceof ApiError&&e.code==='REVISION_CONFLICT')throw Error('资料版本已变化，请保存场次的最新引用并重新发布后查询。');if(e instanceof ApiError&&e.code==='RESOURCE_EXHAUSTED')throw Error('查询达到频率或资料容量上限。每分钟最多12次，请稍后重试；依据过多时需精简FAQ关联。');throw e;}finally{pendingTask.current=false;setCancelling(false);}
 }
 async function loadTask(taskId:string,taskSha:string){if(!context||!snapshotMeta||!snapshot)throw Error('请先选择已发布版本。');clear();setNotice('');const marker=visibility.begin(),selected=context;const result=await api.call({$case:'readFaq',readFaq:{workspaceId:ws,taskId}});if(result.resourceId!==taskId||result.sha256!==taskSha)throw Error('候选身份不匹配，请刷新问题池。');const document=await faqDocument(result,snapshotMeta.id,snapshot.sha256);if(!visibility.active(marker)||contextRef.current!==selected)return;const next={result,document,context:selected};taskRef.current=next;setTask(next);setEdit(document?.text??'');}
 async function decide(decision:FaqDecision){if(!visible)throw Error('请先查询并选择当前建议。');if(decision===FaqDecision.FAQ_DECISION_EDIT&&!ack)throw Error('请先确认已核对编辑内容。');const marker=visibility.begin(),selected=context;const result=await resolveRaw(visible,decision);if(visibility.active(marker)&&contextRef.current===selected){const next={...visible,result,document:null};taskRef.current=next;setTask(next);setEditing(false);setAck(false);if(result.adoptedObjectId){await onAdopted(result.adoptedObjectId);setNotice('已追加到产品话术草稿；请到产品页核对，再保存场次引用并重新发布。');}else setNotice('建议已忽略。');}}
 async function cancel(){const current=taskRef.current;clear();setCancelling(pendingTask.current);setNotice(pendingTask.current?'已隐藏查询结果，返回后将尝试结案；不会采用或出声。':'');if(pendingTask.current)return;if(!current?.document)return;try{await resolveRaw(current,FaqDecision.FAQ_DECISION_CANCEL);setNotice('任务已取消。');}catch(e){setNotice('界面已清空，取消结果未确认；请重新读取核对。');onError(e);}}
 useEffect(()=>{if(!visible?.document)return;const current=visible,selected=context;let active=true;
 const timer=setInterval(()=>{if(api.pending||busy)return;void(async()=>{try{const result=await api.call({$case:'readFaq',readFaq:{workspaceId:ws,taskId:current.result.resourceId}});const document=await faqDocument(result,snapshotMeta?.id??'',snapshot?.sha256??'');if(active&&contextRef.current===selected&&taskRef.current?.result.resourceId===current.result.resourceId){const next={...current,result,document};taskRef.current=next;setTask(next);}}catch(e){if(active){clear();onError(e);}}})();},1500);
 const expiry=setTimeout(()=>{if(active&&contextRef.current===selected){clear();setNotice('建议已到期，已清空内容，请重新查询。');}},Math.max(0,current.document!.expires_at_ms-Date.now()));return()=>{active=false;clearInterval(timer);clearTimeout(expiry);};
 },[visible?.result.resourceId,!!visible?.document,context,busy]);
 return{question,setQuestion,task:visible,edit,setEdit,editing,setEditing,ack,setAck,notice,cancelling,ask,loadTask,decide,cancel,clear,canAdopt:visible?canAdopt(visible.result):false};
}
