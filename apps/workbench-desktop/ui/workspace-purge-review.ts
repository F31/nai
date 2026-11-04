import type {WorkspacePurgePlan,WorkspacePurgeStatus} from '@nai/contracts';
import {ApiError,explain,type Resource} from './api.ts';
import {requireInactiveWorkspace} from './workspace-removal-review.ts';
export type PurgeState={phase:'closed'|'checking'|'ready'|'submitting'|'complete'|'pending'|'failed';plan:WorkspacePurgePlan|null;status:WorkspacePurgeStatus|null;error:string};
export class WorkspacePurgeReview{
 state:PurgeState={phase:'closed',plan:null,status:null,error:''};private generation=0;
 private changed:(s:PurgeState)=>void;constructor(changed:(s:PurgeState)=>void){this.changed=changed;}
 private set(s:PurgeState){this.state=s;this.changed(s);}
 async open(row:Resource,current:string,inspect:()=>Promise<WorkspacePurgePlan>){const gen=++this.generation;this.set({phase:'checking',plan:null,status:null,error:''});try{requireInactiveWorkspace(row.id,current);if(!row.recycled)throw new ApiError('REVISION_CONFLICT');const plan=await inspect();if(gen!==this.generation)return;if(plan.workspaceId!==row.id||plan.revision!==row.revision)throw new ApiError('REVISION_CONFLICT');this.set({phase:'ready',plan,status:null,error:''});}catch(e){if(gen===this.generation)this.set({phase:'failed',plan:null,status:null,error:explain(e)});}}
 cancel(){if(this.state.phase==='submitting')return false;++this.generation;this.set({phase:'closed',plan:null,status:null,error:''});return true;}
 async confirm(current:string,acknowledged:boolean,remove:(p:WorkspacePurgePlan,progress:(s:WorkspacePurgeStatus)=>void)=>Promise<WorkspacePurgeStatus>){const {plan,phase}=this.state;if(!['ready','pending'].includes(phase)||!plan||!acknowledged||plan.blockers.length)return false;this.set({...this.state,phase:'submitting',error:''});try{requireInactiveWorkspace(plan.workspaceId,current);const status=await remove(plan,status=>this.set({...this.state,status}));if(status.workspaceId!==plan.workspaceId||!status.deleted)throw new ApiError('PROTOCOL_ERROR',true);this.set({phase:status.pendingFiles?'pending':'complete',plan,status,error:''});return !status.pendingFiles;}catch(e){const deleted=!!this.state.status?.deleted;this.set({...this.state,phase:deleted?'pending':'failed',error:(deleted?'业务数据已删除，文件清理结果待确认。':'')+explain(e)});return false;}}
}

