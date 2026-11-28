import {readFileSync} from 'node:fs';
import {pathToFileURL} from 'node:url';

// Prepared content only. Never grant fresh-answer or long-soak acceptance.
export function assess(summary) {
 const failures=[],gaps=[],first=[];
 const integer=n=>Number.isSafeInteger(n)&&n>=0;
 const rounds=summary?.reports;
 if(!Array.isArray(rounds)||rounds.length<2) return {functional_passed:false,budget_passed:false,failures:['missing replay rounds']};
 let identities;
 for(const round of rounds){
  const s=round.script,rows=s?.sentences;
  if(!Array.isArray(rows)||rows.length<2||rows.length>8){failures.push('missing bounded multi-sentence timeline');continue;}
  if(s.scope!=='private_session_prepared_plan_playback'||s.status!=='completed'||s.model_api_calls!==0||s.output_device_count!==1||s.output_window_count!==1||!integer(s.device_pid)||s.device_pid===0)failures.push('invalid playback scope or device');
  if(!integer(round.submit_first_sample_upper_ms))failures.push('missing host first-sample upper bound');
  else first.push(round.submit_first_sample_upper_ms);
  let end,epoch=0,samples=0;
  for(const row of rows){
   const a=row.device_first_played_script_ms,b=row.device_completed_script_ms;
   if(!integer(a)||!integer(b)||b<a||!integer(row.epoch)||row.epoch<=epoch)failures.push('invalid device timeline or epoch');
   if(row.device_process_id!==s.device_pid||row.model_api_calls!==0||row.prepared_asset!==true||row.status!=='completed'||!integer(row.samples)||row.samples===0||row.samples!==row.played_samples)failures.push('incomplete actual-device content');
   if(end!==undefined){
    if(!integer(a)||!integer(end)||a<end)failures.push('overlapping or missing clock');
    else {const gap=a-end;gaps.push(gap);if(row.gap_from_previous_device_end_ms!==gap)failures.push('reported gap disagrees with device clock');}
   }
   samples+=row.samples;end=b;epoch=row.epoch;
  }
  if(samples!==s.samples||samples!==s.played_samples||!integer(s.elapsed_ms)||end>s.elapsed_ms)failures.push('incomplete aggregate');
  const ids=rows.map(r=>`${r.asset_identity}:${r.pcm_sha256}`);
  if(ids.some(id=>! /^[a-f0-9]{64}:[a-f0-9]{64}$/.test(id)))failures.push('missing content identity');
  if(identities&&JSON.stringify(ids)!==JSON.stringify(identities))failures.push('replay content changed');
  identities=ids;
 }
 const functional_passed=failures.length===0;
 return {scope:'prepared_plan_short_run_only',functional_passed,budget_passed:functional_passed&&first.every(n=>n<=3000)&&gaps.length>0&&gaps.every(n=>n<=300),first_upper_ms:first,gaps_ms:gaps,max_gap_ms:gaps.length?Math.max(...gaps):null,failures};
}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href){
 const result=assess(JSON.parse(readFileSync(process.argv[2],'utf8')));
 console.log(JSON.stringify(result,null,2));
 if(!result.functional_passed||!result.budget_passed)process.exitCode=1;
}
