import fs from 'node:fs';
import {pathToFileURL} from 'node:url';

// A lower bound can reject a budget; it cannot prove an end-to-end pass.
export function assessFreshReply(summary) {
  const errors = [];
  const rows = [];
  if (summary.failure || !summary.shutdown) errors.push('execution_failed');
  if (summary.llm_included !== false || summary.simultaneous_cached_playback_and_generation !== false)
    errors.push('scope_mismatch');
  if (!Array.isArray(summary.rows) || summary.rows.length !== 2) errors.push('incomplete_trials');
  for (const row of summary.rows ?? []) {
    const report = row.report;
    const sentence = report?.results?.[0];
    const progress = sentence?.generation_progress;
    const first = sentence?.device_first_played_script_ms;
    if (!row.same_worker || report?.status !== 'completed' || report?.model_api_calls !== 1
        || report?.results?.length !== 1 || sentence?.prepared_asset !== false
        || !(sentence?.samples > 0) || sentence?.samples !== sentence?.played_samples
        || row.resumed?.status !== 'completed' || row.resumed?.model_api_calls !== 0
        || !Number.isFinite(first) || first < 0) errors.push(`invalid_trial_${row.request}`);
    const duration = sentence?.samples / 24;
    const generation = progress?.completed_ms - progress?.started_ms;
    rows.push({request:row.request, first_sample_lower_bound_ms:first ?? null,
      first_budget_rejected:Number.isFinite(first) ? first > 3000 : null,
      audio_duration_ms:Number.isFinite(duration) ? duration : null,
      generation_wall_ms:Number.isFinite(generation) ? generation : null,
      generation_supply_rtf:Number.isFinite(generation) && duration > 0 ? generation / duration : null,
      compute_rtf:Number.isFinite(progress?.pipeline_compute_wall_ms) && duration > 0
        ? progress.pipeline_compute_wall_ms / duration : null});
  }
  return {scope:'fresh_text_lower_bound_not_full_ai_mixed_load',
    functional_passed:errors.length === 0, errors, rows,
    fresh_text_3s_rejected:errors.length ? null : rows.some(r=>r.first_budget_rejected),
    full_ai_feasibility:'not_demonstrated', concurrent_mixed_load:'not_implemented'};
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const result = assessFreshReply(JSON.parse(fs.readFileSync(process.argv[2],'utf8')));
  console.log(JSON.stringify(result,null,2));
  if (!result.functional_passed) process.exitCode=1;
}
