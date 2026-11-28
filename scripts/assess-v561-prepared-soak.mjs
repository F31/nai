import {readFileSync,statSync} from 'node:fs';
import {pathToFileURL} from 'node:url';
import {assess} from './assess-v561-prepared-plan.mjs';

export function assessSoak(summary, rounds) {
 const problems=[];
 if(!Array.isArray(rounds)||rounds.length<2||rounds.length>4096)return {functional_passed:false,budget_passed:false,problems:['missing bounded round ledger']};
 const short=assess({reports:rounds});
 const first=short.first_upper_ms??[], gaps=short.gaps_ms??[];
 if(!short.functional_passed)problems.push(...short.failures);
 if(summary?.scope!=='prepared_plan_replay_soak_not_new_generation'||summary.status!=='completed'||summary.shutdown!==true||summary.model_api_calls!==5||summary.replay_model_calls!==0)problems.push('incomplete or wrong execution scope');
 if(!Number.isSafeInteger(summary?.elapsed_ms)||summary.elapsed_ms<1800000)problems.push('less than thirty minutes');
 if(summary?.rounds!==rounds.length)problems.push('round ledger incomplete');
 let total=0,previousEpoch=0,observed=0,worker;
 for(const [index,round] of rounds.entries()){
  if(round.request!==index+3||!Number.isSafeInteger(round.observed_ms)||round.observed_ms<=0)problems.push('invalid request or host duration');
  observed+=round.observed_ms;
  const sentences=round.script?.sentences;
  if(!Array.isArray(sentences)||sentences.length!==4){problems.push('incomplete four-sentence plan');continue;}
  for(const row of sentences){
   if(!Number.isSafeInteger(row.epoch)||row.epoch<=previousEpoch)problems.push('old epoch reused across rounds');
   previousEpoch=row.epoch;
   if(!Number.isSafeInteger(row.worker_pid)||row.worker_pid<=0||worker!==undefined&&row.worker_pid!==worker)problems.push('worker changed or missing');
   worker=row.worker_pid;total+=row.played_samples;
  }
 }
 if(total!==summary?.total_samples||!Number.isSafeInteger(total))problems.push('aggregate samples disagree');
 if(observed>summary?.elapsed_ms)problems.push('host durations exceed run');
 const gapFailures=gaps.filter(n=>n>300).length;
 const firstFailures=first.filter(n=>n>3000).length;
 const maxFirst=first.length?Math.max(...first):null;
 if(summary?.gap_budget_failures!==gapFailures||summary?.first_budget_failures!==firstFailures||summary?.max_gap_ms!==short.max_gap_ms||summary?.max_first_upper_ms!==maxFirst)problems.push('summary hides budget deviations');
 const functional_passed=problems.length===0;
 return {scope:'thirty_minute_prepared_replay_soak_not_new_generation_or_single_device_gapless',functional_passed,budget_passed:functional_passed&&short.budget_passed,elapsed_ms:summary?.elapsed_ms,rounds:rounds.length,total_samples:total,worker_pid:worker,max_gap_ms:short.max_gap_ms,max_first_upper_ms:maxFirst,gap_budget_failures:gapFailures,first_budget_failures:firstFailures,inter_round_and_control_overhead_ms:summary?.elapsed_ms-observed,problems};
}
function read(path){if(statSync(path).size>33554432)throw Error('REPORT_TOO_LARGE');return readFileSync(path,'utf8');}
if(process.argv[1]&&import.meta.url===pathToFileURL(process.argv[1]).href){
 const [summary,ledger,output]=process.argv.slice(2);
 if(!summary||!ledger)throw Error('USAGE: summary.json rounds.jsonl [new-output.json]');
 const result=assessSoak(JSON.parse(read(summary)),read(ledger).trim().split('\n').map(line=>JSON.parse(line)));
 if(output){const {writeFileSync}=await import('node:fs');writeFileSync(output,JSON.stringify(result,null,2),{flag:'wx'});}
 console.log(JSON.stringify(result,null,2));
 if(!result.functional_passed||!result.budget_passed)process.exitCode=1;
}
