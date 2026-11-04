import {WorkspaceEditDialog} from './workspace-edit-dialog';
import {WorkspacePurgeDialog} from './workspace-purge-dialog';
import {useEffect,useState} from 'react';
import type {Workbench} from './workbench';
import type {Resource} from './api';
import {WorkspaceRemovalDialog} from './workspace-removal-dialog';
import {WorkspaceCreateDialog} from './workspace-create-dialog';
export function workspaceTime(value:string){const n=Number(value);return /^\d+$/.test(value)&&Number.isSafeInteger(n)&&n>0&&n<=8640000000000000?new Date(n).toLocaleString('zh-CN',{hour12:false}):'暂无时间记录';}
export function WorkspaceManager({v}:{v:Workbench}){
 const [editing,setEditing]=useState<Resource|null>(null);
 const [purging,setPurging]=useState<Resource|null>(null);
 const [creating,setCreating]=useState(false);
 const [trash,setTrash]=useState(false),[removal,setRemoval]=useState<Resource|null>(null);
 useEffect(()=>{void v.run(v.refreshWorkspaces);},[]);
 const rows=trash?v.recycledWorkspaces:v.workspaces;
 const locked=v.busy||v.dirty||!!editing||!!removal||!!purging||creating;
 return <article><div className="page-title"><h2>已保存的工作区（{v.workspaces.length}）</h2><div className="row-actions"><button disabled={locked} onClick={()=>void v.run(v.refreshWorkspaces)}>刷新列表</button><button className="primary" disabled={locked||!v.connected} onClick={()=>setCreating(true)}>＋ 新建</button></div></div>
 {creating&&<WorkspaceCreateDialog v={v} onClose={()=>setCreating(false)}/>}
 <div className="model-tabs"><button disabled={locked} className={!trash?'active':''} onClick={()=>setTrash(false)}>工作区</button><button disabled={locked} className={trash?'active':''} onClick={()=>setTrash(true)}>回收站（{v.recycledWorkspaces.length}）</button></div>
 {!rows.length?<p>{trash?'回收站为空。':'尚无工作区，请点击右上角新建。'}</p>:<div className="model-table-wrap"><table className="model-table"><thead><tr><th>工作区名称</th><th>创建时间</th><th>{trash?'移入回收站时间':'最近修改'}</th><th>操作</th></tr></thead><tbody>{rows.map(w=><tr key={w.id}><td><strong>{w.name}</strong>{w.id===v.ws&&<small>● 当前使用</small>}</td><td>{workspaceTime(w.createdAtMs)}</td><td>{workspaceTime(w.updatedAtMs)}</td><td><div className="row-actions">{trash?<><button disabled={locked} onClick={()=>void v.run(()=>v.removeWorkspace(w,3))}>恢复</button><button className="danger" disabled={locked||w.id===v.ws} onClick={()=>setPurging(w)}>彻底删除</button></>:<><button disabled={locked||w.id===v.ws} onClick={()=>void v.run(()=>v.selectWorkspace(w.id))}>{w.id===v.ws?'使用中':'进入工作区'}</button><button disabled={locked} onClick={()=>setEditing(w)}>编辑</button><button disabled={locked||w.id===v.ws} title={w.id===v.ws?'正在使用，请先进入其他工作区再删除':undefined} onClick={()=>setRemoval(w)}>删除</button></>}</div></td></tr>)}</tbody></table></div>}
 {removal&&<WorkspaceRemovalDialog v={v} target={removal} onClose={()=>setRemoval(null)}/>}
 {trash&&!!v.cleanupWorkspaces.length&&<section><h3>待完成的文件清理</h3><p className="hint">以下工作区的业务数据已永久删除，可继续处理尚未清完的内容文件。</p>{v.cleanupWorkspaces.map(w=><div className="row-head" key={w.id}><span>{w.name||'已删除工作区'}</span><button disabled={locked} onClick={()=>setPurging(w)}>继续文件清理</button></div>)}</section>}
 {purging&&<WorkspacePurgeDialog v={v} target={purging} onClose={()=>setPurging(null)}/>}
 {editing&&<WorkspaceEditDialog v={v} target={editing} onClose={()=>setEditing(null)}/>}
 </article>;
}

