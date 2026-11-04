import {ttsForm,ttsPrice,ttsPresets} from '../../apps/workbench-desktop/ui/tts.ts';
import test from 'node:test';
import assert from 'node:assert/strict';
import {webcrypto} from 'node:crypto';
import {modelIsSaved,modelTestMessage,modelTestText,minor,yuan,modelForm,draftCard,isRunning,localCheckReady,localCheckMessage} from '../../apps/workbench-desktop/ui/llm.ts';
globalThis.crypto??=webcrypto;
test('local readiness is not inferred from saved credentials, an estimate, or an unknown reason',()=>{
 const c={reason:1,estimateKnown:true,credentialChecked:true,estimatedReserveMinor:'2',availableMinor:'2'};
 assert.equal(localCheckReady(c),true);assert.match(localCheckMessage(c),/供应商.*仍未验证/);
 for(const patch of [{reason:0},{reason:2},{reason:6},{reason:99},{credentialChecked:false},{availableMinor:'1'},{estimatedReserveMinor:'0'}])assert.equal(localCheckReady({...c,...patch}),false);
 assert.match(localCheckMessage({...c,reason:3}),/上限不足/);assert.match(localCheckMessage({...c,reason:4}),/未知费用/);assert.match(localCheckMessage({...c,reason:6}),/绑定已失效/);
});
test('yuan cost ceilings use exact decimal money and require an explicit estimate confirmation',()=>{
 assert.equal(minor('0.01'),'1');assert.equal(minor('12.3'),'1230');assert.equal(yuan('1230'),'12.30');
 for(const x of ['-1','1e3','0.001','NaN','1,000'])assert.throws(()=>minor(x));
 const m=modelForm();assert.equal(m.key,'');assert.equal(m.confirmed,false);assert.equal(m.inputPrice,'');
 const existing=modelForm({id:m.id,revision:'1',name:'已存模型',baseUrl:'https://example.invalid/v1',model:'test',inputMinorPerMillion:'10',outputMinorPerMillion:'20',credentialSaved:true,enabled:true});
 assert.equal(existing.key,'');assert.equal(existing.confirmed,true);assert.equal(existing.preset,'custom');
});
test('private drafting separates running, review, adopted and expired result visibility',()=>{
 const id='00000000-0000-4000-8000-000000000001',sha='a'.repeat(64);
 const v={kind:'publication_llm_task',target:'private_workbench',text:'标准款500毫升',fact_ids:['capacity'],snapshot:{object_id:id,sha256:sha},expires_at_ms:Date.now()+30000};
 const r={taskState:5,suggestionState:1,body:JSON.stringify(v)};
 assert.equal(draftCard(r,id,sha).text,v.text);assert.equal(isRunning({taskState:3}),true);assert.equal(isRunning(r),false);
 for(const p of [{input:'private prompt'},{target:'live'},{text:''},{fact_ids:[]},{fact_ids:['capacity','capacity']},{expires_at_ms:Date.now()-1}])assert.throws(()=>draftCard({...r,body:JSON.stringify({...v,...p})},id,sha));
 assert.throws(()=>draftCard(r,id,'b'.repeat(64)));assert.throws(()=>draftCard({...r,suggestionState:2},id,sha));assert.equal(draftCard({taskState:6,suggestionState:5,body:''},id,sha),null);
});

test('model testing uses saved configuration and finite response evidence',()=>{
 const m={id:'00000000-0000-4000-8000-000000000001',revision:'2',name:'模型',baseUrl:'https://example.invalid/v1',model:'test',inputMinorPerMillion:'100',outputMinorPerMillion:'200',enabled:true,credentialSaved:true};const f=modelForm(m);assert.equal(modelIsSaved(f,m),true);
 for(const patch of [{key:'unsaved-secret'},{model:'other'},{revision:'1'},{inputPrice:'9.99'},{url:'https://other.invalid/v1'}])assert.equal(modelIsSaved({...f,...patch},m),false);
 assert.equal(modelIsSaved(f,{...m,enabled:false}),false);assert.equal(modelIsSaved(f,{...m,credentialSaved:false}),false);
 const r={taskState:5,taskReason:'API_RESPONSE_OK',body:JSON.stringify({kind:'publication_llm_test',target:'private_workbench',text:'NAI_OK'})};assert.equal(modelTestText(r),'NAI_OK');assert.match(modelTestMessage(r),/成功/);
 assert.match(modelTestMessage({...r,taskReason:'RESPONSE_EMPTY_CONTENT'}),/没有返回/);assert.match(modelTestMessage({...r,taskReason:'RESPONSE_TRUNCATED'}),/上限/);assert.match(modelTestMessage({...r,taskReason:'HTTP_401'}),/认证失败/);
 assert.equal(modelTestText({...r,taskState:8}),'');assert.throws(()=>modelTestText({...r,body:JSON.stringify({kind:'publication_llm_task',text:'wrong'})}));
});


test('speech model presets keep unknown prices distinct from confirmed free pricing',()=>{
 const f=ttsForm();assert.equal(f.model,ttsPresets[0].model);assert.equal(f.voice,ttsPresets[0].voice);assert.equal(f.key,'');assert.deepEqual(ttsPrice(f),{priceKnown:false,characterMinorPerMillion:'0'});
 assert.throws(()=>ttsPrice({...f,price:'0'}));assert.deepEqual(ttsPrice({...f,price:'0',priceConfirmed:true}),{priceKnown:true,characterMinorPerMillion:'0'});assert.deepEqual(ttsPrice({...f,price:'1.25',priceConfirmed:true}),{priceKnown:true,characterMinorPerMillion:'125'});
 assert.throws(()=>ttsPrice({...f,price:'0.001',priceConfirmed:true}));
 const saved=ttsForm({id:f.id,revision:'2',name:'语音',baseUrl:f.url,model:f.model,voice:f.voice,enabled:true,credentialSaved:true,priceKnown:false,characterMinorPerMillion:'0'});assert.equal(saved.key,'');assert.equal(saved.credentialSaved,true);assert.equal(saved.price,'');
 const custom=ttsForm({id:f.id,revision:'3',name:'自定义',baseUrl:'https://example.invalid/v1',model:'custom',voice:'voice-other',enabled:false,credentialSaved:false,priceKnown:true,characterMinorPerMillion:'200'});assert.equal(custom.preset,'custom');assert.equal(custom.customVoice,true);assert.equal(custom.price,'2.00');
});


