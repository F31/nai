import test from 'node:test';
import assert from 'node:assert/strict';
import {webcrypto} from 'node:crypto';
import {Api,previewCard} from '../../apps/workbench-desktop/ui/api.ts';
import {emptyProduct,toProduct,fromProduct,yuanToMinor,minorToYuan,updateFact} from '../../apps/workbench-desktop/ui/forms.ts';
import {FoundationResponse,FoundationCommand,DeliveryState} from '../../packages/contracts-ts/dist/index.js';
globalThis.crypto??=webcrypto;
const id='00000000-0000-4000-8000-000000000001';
const wire=result=>Array.from(FoundationResponse.encode(FoundationResponse.fromPartial({requestId:id,seq:'1',outcome:{$case:'result',result}})).finish());
test('money is exact and ambiguous or excessive precision inputs are rejected',()=>{assert.equal(yuanToMinor('129.90'),'12990');assert.equal(minorToYuan('12990'),'129.90');for(const value of ['-1','1.001','1e2','01','NaN'])assert.throws(()=>yuanToMinor(value));});
test('form round trip preserves fact validity and never invents confirmation or bindings',()=>{const p=emptyProduct();p.name='杯子';p.facts[0]={...p.facts[0],label:'容量',value:'500',type:'decimal',unit:'毫升',source:'规格页',from:'2026-10-22T00:00:00.123',until:'2027-10-22T00:00:00.456'};p.talk_tracks[0].text='容量500毫升';let doc=toProduct(p);assert.equal(doc.facts[0].authority,'unverified');assert.deepEqual(doc.talk_tracks[0].fact_ids,[]);assert.deepEqual(toProduct(fromProduct(doc)),doc);p.talk_tracks[0].fact_ids=[p.facts[0].id];doc=toProduct(p);assert.deepEqual(doc.talk_tracks[0].fact_ids,[p.facts[0].id]);});
test('writes use one authenticated command and do not retry uncertain outcomes',async()=>{let calls=0;const api=new Api(async(name,args)=>{calls++;assert.equal(name,'foundation_command');assert.equal(FoundationCommand.decode(Uint8Array.from(args.command)).operation.$case,'putPreparation');throw Error('WB_OUTCOME_UNKNOWN');});await assert.rejects(api.save(id,id,'0',{}),/OUTCOME_UNKNOWN/);assert.equal(calls,1);assert.equal(api.pending,false);});
test('workspace paging is bounded and malformed result identities are rejected',async()=>{let calls=0;const api=new Api(async()=>wire({resources:[{id,name:'工作区'}],nextCursor:++calls===1?id:''}));assert.equal((await api.workspaces()).length,2);assert.equal(calls,2);const bad=new Api(async()=>wire({resources:[{id:'invalid'}]}));await assert.rejects(bad.open(),/PROTOCOL_ERROR/);});
test('preview requires matching bytes, private target, current TTL and delivering state',async()=>{const body=JSON.stringify({kind:'publication_private_preview',target:'private_workbench',text:'容量500毫升',session_id:id,epoch:1,expires_at_ms:Date.now()+60000,cue_id:'opening'});const sha=Buffer.from(await crypto.subtle.digest('SHA-256',new TextEncoder().encode(body))).toString('hex');const result={body,sha256:sha,state:DeliveryState.DELIVERY_STATE_DELIVERING,epoch:'1'};assert.equal((await previewCard(result)).text,'容量500毫升');await assert.rejects(previewCard({...result,sha256:'0'.repeat(64)}));await assert.rejects(previewCard({...result,state:DeliveryState.DELIVERY_STATE_CANCELLED}));});

test('fact edits clear prior confirmation until explicitly checked again',()=>{const f={...emptyProduct().facts[0],confirmed:true};for(const patch of [{value:'600'},{source:'新来源'},{until:'2027-01-01T00:00'}])assert.equal(updateFact(f,patch).confirmed,false);assert.equal(updateFact(f,{confirmed:true}).confirmed,true);});


