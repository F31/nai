import {useEffect,useRef,useState} from 'react';
import type {FoundationLlmPlan,LlmLocalCheck} from '@nai/contracts';
import {Api,ApiError,explain} from './api';
import {minor} from './llm';
export function useLlmCheck(api:Api,request:Omit<FoundationLlmPlan,'maxCostMinor'>|null,maxCost:string,busy:boolean,running:boolean,budgetStamp:string){
 let amount='';try{amount=minor(maxCost);}catch{/* Invalid local amount cannot issue a request. */}
 const key=request&&amount?JSON.stringify([request,amount,budgetStamp]):'';
 const current=useRef(key);current.current=key;
 const [result,setResult]=useState<{key:string;value:LlmLocalCheck|null;error:string}|null>(null),[retry,setRetry]=useState(0);
 useEffect(()=>{
  if(!key||!request||busy||running)return;
  let cancelled=false,attempts=0;let timer:ReturnType<typeof setTimeout>;
  async function check(){
   if(cancelled)return;
   if(api.pending){if(++attempts<20)timer=setTimeout(()=>void check(),200);else setResult({key,value:null,error:'其他读取尚未结束，请稍后重新检查。'});return;}
   try{const r=await api.call({$case:'checkLlm',checkLlm:{...request!,maxCostMinor:amount}});if(!r.llmCheck)throw new ApiError('PROTOCOL_ERROR',true);if(!cancelled&&current.current===key)setResult({key,value:r.llmCheck,error:''});}
   catch(e){if(!cancelled&&current.current===key)setResult({key,value:null,error:explain(e)});}
  }
  timer=setTimeout(()=>void check(),200);return()=>{cancelled=true;clearTimeout(timer);};
 },[key,busy,running,retry]);
 const visible=result?.key===key?result:null;
 return{value:visible?.value??null,error:visible?.error||(!amount&&request?'本次预留上限请填写非负元金额，最多两位小数。':''),pending:!!key&&!visible&&!busy&&!running,recheck:()=>{setResult(null);setRetry(x=>x+1);}};
}
