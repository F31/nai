import type {SharedModelMigration,SharedModelMigrationStatus} from '@nai/contracts';
const uuid=/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const sha=/^[a-f0-9]{64}$/;
export function validMigration(r:SharedModelMigration){return uuid.test(r.migrationId)&&sha.test(r.planSha256)&&r.entries.length<=16&&new Set(r.entries.map(e=>e.catalogModelId)).size===r.entries.length&&new Set(r.entries.map(e=>e.sourceWorkspaceId+':'+e.sourceModelId)).size===r.entries.length&&r.entries.every(e=>uuid.test(e.catalogModelId)&&uuid.test(e.sourceWorkspaceId)&&uuid.test(e.sourceModelId)&&/^[1-9][0-9]*$/.test(e.sourceRevision)&&sha.test(e.sourceSha256)&&['llm','tts'].includes(e.capability));}
export function validMigrationStatus(s:SharedModelMigrationStatus){return uuid.test(s.migrationId)&&['preparing','rewrapping','committed'].includes(s.stage)&&Number.isInteger(s.total)&&s.total>=0&&s.total<=16&&Number.isInteger(s.completed)&&s.completed>=0&&s.completed<=s.total&&!!s.request&&validMigration(s.request)&&s.request.migrationId===s.migrationId&&s.request.entries.length===s.total&&(!s.backupId||uuid.test(s.backupId))&&(!s.backupManifestSha256||sha.test(s.backupManifestSha256))&&(s.stage!=='committed'||s.completed===s.total&&!!s.backupId&&!!s.backupManifestSha256);}
export async function runMigration(request:SharedModelMigration,step:(r:SharedModelMigration)=>Promise<SharedModelMigrationStatus>,progress:(s:SharedModelMigrationStatus)=>void){
 if(!validMigration(request))throw Error('迁移映射无效，请重新核对。');
 for(let i=0;i<request.entries.length+4;i++){const s=await step(request);if(!validMigrationStatus(s)||s.migrationId!==request.migrationId||JSON.stringify(s.request)!==JSON.stringify(request))throw Error('迁移响应不匹配，请重新读取进度。');progress(s);if(s.stage==='committed')return;}
 throw Error('迁移尚未完成，请读取进度后继续。');
}
