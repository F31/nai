import {ApiError} from './api';
import {useEffect,useRef,useState} from 'react';
import type {Workbench} from './workbench';
import type {Resource} from './api';
import {WorkspaceBudgetFields} from './workspace-budget-fields';
import {yuan} from './llm';
export function WorkspaceEditDialog({v,target,onClose}:{v:Workbench;target:Resource;onClose:()=>void}){
 const dialog=useRef<HTMLDialogElement>(null),[name,setName]=useState(target.name),[mode,setMode]=useState(1),[limit,setLimit]=useState('10.00'),[revision,setRevision]=useState('');
 useEffect(()=>{let active=true;const previous=document.activeElement as HTMLElement|null;dialog.current?.showModal();void v.run(async()=>{const b=await v.readWorkspaceBudget(target);if(b.revision!==target.revision)throw new ApiError('REVISION_CONFLICT');if(active){setMode(b.mode);setLimit(b.limitMinor==='0'?'10.00':yuan(b.limitMinor));setRevision(b.revision);}});return()=>{active=false;dialog.current?.close();previous?.focus();};},[]);
 return <dialog ref={dialog} className="workspace-removal-dialog" aria-labelledby="workspace-edit-title" onCancel={e=>{e.preventDefault();if(!v.busy)onClose();}}><h2 id="workspace-edit-title">编辑工作区</h2><form onSubmit={e=>{e.preventDefault();void v.run(async()=>{await v.editWorkspaceSettings(target,name,mode,limit,revision);onClose();});}}><label>工作区名称<input required autoFocus maxLength={128} value={name} disabled={v.busy} onChange={e=>setName(e.target.value)}/></label>{revision?<WorkspaceBudgetFields mode={mode} limit={limit} onMode={setMode} onLimit={setLimit} disabled={v.busy}/>:<p role="status">正在读取预算…</p>}{v.error&&<p role="alert" className="banner error">{v.error}</p>}<div className="row-actions"><button type="button" disabled={v.busy} onClick={onClose}>取消</button><button className="primary" disabled={v.busy||!revision||!name.trim()}>保存工作区</button></div></form></dialog>;
}
