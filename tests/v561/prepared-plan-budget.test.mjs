import test from 'node:test';
import assert from 'node:assert/strict';
import {assess} from '../../scripts/assess-v561-prepared-plan.mjs';
function input(){
 const rows=[1,2].map((epoch,i)=>({epoch,device_process_id:7,model_api_calls:0,prepared_asset:true,status:'completed',samples:24000,played_samples:24000,device_first_played_script_ms:200+i*1200,device_completed_script_ms:1200+i*1200,gap_from_previous_device_end_ms:i?200:null,asset_identity:'a'.repeat(64),pcm_sha256:'b'.repeat(64)}));
 return {reports:[1,2].map(()=>({submit_first_sample_upper_ms:250,script:{scope:'private_session_prepared_plan_playback',status:'completed',model_api_calls:0,output_device_count:1,output_window_count:1,device_pid:7,samples:48000,played_samples:48000,elapsed_ms:2500,sentences:structuredClone(rows)}}))};
}
test('prepared assessment requires actual full device timelines and repeated content',()=>{
 assert.equal(assess(input()).budget_passed,true);
 for(const alter of [s=>delete s.reports[0].script.sentences[0].device_first_played_script_ms,s=>s.reports[0].script.sentences[1].played_samples=1,s=>s.reports[1].script.sentences[0].pcm_sha256='c'.repeat(64),s=>s.reports[0].script.model_api_calls=1]){
  const s=input();alter(s);assert.equal(assess(s).functional_passed,false);
 }
});
test('functional completion does not override first-sample or gap budget',()=>{
 let s=input();s.reports[0].submit_first_sample_upper_ms=3001;assert.equal(assess(s).functional_passed,true);assert.equal(assess(s).budget_passed,false);
 s=input();const row=s.reports[0].script.sentences[1];row.device_first_played_script_ms+=101;row.gap_from_previous_device_end_ms+=101;assert.equal(assess(s).functional_passed,true);assert.equal(assess(s).budget_passed,false);
});
