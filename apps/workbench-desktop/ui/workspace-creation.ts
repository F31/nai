import type {WorkspaceSettingsDraft} from '@nai/contracts';
import {Api,ApiError,newId,type Resource} from './api.ts';
/** Retain an acknowledged write across follow-up failures. Never replay creation. */
export class WorkspaceCreation {
 pending:{id:string;name:string;confirmed:boolean}|null=null;
 private api:Api;
 constructor(api:Api){this.api=api;}
 acknowledge(id:string){if(this.pending?.id===id)this.pending=null;}
 async create(name:string,onList:(rows:Resource[])=>void,activate:(id:string)=>Promise<void>,settings?:WorkspaceSettingsDraft){
  if(!this.pending){
   const clean=name.trim();if(!clean||new TextEncoder().encode(clean).length>128||/[\u0000-\u001f\u007f]/.test(clean))throw new ApiError('INVALID_ARGUMENT');
   const id=newId();
   try{
    const r=await this.api.call(settings?{$case:'createWorkspaceSettings',createWorkspaceSettings:{workspaceId:id,settings:{...settings,name:clean,expectedRevision:'0'}}}:{$case:'createWorkspace',createWorkspace:{workspaceId:id,name:clean,budgetLimitMinor:'0',currency:'CNY'}});
    this.pending={id,name:clean,confirmed:r.resourceId===id};
    if(!this.pending.confirmed)throw new ApiError('OUTCOME_UNKNOWN',true);
   }catch(e){if(e instanceof ApiError&&['OUTCOME_UNKNOWN','ENGINE_LOST','DEADLINE_EXCEEDED','PROTOCOL_ERROR'].includes(e.code))this.pending={id,name:clean,confirmed:false};throw e;}
  }
  const id=this.pending.id;
  try{const rows=await this.api.workspaces();onList(rows);if(!rows.some(r=>r.id===id))throw new ApiError('NOT_FOUND');this.pending.confirmed=true;await activate(id);this.acknowledge(id);}
  catch(e){throw new ApiError(this.pending?.confirmed?'WORKSPACE_CREATED_PENDING':'WORKSPACE_CREATE_UNCONFIRMED',e instanceof ApiError&&e.transport);}
 }
}
