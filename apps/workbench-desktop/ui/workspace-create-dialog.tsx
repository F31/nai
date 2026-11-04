import {useEffect,useRef} from 'react';
import type {Workbench} from './workbench';
import {WorkspaceBudgetFields} from './workspace-budget-fields';
export function WorkspaceCreateDialog({v,onClose}:{v:Workbench;onClose:()=>void}){
 const dialog=useRef<HTMLDialogElement>(null);
 useEffect(()=>{const previous=document.activeElement as HTMLElement|null;dialog.current?.showModal();return()=>{dialog.current?.close();previous?.focus();};},[]);
 return <dialog ref={dialog} className="workspace-removal-dialog" aria-labelledby="workspace-create-title" onCancel={e=>{e.preventDefault();if(!v.busy)onClose();}}><h2 id="workspace-create-title">新建工作区</h2><form onSubmit={e=>{e.preventDefault();void v.run(async()=>{await v.createWorkspace();onClose();});}}><label>工作区名称<input autoFocus required maxLength={128} disabled={v.busy||v.workspaceCreatePending} value={v.workspaceName} onChange={e=>v.setWorkspaceName(e.target.value)}/></label><WorkspaceBudgetFields mode={v.workspaceBudgetMode} limit={v.workspaceBudgetLimit} onMode={v.setWorkspaceBudgetMode} onLimit={v.setWorkspaceBudgetLimit} disabled={v.busy||v.workspaceCreatePending}/>{v.error&&<p role="alert" className="banner error">{v.error}</p>}<div className="row-actions"><button type="button" disabled={v.busy} onClick={onClose}>取消</button><button className="primary" disabled={v.busy||v.dirty||!v.connected||!v.workspaceName.trim()}>{v.workspaceCreatePending?'恢复切换':'创建工作区'}</button></div></form></dialog>;
}
