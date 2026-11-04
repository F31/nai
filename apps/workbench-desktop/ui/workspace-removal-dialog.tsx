import {useEffect,useRef,useState} from 'react';
import type {Resource} from './api';
import type {Workbench} from './workbench';
import {WorkspaceRemovalReview,type RemovalReviewState} from './workspace-removal-review';
export function WorkspaceRemovalDialog({v,target,onClose}:{v:Workbench;target:Resource;onClose:()=>void}){
 const dialog=useRef<HTMLDialogElement>(null);
 const [state,setState]=useState<RemovalReviewState>({phase:'checking',row:null,error:''});
 const [flow]=useState(()=>new WorkspaceRemovalReview(setState));
 const submitting=state.phase==='submitting';
 function cancel(){if(flow.cancel())onClose();}
 useEffect(()=>{
  const previous=document.activeElement as HTMLElement|null;
  dialog.current?.showModal();
  void v.run(()=>flow.open(target,v.ws,()=>v.inspectWorkspace(target)));
  return()=>{flow.cancel();dialog.current?.close();previous?.focus();};
 },[]);
 return <dialog ref={dialog} className="workspace-removal-dialog" aria-labelledby="workspace-removal-title" aria-describedby="workspace-removal-description" onCancel={e=>{e.preventDefault();cancel();}}>
  <h2 id="workspace-removal-title">删除“{target.name}”</h2>
  <div id="workspace-removal-description">{state.phase==='checking'?<p role="status">正在检查工作区关联，请稍候…</p>:state.error?<p className="banner error" role="alert">{state.error} 请取消后刷新列表核对。</p>:<p>{state.row?.hasContent?'此工作区有关联内容、任务、费用记录或自定义预算。将移入回收站，保留内容和密钥，可恢复；旧任务和播放许可将撤销。':'此工作区没有关联业务内容，将彻底删除且无法恢复。删除审计会保留。'}</p>}</div>
  <p className="muted">只有点击下方确认按钮才执行删除；确认时会再次检查版本及关联。</p>
  <div className="row-actions"><button type="button" autoFocus disabled={submitting} onClick={cancel}>取消</button><button type="button" className="danger" disabled={state.phase!=='ready'||v.busy||v.dirty||target.id===v.ws} onClick={()=>void v.run(async()=>{if(await flow.confirm(v.ws,v.removeWorkspace))onClose();})}>{submitting?'正在处理…':state.row?.hasContent?'确认移入回收站':'确认彻底删除'}</button></div>
 </dialog>;
}
