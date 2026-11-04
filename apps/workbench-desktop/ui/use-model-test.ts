import {useEffect,useRef,useState} from 'react';
import type {FoundationResult,LlmModelView} from '@nai/contracts';
import {Api,ApiError,newId} from './api';
import {isRunning,modelIsSaved,modelTestText,type ModelForm} from './llm';
export function useModelTest(api:Api,ws:string,page:string,connected:boolean,busy:boolean,form:ModelForm,models:LlmModelView[],refresh:()=>Promise<unknown>,onError:(e:unknown)=>void){
 const [result,setResult]=useState<FoundationResult|null>(null);
 const stamp=connected&&page==='models'?JSON.stringify([ws,form]):'';
 const stampRef=useRef(stamp);stampRef.current=stamp;const current=useRef<{ws:string;id:string;stamp:string}|null>(null),starting=useRef(false);
 const saved=modelIsSaved(form,models.find(m=>m.id===form.id));
 useEffect(()=>{setResult(null);},[stamp]);
 async function cancel(){
  const previous=result,t=current.current;if(!t)return;setResult(null);current.current=null;
  try{for(let n=0;api.pending&&n<180;n++)await new Promise(resolve=>setTimeout(resolve,25));if(api.pending)throw new ApiError('BUSY');await api.call({$case:'cancelLlm',cancelLlm:{workspaceId:t.ws,taskId:t.id}});await refresh();if(stampRef.current===t.stamp){const r=await api.call({$case:'readLlmTask',readLlmTask:{workspaceId:t.ws,taskId:t.id}});if(stampRef.current===t.stamp)setResult(r);}}
  catch(e){if(stampRef.current===t.stamp){current.current=t;setResult(previous);}throw e;}
 }

 async function start(){if(starting.current||!saved||!stamp)throw Error('请先保存并启用当前模型。');starting.current=true;setResult(null);const t={ws,id:newId(),stamp};current.current=t;try{const r=await api.call({$case:'testLlm',testLlm:{workspaceId:ws,taskId:t.id,modelId:form.id,modelRevision:form.revision}});if(stampRef.current!==t.stamp||current.current!==t){await api.call({$case:'cancelLlm',cancelLlm:{workspaceId:t.ws,taskId:t.id}});return;}modelTestText(r);setResult(r);await refresh();}catch(e){if(current.current===t)current.current=null;if(e instanceof ApiError&&e.code==='RESOURCE_EXHAUSTED')throw Error('测试未发出：已达到请求或资源容量限制，请稍后再试。');if(e instanceof ApiError&&e.code==='CAPABILITY_UNAVAILABLE')throw Error('测试未发出：模型已停用或凭据不可用，请检查已保存的Key。');throw e;}finally{starting.current=false;}}
 useEffect(()=>{if(!result||!isRunning(result))return;const t=current.current;if(!t)return;let active=true;const timer=setInterval(()=>{if(api.pending||busy)return;void(async()=>{try{const r=await api.call({$case:'readLlmTask',readLlmTask:{workspaceId:t.ws,taskId:t.id}});if(!active||stampRef.current!==t.stamp||current.current!==t)return;modelTestText(r);setResult(r);if(!isRunning(r))await refresh();}catch(e){if(active){setResult(null);current.current=null;onError(e);}}})();},350);return()=>{active=false;clearInterval(timer);};},[result?.resourceId,result?.taskState,busy]);
 return {result,start,cancel,saved,running:!!result&&isRunning(result)};
}


