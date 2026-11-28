import test from 'node:test';
import assert from 'node:assert/strict';
import {assessFreshReply} from '../../scripts/assess-v561-fresh-reply.mjs';
function fixture(first=4000) {
  return {failure:null,shutdown:true,llm_included:false,simultaneous_cached_playback_and_generation:false,
    rows:[5,9].map(request=>({request,same_worker:true,resumed:{status:'completed',model_api_calls:0},
      report:{status:'completed',model_api_calls:1,results:[{prepared_asset:false,samples:48000,played_samples:48000,
        device_first_played_script_ms:first,generation_progress:{started_ms:100,completed_ms:3100,pipeline_compute_wall_ms:2000}}]}}))};
}
test('fresh lower bound rejects slow response without claiming full AI feasibility',()=>{
  const r=assessFreshReply(fixture());
  assert.equal(r.functional_passed,true); assert.equal(r.fresh_text_3s_rejected,true);
  assert.equal(r.rows[0].generation_supply_rtf,1.5);
  assert.equal(r.full_ai_feasibility,'not_demonstrated');
});
test('a fast text-only result never grants LLM or concurrent acceptance',()=>{
  const r=assessFreshReply(fixture(2000)); assert.equal(r.fresh_text_3s_rejected,false);
  assert.equal(r.full_ai_feasibility,'not_demonstrated'); assert.equal(r.concurrent_mixed_load,'not_implemented');
});
test('missing trials, stale worker, cached answers or partial samples reject evidence',()=>{
  for(const change of [s=>s.rows.pop(),s=>s.rows[0].same_worker=false,
    s=>s.rows[0].report.results[0].prepared_asset=true,
    s=>s.rows[0].report.results[0].played_samples--,s=>s.rows[0].report.results[0].device_first_played_script_ms=null]) {
    const s=fixture(); change(s); assert.equal(assessFreshReply(s).functional_passed,false);
  }
});
