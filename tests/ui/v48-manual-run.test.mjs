import test from 'node:test';import assert from 'node:assert/strict';
import {moveManual,endManual,markManual} from '../../apps/workbench-desktop/ui/manual-run.ts';
const doc=()=>({status:'open',cursor:0,cue_ids:['first','second'],marks:[],mode:'private_manual',evidence:'self_reported'});
test('manual advance stops old display, commits checkpoint, then explicitly presents new cue',async()=>{
 const events=[];await moveManual(doc(),1,{stop:async()=>events.push('stop'),save:async d=>events.push('save:'+d.cursor),show:async c=>events.push('show:'+c)});assert.deepEqual(events,['stop','save:1','show:second']);
});
test('failed stop or checkpoint save never displays new or previous text',async()=>{
 for(const failure of ['stop','save']){const events=[];const port=n=>async()=>{events.push(n);if(n===failure)throw Error(failure);};await assert.rejects(moveManual(doc(),1,{stop:port('stop'),save:port('save'),show:port('show')}));assert.equal(events.includes('show'),false);}
});
test('failed display keeps committed checkpoint and never retries',async()=>{
 let saved,shows=0;await assert.rejects(moveManual(doc(),1,{stop:async()=>{},save:async d=>saved=d,show:async()=>{shows++;throw Error('revoked');}}),/revoked/);assert.equal(saved.cursor,1);assert.equal(shows,1);
});
test('end stops first; save failure cannot restart output or claim persisted completion',async()=>{
 const events=[];await assert.rejects(endManual(doc(),{stop:async()=>events.push('stop'),save:async d=>{assert.equal(d.status,'ended');events.push('save');throw Error('conflict');}}),/conflict/);assert.deepEqual(events,['stop','save']);assert.equal(doc().status,'open');
});
test('ended and out-of-range records cannot trigger any output operation',async()=>{
 for(const [d,i] of [[{...doc(),status:'ended'},0],[doc(),-1],[doc(),2],[doc(),0.5]]){let calls=0;const p=async()=>{calls++;};await assert.rejects(moveManual(d,i,{stop:p,save:p,show:p}));assert.equal(calls,0);}
});
test('manual notes are bounded, attributed to selected cue and never mutate original',()=>{
 const d=doc(),r=markManual(d,' 改进说明 ','one');assert.equal(d.marks.length,0);assert.deepEqual(r.marks,[{id:'one',cue_id:'first',note:'改进说明'}]);for(const text of [' ','x'.repeat(257)])assert.throws(()=>markManual(d,text,'two'));assert.throws(()=>markManual({...d,marks:Array(8).fill({id:'m'})},'more','two'));assert.throws(()=>markManual(r,'duplicate','one'));
});

