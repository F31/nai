import test from 'node:test';
import assert from 'node:assert/strict';
import {Api} from '../../apps/workbench-desktop/ui/api.ts';
import {FoundationResponse} from '../../packages/contracts-ts/dist/index.js';
const id='00000000-0000-4000-8000-000000000001';
const valid={artifactId:id,sampleCount:'24000',sampleRate:24000,pcmSha256:'aa'.repeat(32),leaseEpoch:'123',ttlMs:500};
const wire=ttsAudio=>Array.from(FoundationResponse.encode(FoundationResponse.fromPartial({requestId:id,seq:'1',outcome:{$case:'result',result:{ttsAudio}}})).finish());
test('audio authorization stays metadata-only and malformed lease boundaries are refused',async()=>{
 let calls=0;const api=new Api(async()=>{calls++;return wire(valid);});const r=await api.open();assert.equal(r.ttsAudio.ttlMs,500);assert.equal(calls,1);assert.equal(r.body,'');
 for(const patch of [{ttlMs:0},{ttlMs:501},{sampleCount:'0'},{sampleCount:'480001'},{sampleRate:48000},{leaseEpoch:'0'},{pcmSha256:'bad'},{artifactId:'../../file'}]){
  const bad=new Api(async()=>wire({...valid,...patch}));await assert.rejects(()=>bad.open(),/PROTOCOL_ERROR/);
 }
});

test('native audition status is bounded and submission is distinct from a hardware ack',async()=>{
 const response=audition=>Array.from(FoundationResponse.encode(FoundationResponse.fromPartial({requestId:id,seq:'1',outcome:{$case:'result',result:{audition}}})).finish());
 const good={state:4,reason:'SUBMITTED_NOT_DEVICE_ACK',submittedSamples:'24000'};
 const api=new Api(async()=>response(good));assert.equal((await api.open()).audition.reason,good.reason);
 for(const patch of [{state:0},{state:7},{reason:'x'.repeat(65)},{submittedSamples:'480001'}]){
  const bad=new Api(async()=>response({...good,...patch}));await assert.rejects(()=>bad.open(),/PROTOCOL_ERROR/);
 }
});

test('published speech plan rejects wrong target, text length, fee and expiration',async()=>{
 const good={approvalSha256:'ab'.repeat(32),text:'已发布',characters:'3',reserveMinor:'1',expiresAtMs:String(Date.now()+30000),target:'private_native_audio',voice:'test:voice'};
 const response=p=>Array.from(FoundationResponse.encode(FoundationResponse.fromPartial({requestId:id,seq:'1',outcome:{$case:'result',result:{speechPlan:p}}})).finish());
 assert.equal((await new Api(async()=>response(good)).open()).speechPlan.text,good.text);
 for(const patch of [{approvalSha256:'bad'},{target:'public'},{characters:'4'},{reserveMinor:'0'},{expiresAtMs:'1'},{text:'中'.repeat(257),characters:'257'}])await assert.rejects(()=>new Api(async()=>response({...good,...patch})).open(),/PROTOCOL_ERROR/);
});

test('device choices are session-bound, bounded and cannot claim audio submission before selection',async()=>{
 const good={state:7,reason:'',submittedSamples:'0',outputSessionId:'ab'.repeat(32),devices:[{deviceId:1,name:'耳机',isDefault:true}],selectedDeviceName:''};
 const response=a=>Array.from(FoundationResponse.encode(FoundationResponse.fromPartial({requestId:id,seq:'1',outcome:{$case:'result',result:{audition:a}}})).finish());
 assert.equal((await new Api(async()=>response(good)).open()).audition.devices[0].name,'耳机');
 for(const patch of [{outputSessionId:''},{devices:[]},{submittedSamples:'1'},{devices:[...good.devices,...good.devices]},{devices:[{deviceId:17,name:'bad'}]},{devices:[{deviceId:1,name:'中'.repeat(50)}]}])await assert.rejects(()=>new Api(async()=>response({...good,...patch})).open(),/PROTOCOL_ERROR/);
});

