// Read-only short-run assessment. Device timestamps, not TTS packets, define voice/gaps.
import {readFileSync, writeFileSync, openSync, readSync, closeSync} from 'node:fs';
import {resolve} from 'node:path';
import {fileURLToPath} from 'node:url';
import Ajv from 'ajv';

const validateBudget = new Ajv({allErrors: true}).compile(JSON.parse(readFileSync(
  new URL('../schemas/tests/v561-experience-budget.schema.json', import.meta.url), 'utf8')));
const integer = (value, name, minimum = 0) => {
  if (!Number.isSafeInteger(value) || value < minimum) throw Error(`INVALID_${name}`);
  return value;
};

export function assess(report, observation, budget) {
  if (!validateBudget(budget)) throw Error('INVALID_BUDGET');
  if (observation?.input_mode !== 'prepared_text' || observation.llm_included !== false
      || observation.prepared_before_submit !== true) throw Error('SUBMISSION_SCOPE_MISSING');
  const submittedToCompleted = integer(observation.submit_to_completed_ms, 'SUBMISSION_DURATION', 1);
  const rows = report?.results;
  if (report?.status !== 'completed' || report.failure_code != null || !Array.isArray(rows)
      || rows.length < 2 || rows.length > 32 || report.completed_sentences !== rows.length
      || report.sentences !== rows.length || report.model_api_calls !== rows.length
      || report.output_window_creations !== 1 || report.device_open_count !== 1)
    throw Error('INCOMPLETE_SCRIPT');
  const pid = integer(report.device_process_id, 'DEVICE_PID', 1);
  const worker = integer(report.worker_pid, 'WORKER_PID', 1);
  const window = rows[0].output_window_identity;
  if (typeof window !== 'string' || !window.length) throw Error('WINDOW_ID_MISSING');
  const gaps = [];
  let previousEnd, totalSamples = 0, previousEpoch = 0;
  for (const [index, row] of rows.entries()) {
    if (row.status !== 'completed' || row.sentence_index !== index + 1
        || row.device_process_id !== pid || row.worker_pid !== worker
        || row.output_window_identity !== window || row.model_api_calls !== 1)
      throw Error('SENTENCE_IDENTITY');
    const samples = integer(row.samples, 'SAMPLES', 1);
    const epoch = integer(row.epoch, 'EPOCH', 1);
    if (epoch <= previousEpoch) throw Error('EPOCH_ORDER');
    previousEpoch = epoch;
    if (row.played_samples !== samples || !Number.isSafeInteger(totalSamples + samples))
      throw Error('SAMPLE_MISMATCH');
    integer(row.frames, 'FRAMES', 1);
    integer(row.shown_frames, 'SHOWN_FRAMES', 1);
    totalSamples += samples;
    const start = integer(row.device_first_played_script_ms, 'DEVICE_START');
    const end = integer(row.device_completed_script_ms, 'DEVICE_END', 1);
    if (end <= start || (previousEnd !== undefined && start < previousEnd))
      throw Error('DEVICE_TIME_ORDER');
    if (previousEnd !== undefined) gaps.push(start - previousEnd);
    previousEnd = end;
  }
  if (report.epoch !== previousEpoch) throw Error('SCRIPT_EPOCH_MISMATCH');
  if (report.samples !== totalSamples || report.played_samples !== totalSamples)
    throw Error('SCRIPT_SAMPLE_MISMATCH');
  // Completion observation is later than the final device timestamp. The excess
  // conservatively includes submit/IPC/startup and completion notification overhead.
  if (submittedToCompleted < previousEnd) throw Error('SUBMISSION_TIME_ORDER');
  const originUpperBound = submittedToCompleted - previousEnd;
  const firstVoiceUpperBound = originUpperBound + rows[0].device_first_played_script_ms;
  const maxGap = Math.max(...gaps);
  const firstPassed = firstVoiceUpperBound <= budget.ready_first_voice_ms;
  const continuousPassed = maxGap <= budget.sentence_gap_ms;
  return {
    version: 1, scope: 'bounded_prepared_text_device_experience', input_mode: 'prepared_text',
    llm_included: false, long_run_accepted: false, model_api_calls: report.model_api_calls,
    sentences: rows.length, audio_seconds: totalSamples / budget.sample_rate,
    first_voice: {status: firstPassed ? 'passed'
      : rows[0].device_first_played_script_ms > budget.ready_first_voice_ms ? 'failed' : 'not_proven',
      lower_bound_ms: rows[0].device_first_played_script_ms,
      upper_bound_ms: firstVoiceUpperBound, budget_ms: budget.ready_first_voice_ms,
      submit_to_script_origin_upper_bound_ms: originUpperBound},
    continuity: {status: continuousPassed ? 'passed' : 'failed', budget_ms: budget.sentence_gap_ms,
      gaps_ms: gaps, max_gap_ms: maxGap, cumulative_silence_ms: gaps.reduce((a, b) => a + b, 0),
      last_minus_first_gap_ms: gaps.at(-1) - gaps[0]},
    tts_first_pcm: {role: 'optimization_only', goal_ms: budget.tts_first_pcm_goal_ms,
      observed_ms: rows.map(row => Number.isSafeInteger(row.source_adapter_first_pcm_ms)
        && row.source_adapter_first_pcm_ms >= 0 ? row.source_adapter_first_pcm_ms : null)},
    ready_before_previous_device_end_count: report.ready_before_previous_device_end_count ?? null,
    short_run_passed: firstPassed && continuousPassed,
    note: 'Device callback evidence; not acoustic measurement, human review, LLM, OBS or long-soak acceptance.',
  };
}

function readJson(path, maximum) {
  const fd = openSync(path, 'r');
  try {
    const bytes = Buffer.alloc(maximum + 1);
    let size = 0, received;
    while (size <= maximum && (received = readSync(fd, bytes, size, bytes.length - size, null)) > 0)
      size += received;
    if (size > maximum) throw Error('INPUT_TOO_LARGE');
    return JSON.parse(bytes.subarray(0, size).toString('utf8').replace(/^\uFEFF/, ''));
  } finally { closeSync(fd); }
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const [report, observation, output, budget = 'tests/v561/experience-budget.json', ...extra] = process.argv.slice(2);
    if (!output || extra.length) throw Error('USAGE: report.json observation.json new-output.json [budget.json]');
    const result = assess(readJson(report, 4 * 1024 * 1024), readJson(observation, 4096), readJson(budget, 4096));
    writeFileSync(output, JSON.stringify(result, null, 2) + '\n', {flag: 'wx'});
    console.log(JSON.stringify(result, null, 2));
    if (!result.short_run_passed) process.exitCode = 1;
  } catch (error) {
    console.error(`experience assessment: ${error.message}`);
    process.exitCode = 2;
  }
}
