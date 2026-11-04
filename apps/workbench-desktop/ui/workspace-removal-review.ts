import {ApiError,explain,type Resource} from './api.ts';
export function requireInactiveWorkspace(id:string,current:string,action=1){if(action!==3&&id===current)throw new ApiError('WORKSPACE_IN_USE');}
export type RemovalReviewState={phase:'checking'|'ready'|'submitting'|'failed'|'closed';row:Resource|null;error:string};
export class WorkspaceRemovalReview {
 state:RemovalReviewState={phase:'closed',row:null,error:''};
 private generation=0;
 private changed:(state:RemovalReviewState)=>void;
 constructor(changed:(state:RemovalReviewState)=>void){this.changed=changed;}
 private set(state:RemovalReviewState){this.state=state;this.changed(state);}
 async open(meta:Resource,current:string,inspect:()=>Promise<Resource>){
  const generation=++this.generation;this.set({phase:'checking',row:null,error:''});
  try{requireInactiveWorkspace(meta.id,current);const row=await inspect();if(generation!==this.generation)return;if(row.id!==meta.id||row.recycled)throw new ApiError('REVISION_CONFLICT');this.set({phase:'ready',row,error:''});}
  catch(e){if(generation===this.generation)this.set({phase:'failed',row:null,error:explain(e)});}
 }
 cancel(){if(this.state.phase==='submitting')return false;++this.generation;this.set({phase:'closed',row:null,error:''});return true;}
 async confirm(current:string,remove:(row:Resource,action:1|2)=>Promise<void>){
  if(this.state.phase!=='ready'||!this.state.row)return false;
  const row=this.state.row;this.set({phase:'submitting',row,error:''});
  try{requireInactiveWorkspace(row.id,current);await remove(row,row.hasContent?2:1);this.set({phase:'closed',row:null,error:''});return true;}
  catch(e){this.set({phase:'failed',row:null,error:explain(e)});return false;}
 }
}
