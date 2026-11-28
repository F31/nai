import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {assess} from '../../scripts/assess-v561-experience.mjs';
const budget = JSON.parse(readFileSync(new URL('./experience-budget.json', import.meta.url)));
function fixture() {
  const rows = [0, 1, 2].map(index => ({status: 'completed', sentence_index: index + 1,
    samples: 48000, played_samples: 48000, frames: 50, shown_frames: 50,
    device_process_id: 12, worker_pid: 34, epoch: index + 1, output_window_identity: '0x123', model_api_calls: 1,
    device_first_played_script_ms: 2800 + index * 2300,
    device_completed_script_ms: 4800 + index * 2300, source_adapter_first_pcm_ms: 1700,
    gap_from_previous_device_end_ms: 0}));
  return [{status: 'completed', failure_code: null, sentences: 3, completed_sentences: 3,
    model_api_calls: 3, output_window_creations: 1, device_open_count: 1,
    device_process_id: 12, worker_pid: 34, epoch: 3, samples: 144000, played_samples: 144000,
    ready_before_previous_device_end_count: 0, results: rows},
  {input_mode: 'prepared_text', llm_included: false, prepared_before_submit: true,
    submit_to_completed_ms: 9600}];
}
test('3 second boundary includes conservative submit overhead; 800ms is advisory', () => {
  const result = assess(...fixture(), budget);
  assert.equal(result.first_voice.upper_bound_ms, 3000);
  assert.equal(result.short_run_passed, true);
  assert.deepEqual(result.continuity.gaps_ms, [300, 300]);
  assert.equal(result.continuity.cumulative_silence_ms, 600);
  assert.equal(result.llm_included, false);
  assert.equal(result.long_run_accepted, false);
});
test('TTS packet success cannot mask slow first device sample', () => {
  const [report, observation] = fixture();
  observation.submit_to_completed_ms++;
  report.results[0].source_adapter_first_pcm_ms = 10;
  assert.equal(assess(report, observation, budget).first_voice.status, 'not_proven');
  for (const row of report.results) {
    row.device_first_played_script_ms += 300;
    row.device_completed_script_ms += 300;
  }
  observation.submit_to_completed_ms += 300;
  assert.equal(assess(report, observation, budget).first_voice.status, 'failed');
});
test('gap is recomputed rather than trusting zero gap or ready count', () => {
  const [report, observation] = fixture();
  report.results[1].device_first_played_script_ms++;
  report.ready_before_previous_device_end_count = 3;
  const result = assess(report, observation, budget);
  assert.equal(result.continuity.max_gap_ms, 301);
  assert.equal(result.short_run_passed, false);
});
test('missing timestamps and invalid submission clocks fail closed', () => {
  const [report, observation] = fixture();
  assert.throws(() => assess(report, null, budget), /SCOPE/);
  assert.throws(() => assess(report, {...observation, submit_to_completed_ms: 10}, budget), /TIME_ORDER/);
  delete report.results[1].device_first_played_script_ms;
  assert.throws(() => assess(report, observation, budget), /DEVICE_START/);
});
test('sample loss, overlapping clocks, extra devices and incomplete scripts are rejected', () => {
  for (const mutate of [r => r.results[1].played_samples--,
    r => r.results[1].device_first_played_script_ms = 4000,
    r => r.results[1].device_process_id++, r => r.completed_sentences--,
    r => r.results[1].epoch = 1]) {
    const [report, observation] = fixture(); mutate(report);
    assert.throws(() => assess(report, observation, budget));
  }
});
test('budget is configurable and unknown fields cannot silently relax acceptance', () => {
  assert.equal(assess(...fixture(), {...budget, ready_first_voice_ms: 2999}).short_run_passed, false);
  assert.throws(() => assess(...fixture(), {...budget, skip_gap: true}), /BUDGET/);
  assert.throws(() => assess(...fixture(), {...budget, sentence_gap_ms: -1}), /BUDGET/);
});
