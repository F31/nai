import {useEffect,useRef,useState} from 'react';
import type {FoundationResult,QuestionPoolEntry,QuestionPoolView,ReviewSource} from '@nai/contracts';
import {Api,newId,type Resource} from './api';
import {validateQuestionPool} from './question-pool';
export type QuestionRun={source:ReviewSource;expectedEpoch:string;label:string};
export function useQuestionPool({api,ws,snapshot,snapshotMeta,run,enabled,busy,onError,onCandidate,onClear}:{api:Api;ws:string;snapshot:FoundationResult|null;snapshotMeta:Resource|null;run?:QuestionRun;enabled:boolean;busy:boolean;onError:(e:unknown)=>void;onCandidate:(id:string,sha:string)=>Promise<void>;onClear:()=>void}){
 const [pool,setPool]=useState<QuestionPoolView|null>(null),[notice,setNotice]=useState('');
 const context=enabled&&snapshotMeta&&snapshot?`${ws}:${snapshotMeta.id}:${snapshot.sha256}:${run?.source.runKind??0}:${run?.source.runId??""}:${run?.expectedEpoch??""}`:'',current=useRef(context),version=useRef(0),loaded=useRef('');current.current=context;
 useEffect(()=>{version.current++;loaded.current='';setPool(null);setNotice('');},[context]);
 useEffect(()=>()=>{version.current++;},[]);
 function source(){if(!context||!snapshotMeta||!snapshot)throw Error('请先选择有效的已发布版本。');return{workspaceId:ws,snapshotId:snapshotMeta.id,snapshotSha256:snapshot.sha256};}
 async function refresh(){const scope=source(),selected=context,marker=++version.current;const r=await api.call(run?{$case:'readRunQuestionPool',readRunQuestionPool:{source:run.source,scope,expectedEpoch:run.expectedEpoch}}:{$case:'readQuestionPool',readQuestionPool:scope});const next=validateQuestionPool(r.questionPool,ws,scope.snapshotId,scope.snapshotSha256);if(current.current===selected&&version.current===marker)setPool(next);}
 async function enqueue(question:string){const scope=source(),selected=context;const r=await api.call(run?{$case:'enqueueRunQuestion',enqueueRunQuestion:{source:run.source,expectedEpoch:run.expectedEpoch,question:{scope,questionId:newId(),question:question.trim()}}}:{$case:'enqueueQuestion',enqueueQuestion:{scope,questionId:newId(),question:question.trim()}});validateQuestionPool(r.questionPool,ws,scope.snapshotId,scope.snapshotSha256);if(current.current!==selected)return;setNotice('问题已入池；重复问题合并计数，有效期不会延长。');await refresh();}
 async function inspect(e:QuestionPoolEntry){if(!pool||!e.taskId)throw Error('请先处理该问题。');await onCandidate(e.taskId,e.taskSha256);}
 async function prepare(e:QuestionPoolEntry){const scope=source(),selected=context;const r=await api.call({$case:'runQuestion',runQuestion:{workspaceId:ws,questionId:e.id,expectedRevision:e.revision,taskId:newId()}});const next=validateQuestionPool(r.questionPool,ws,scope.snapshotId,scope.snapshotSha256);if(current.current!==selected)return;await refresh();const row=next.entries.find(x=>x.id===e.id);if(row?.taskId)await onCandidate(row.taskId,row.taskSha256);setNotice('资料检索已结束，请核对下方候选；没有自动播出。');}
 async function cancel(e:QuestionPoolEntry){source();const selected=context;onClear();try{await api.call({$case:'cancelQuestion',cancelQuestion:{workspaceId:ws,questionId:e.id,expectedRevision:e.revision}});if(current.current===selected){setNotice('问题及未采用的子候选已取消。已采用的草稿保留。');await refresh();}}catch(error){if(current.current===selected){setPool(null);setNotice('取消结果未确认，已隐藏候选；请刷新核对，不会自动重试。');}throw error;}}
 useEffect(()=>{if(!context||busy||api.pending||loaded.current===context)return;loaded.current=context;void refresh().catch(e=>{if(current.current===context){setNotice('问题池读取失败，请刷新重试。');onError(e);}});},[context,busy]);
 return{runLabel:run?.label,pool,notice,refresh,enqueue,prepare,inspect,cancel};
}
