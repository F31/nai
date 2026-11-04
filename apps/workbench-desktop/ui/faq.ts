import {AgentTaskState,SuggestionState,FaqReason,type FoundationResult,type publicationConfig} from '@nai/contracts';
import {ApiError} from './api.ts';
export type FaqDocument=Extract<publicationConfig.PublicationArtifact,{kind:'publication_faq_task'}>;
export class FaqVisibility{private value=0;begin(){return ++this.value;}invalidate(){this.value++;}active(value:number){return this.value===value;}}
export const faqMessages:Record<number,string>={
 [FaqReason.FAQ_REASON_RETRIEVED]:'已检索到相关FAQ原文，请核对原问题及适用条件后采用。',
 [FaqReason.FAQ_REASON_EXACT]:'已找到精确FAQ，答案来自当前发布资料。',
 [FaqReason.FAQ_REASON_NO_MATCH]:'没有足够匹配的FAQ或已核对依据。请换种问法或补充资料，本次没有调用AI。',
 [FaqReason.FAQ_REASON_AMBIGUOUS]:'存在多个匹配FAQ，需要先合并或更正，本次没有猜测答案。',
 [FaqReason.FAQ_REASON_STALE_CONTEXT]:'资料版本已变化，请保存场次的最新引用并重新发布后查询。',
 [FaqReason.FAQ_REASON_EXPIRED]:'建议已到期，请重新查询。',
 [FaqReason.FAQ_REASON_REVOKED]:'资料许可或事实已失效，旧建议不能采用。',
 [FaqReason.FAQ_REASON_CANCELLED]:'任务已取消。',
 [FaqReason.FAQ_REASON_RESOLVED]:'本条建议已处理。',
};
export function canAdopt(result:FoundationResult){return result.taskState===AgentTaskState.AGENT_TASK_STATE_COMPLETED&&result.suggestionState===SuggestionState.SUGGESTION_STATE_PROPOSED&&[FaqReason.FAQ_REASON_EXACT,FaqReason.FAQ_REASON_RETRIEVED].includes(result.faqReason);}
export async function faqDocument(result:FoundationResult,snapshotId:string,snapshotSha:string):Promise<FaqDocument|null>{
 if(!result.body){if(canAdopt(result))throw new ApiError('PROTOCOL_ERROR',true);return null;}
 const value=JSON.parse(result.body) as FaqDocument;
 const digest=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(result.body)))).map(b=>b.toString(16).padStart(2,'0')).join('');
 if(digest!==result.sha256||value.kind!=='publication_faq_task'||value.snapshot.object_id!==snapshotId||value.snapshot.sha256!==snapshotSha||value.target!=='private_workbench'||value.provider_calls!==0||value.api_cost_minor!==0||value.currency!=='CNY'||!Number.isSafeInteger(value.expires_at_ms)||value.expires_at_ms<=Date.now()||value.expires_at_ms>Date.now()+305000||!Array.isArray(value.evidence)||value.evidence.length>32||!Array.isArray(value.fact_ids)||value.fact_ids.length>32||typeof value.text!=='string'||value.text.length>2048)throw new ApiError('PROTOCOL_ERROR',true);
 if(canAdopt(result)&&(value.result!==(result.faqReason===FaqReason.FAQ_REASON_RETRIEVED?'retrieved':'exact')||!value.text||!value.fact_ids.length||value.evidence.length!==value.fact_ids.length||value.fact_ids.some(id=>!value.evidence.some(f=>f.fact_id===id&&f.source.trim()))))throw new ApiError('PROTOCOL_ERROR',true);
 if(!canAdopt(result)&&(value.text||value.evidence.length||value.fact_ids.length))throw new ApiError('PROTOCOL_ERROR',true);
 if(value.result==='retrieved'||value.retrieval){const r=value.retrieval;if(!r||r.strategy!=='faq-lexical-zh-v1'||r.method!==(value.result==='retrieved'?'lexical':'exact')||typeof r.matched_question!=='string'||!r.matched_question.trim()||r.matched_question.length>256||!Number.isInteger(r.score)||r.score<500||r.score>1000)throw new ApiError('PROTOCOL_ERROR',true);}
 return value;
}
