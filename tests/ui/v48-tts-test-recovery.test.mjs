import test from 'node:test';
import assert from 'node:assert/strict';
import {submitTtsTest,TtsWaitError,ttsDeadline,TtsActionGate} from '../../apps/workbench-desktop/ui/tts-test-flow.ts';
const deferred=()=>{let resolve,reject;const promise=new Promise((a,b)=>{resolve=a;reject=b;});return{promise,resolve,reject};};
test('same-tick duplicate clicks cannot dispatch twice and a settled wait releases the local gate',async()=>{const gate=new TtsActionGate(),d=deferred();let calls=0;const first=gate.run(()=>{calls++;return d.promise;});await gate.run(async()=>{calls++;});assert.equal(calls,1);d.resolve();await first;assert.equal(gate.busy,false);await assert.rejects(gate.run(async()=>{throw Error('no response');}),/no response/);assert.equal(gate.busy,false);});
test('a stuck stop settles the UI wait and never dispatches, including after a late stop',async()=>{
 const stop=deferred();let calls=0;await assert.rejects(submitTtsTest(()=>stop.promise,()=>true,async()=>{calls++;},()=>{},()=>{},5),e=>e instanceof TtsWaitError&&e.stage==='stop');
 stop.resolve();await new Promise(resolve=>setImmediate(resolve));assert.equal(calls,0);
});
test('navigation during preparation prevents a paid dispatch',async()=>{
 const stop=deferred();let active=true,calls=0;const wait=submitTtsTest(()=>stop.promise,()=>active,async()=>{calls++;},()=>{},()=>{},100);active=false;stop.resolve();await assert.rejects(wait,/已离开/);assert.equal(calls,0);
});
test('unknown submission is sent once and a late acknowledgment stays on the original task',async()=>{
 const remote=deferred();let sends=0,marked=0;const accepted=[];await assert.rejects(submitTtsTest(async()=>{},()=>true,()=>{sends++;return remote.promise;},()=>marked++,v=>accepted.push(v),5),e=>e instanceof TtsWaitError&&e.stage==='submit');
 assert.equal(sends,1);assert.equal(marked,1);assert.deepEqual(accepted,[]);remote.resolve({resourceId:'original-task'});await new Promise(resolve=>setImmediate(resolve));assert.deepEqual(accepted,[{resourceId:'original-task'}]);assert.equal(sends,1);
});
test('normal reply is accepted once and errors do not retry or manufacture success',async()=>{
 const values=[];assert.equal(await submitTtsTest(async()=>{},()=>true,async()=>42,()=>{},v=>values.push(v),100),42);assert.deepEqual(values,[42]);
 let calls=0;await assert.rejects(submitTtsTest(async()=>{},()=>true,async()=>{calls++;throw Error('offline');},()=>{},()=>assert.fail('unexpected audio'),100),/offline/);assert.equal(calls,1);
});
test('read-only recovery is bounded without changing a task or dispatching synthesis',async()=>{await assert.rejects(ttsDeadline(new Promise(()=>{}),'status',5),e=>e.stage==='status');});
