import test from 'node:test';
import assert from 'node:assert/strict';
import {WorkspaceRemovalReview,requireInactiveWorkspace} from '../../apps/workbench-desktop/ui/workspace-removal-review.ts';
import {ApiError} from '../../apps/workbench-desktop/ui/api.ts';
const row={id:'other',revision:'4',hasContent:false,recycled:false};
test('current workspace is rejected before inspecting or writing, restore stays allowed',async()=>{
 let calls=0;const flow=new WorkspaceRemovalReview(()=>{});
 await flow.open(row,row.id,async()=>{calls++;return row;});
 assert.equal(calls,0);assert.equal(flow.state.phase,'failed');assert.match(flow.state.error,/正在使用/);
 assert.throws(()=>requireInactiveWorkspace('current','current',2),e=>e.code==='WORKSPACE_IN_USE');
 requireInactiveWorkspace('current','current',3);
});
test('dialog checking is immediate and cancellation suppresses late inspection',async()=>{
 let resolve;const seen=[];const flow=new WorkspaceRemovalReview(s=>seen.push(s.phase));
 const wait=flow.open(row,'current',()=>new Promise(r=>resolve=r));
 assert.equal(flow.state.phase,'checking');assert.ok(flow.cancel());resolve(row);await wait;
 assert.equal(flow.state.phase,'closed');assert.deepEqual(seen,['checking','closed']);
});
test('inspection failure is visible and cancel clears pending lock',async()=>{
 const flow=new WorkspaceRemovalReview(()=>{});
 await flow.open(row,'current',async()=>{throw new ApiError('DEADLINE_EXCEEDED');});
 assert.equal(flow.state.phase,'failed');assert.match(flow.state.error,/超时/);
 assert.ok(flow.cancel());assert.equal(flow.state.phase,'closed');
});
test('inspection never deletes; confirmation uses inspected revision and correct mode',async()=>{
 for(const hasContent of [false,true]){
  let writes=0;const flow=new WorkspaceRemovalReview(()=>{}),checked={...row,revision:'5',hasContent};
  await flow.open(row,'current',async()=>checked);assert.equal(writes,0);
  assert.ok(await flow.confirm('current',async(r,action)=>{writes++;assert.equal(r.revision,'5');assert.equal(action,hasContent?2:1);}));
  assert.equal(writes,1);assert.equal(flow.state.phase,'closed');
 }
});
test('selection change and submission failure prevent writes/retries and allow cancellation',async()=>{
 const flow=new WorkspaceRemovalReview(()=>{});await flow.open(row,'current',async()=>row);
 let writes=0;assert.equal(await flow.confirm(row.id,async()=>writes++),false);assert.equal(writes,0);
 assert.ok(flow.cancel());await flow.open(row,'current',async()=>row);
 assert.equal(await flow.confirm('current',async()=>{writes++;throw new ApiError('OUTCOME_UNKNOWN');}),false);
 assert.equal(flow.state.phase,'failed');assert.match(flow.state.error,/尚未确认/);
 await flow.confirm('current',async()=>writes++);assert.equal(writes,1);assert.ok(flow.cancel());
});
test('submitting prevents duplicate confirmation and Escape cancellation until completion',async()=>{
 const flow=new WorkspaceRemovalReview(()=>{});await flow.open(row,'current',async()=>row);
 let resolve,writes=0;const pending=flow.confirm('current',()=>{writes++;return new Promise(r=>resolve=r);});
 assert.equal(flow.state.phase,'submitting');assert.equal(flow.cancel(),false);
 assert.equal(await flow.confirm('current',async()=>writes++),false);assert.equal(writes,1);
 resolve();assert.ok(await pending);assert.equal(flow.state.phase,'closed');
});

