import type {FoundationResult,SharedModelMigration} from '@nai/contracts';
import type {Api} from './api.ts';
import {ApiError,newId} from './api.ts';
import {runMigration} from './shared-model-migration.ts';

export type CatalogRead={result:FoundationResult};
export function catalogError(e:unknown):string{
 const code=e instanceof ApiError?e.code:'';
 const reason:Record<string,string>={RESOURCE_EXHAUSTED:'数据库中同类模型超过当前容量（每类 8 个），未截断或删除任何配置。',BUSY:'仍有模型任务或预演正在运行，请停止后刷新列表。',REVISION_CONFLICT:'配置在读取过程中发生变化，请刷新列表重新核对。',PERMISSION_DENIED:'当前权限或凭据验证未通过，请检查本机凭据后重试。',INTEGRITY_FAILED:'配置或备份完整性验证未通过，请保留现场。'};
 return reason[code]??'模型读取或准备未完成，请刷新列表重试；原配置与备份保留。';
}
// User-authorized automatic preparation uses the same backup/journal and
// credential verification as explicit migration. Never merge or drop sources.
export async function readModelCatalog(api:Pick<Api,'call'>,workspaceId:string):Promise<CatalogRead>{
 const read=()=>api.call({$case:'readLlmSettings',readLlmSettings:{workspaceId}});
 const result=await read();
 if(result.sharedCatalogEnabled)return{result};
 const state=await api.call({$case:'readSharedMigration',readSharedMigration:{workspaceId:''}});
 let request:SharedModelMigration;
 if(state.sharedCatalogEnabled){const result=await read();if(!result.sharedCatalogEnabled)throw new ApiError('PROTOCOL_ERROR',true);return{result};}
 if(state.sharedModelMigration){
  if(!state.sharedModelMigration.request)throw new ApiError('PROTOCOL_ERROR',true);
  request=state.sharedModelMigration.request;
 }else{
  const r=await api.call({$case:'previewSharedModels',previewSharedModels:{workspaceId:''}}),p=r.sharedModelPreview;
  if(!p||p.executable||p.rows.length>64||!/^[a-f0-9]{64}$/.test(p.planSha256)||p.rows.some(row=>!row.source))throw new ApiError('PROTOCOL_ERROR',true);
  if(['llm','tts'].some(k=>p.rows.filter(row=>row.source!.capability===k).length>8))throw new ApiError('RESOURCE_EXHAUSTED');
  request={migrationId:newId(),planSha256:p.planSha256,entries:p.rows.map(row=>({...row.source!,catalogModelId:newId()}))};
 }
 await runMigration(request,async request=>{
  const r=await api.call({$case:'migrateSharedModels',migrateSharedModels:request});
  if(!r.sharedModelMigration)throw new ApiError('PROTOCOL_ERROR',true);
  return r.sharedModelMigration;
 },()=>{});
 const shared=await read();
 if(!shared.sharedCatalogEnabled)throw new ApiError('PROTOCOL_ERROR',true);
 return{result:shared};
}


