import test from 'node:test';
import assert from 'node:assert/strict';
import {webcrypto} from 'node:crypto';
import {FaqVisibility,canAdopt,faqDocument} from '../../apps/workbench-desktop/ui/faq.ts';
import {AgentTaskState,SuggestionState,FaqReason} from '../../packages/contracts-ts/dist/index.js';
globalThis.crypto??=webcrypto;
const id='00000000-0000-4000-8000-000000000001',sha='a'.repeat(64);
const value=()=>({kind:'publication_faq_task',snapshot:{object_id:id,sha256:sha},target:'private_workbench',provider_calls:0,api_cost_minor:0,currency:'CNY',expires_at_ms:Date.now()+300000,result:'exact',text:'容量500毫升',fact_ids:['capacity'],evidence:[{fact_id:'capacity',source:'规格页',conditions:'标准款'}]});
async function encoded(v){const body=JSON.stringify(v),digest=Buffer.from(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(body))).toString('hex');return{body,sha256:digest,taskState:AgentTaskState.AGENT_TASK_STATE_COMPLETED,suggestionState:SuggestionState.SUGGESTION_STATE_PROPOSED,faqReason:FaqReason.FAQ_REASON_EXACT};}
test('task completion alone grants no suggestion adoption',()=>{assert.equal(canAdopt({taskState:5,suggestionState:0,faqReason:2}),false);assert.equal(canAdopt({taskState:5,suggestionState:1,faqReason:1}),true);assert.equal(canAdopt({taskState:6,suggestionState:1,faqReason:1}),false);});
test('late queries are invalidated by cancel and preparation context switch',()=>{const gate=new FaqVisibility();const old=gate.begin();gate.invalidate();assert.equal(gate.active(old),false);const next=gate.begin();assert.equal(gate.active(next),true);assert.equal(gate.active(old),false);gate.invalidate();assert.equal(gate.active(next),false);});
test('candidate binds exact snapshot, bytes, source, target, expiry and zero API costs',async()=>{const good=await encoded(value());assert.equal((await faqDocument(good,id,sha)).text,'容量500毫升');await assert.rejects(faqDocument({...good,sha256:'0'.repeat(64)},id,sha));await assert.rejects(faqDocument(good,id,'b'.repeat(64)));for(const patch of [{target:'public'},{api_cost_minor:1},{provider_calls:1},{expires_at_ms:Date.now()-1},{evidence:[]},{fact_ids:[]},{evidence:[{fact_id:'capacity',source:'',conditions:''}]}])await assert.rejects(faqDocument(await encoded({...value(),...patch}),id,sha));});
test('cancelled and stale results cannot smuggle text or remain adoptable',async()=>{assert.equal(await faqDocument({body:'',taskState:7,suggestionState:5,faqReason:4},id,sha),null);const r=await encoded(value());await assert.rejects(faqDocument({...r,suggestionState:5},id,sha));await assert.rejects(faqDocument({...r,body:''},id,sha));});

