import {AgentTaskState,type QuestionPoolView,type QuestionPoolEntry} from '@nai/contracts';
const uuid=/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/,sha=/^[0-9a-f]{64}$/;
export function validateQuestionPool(v:QuestionPoolView|undefined,ws:string,snapshot:string,snapshotSha:string):QuestionPoolView{
 const c=v?.context;
 if(!v||!c||c.workspaceId!==ws||c.snapshotId!==snapshot||c.snapshotSha256!==snapshotSha||!sha.test(c.factsSha256)||!sha.test(c.contextSha256)||v.capacity!==32||v.ttlMs!==120000||v.entries.length>32||new Set(v.entries.map(e=>e.id)).size!==v.entries.length)throw Error('问题池上下文不匹配，请重新读取。');
 for(const e of v.entries){const created=Number(e.createdAtMs),expires=Number(e.expiresAtMs);if(!uuid.test(e.id)||!/^\d+$/.test(e.revision)||BigInt(e.revision)<1n||e.contextSha256!==c.contextSha256||!e.question.trim()||[...e.question].length>256||!Number.isSafeInteger(created)||!Number.isSafeInteger(expires)||created<0||expires<=created||expires-created>v.ttlMs||e.occurrences<1||e.occurrences>99||!Number.isInteger(e.occurrences)||![2,4,5,6,7].includes(e.state)||e.reason.length>64||e.taskId!==''&&(!uuid.test(e.taskId)||!sha.test(e.taskSha256))||e.taskId===''&&e.taskSha256!=='')throw Error('问题池结果不完整，请重新读取。');}
 return v;
}
export function questionState(e:QuestionPoolEntry,now=Date.now()):string{
 if(e.state===AgentTaskState.AGENT_TASK_STATE_CANCELLED)return '已取消';
 if(Number(e.expiresAtMs)<=now||Number(e.createdAtMs)>now||e.state===AgentTaskState.AGENT_TASK_STATE_EXPIRED)return '已失效';
 if(e.reason==='ADOPTED_TO_DRAFT_ONLY')return '已采用到草稿';
 if(e.state===AgentTaskState.AGENT_TASK_STATE_WAITING_HUMAN)return '待核对候选';
 if(e.state===AgentTaskState.AGENT_TASK_STATE_QUEUED)return '待处理';
 return '已结案';
}
export function canRunQuestion(e:QuestionPoolEntry,now=Date.now()){return e.state===AgentTaskState.AGENT_TASK_STATE_QUEUED&&Number(e.createdAtMs)<=now&&Number(e.expiresAtMs)>now;}
export function canInspectQuestion(e:QuestionPoolEntry,now=Date.now()){return e.state===AgentTaskState.AGENT_TASK_STATE_WAITING_HUMAN&&Number(e.expiresAtMs)>now&&Number(e.createdAtMs)<=now&&!!e.taskId;}

