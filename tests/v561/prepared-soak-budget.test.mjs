import test from 'node:test';
import assert from 'node:assert/strict';
import {assessSoak} from '../../scripts/assess-v561-prepared-soak.mjs';
function fixture(){
 const rounds=Array.from({length:120},(_,i)=>({request:i+3,observed_ms:14990,submit_first_sample_upper_ms:500,script:{scope:'private_session_prepared_plan_playback',status:'completed',model_api_calls:0,output_window_count:1,output_device_count:1,device_pid:i+20,samples:96000,played_samples:96000,elapsed_ms:14980,sentences:Array.from({length:4},(_,j)=>({epoch:i*4+j+1,worker_pid:10,device_process_id:i+20,status:'completed',model_api_calls:0,prepared_asset:true,samples:24000,played_samples:24000,device_first_played_script_ms:200+j*1200,device_completed_script_ms:1200+j*1200,gap_from_previous_device_end_ms:j?200:null,asset_identity:'a'.repeat(64),pcm_sha256:'b'.repeat(64)}))}}));
 return {summary:{scope:'prepared_plan_replay_soak_not_new_generation',status:'completed',shutdown:true,model_api_calls:5,replay_model_calls:0,elapsed_ms:1800000,rounds:120,total_samples:11520000,max_gap_ms:200,max_first_upper_ms:500,gap_budget_failures:0,first_budget_failures:0},rounds};
}
test('soak needs full duration, complete ledger, unique epochs and stable worker',()=>{
 const good=fixture();assert.equal(assessSoak(good.summary,good.rounds).budget_passed,true);
 for(const change of [x=>x.summary.elapsed_ms=1799999,x=>x.rounds.pop(),x=>x.rounds[1].script.sentences[0].epoch=1,x=>x.rounds[1].script.sentences[0].worker_pid=11,x=>x.summary.total_samples--]){
  const f=fixture();change(f);assert.equal(assessSoak(f.summary,f.rounds).functional_passed,false);
 }
});
test('budget failures must stay visible even when thirty minutes complete',()=>{
 const f=fixture();f.rounds[0].submit_first_sample_upper_ms=3001;
 assert.equal(assessSoak(f.summary,f.rounds).functional_passed,false);
 f.summary.first_budget_failures=1;f.summary.max_first_upper_ms=3001;
 assert.equal(assessSoak(f.summary,f.rounds).functional_passed,true);
 assert.equal(assessSoak(f.summary,f.rounds).budget_passed,false);
});
test('partial round records cannot grant endurance completion',()=>{
 const f=fixture();
 assert.equal(assessSoak(f.summary,[]).functional_passed,false);
 assert.equal(assessSoak(f.summary,[{request:3},{request:4}]).functional_passed,false);
});
