// Launch the real assembled engine (single storage owner, shared session,
// loopback control surface) and drive it with the desktop ControlClient.
// The credential is generated in memory, written to an owner-only file and
// never printed or committed.
import assert from 'node:assert/strict';
import { randomBytes, randomUUID } from 'node:crypto';
import { spawn } from 'node:child_process';
import { mkdtemp, writeFile, chmod, mkdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { once } from 'node:events';
import { setTimeout as delay } from 'node:timers/promises';
import { ControlClient } from '../apps/desktop/src/control.ts';

const root = await mkdtemp(join(tmpdir(), 'nai-engine-smoke-'));
const dataRoot = join(root, 'data');
const halt = join(root, 'halt');
await mkdir(dataRoot, { recursive: true });
const token = randomBytes(32).toString('hex');
const tokenFile = join(root, 'token');
await writeFile(tokenFile, token, { mode: 0o600 });
await chmod(tokenFile, 0o600);

const profile = join(root, 'runtime-profile.json');
await writeFile(profile, JSON.stringify({
  schema_version: 1, edition: 'standard', deployment: 'local', workspace_id: 'ws-smoke',
  runtime: { max_active_sessions: 4, plugin_catalog: 'catalog-local', control_listener: 'named_pipe', http_enabled: false },
  bindings: {},
  session_defaults: { mode: 'rehearsal', allow_auto_without_platform_grant: false, viewer_memory_enabled: false },
  scheduler: { max_pending_utterances: 8, max_pending_speech_ms: 60000, qa_ttl_ms: 60000, welcome_ttl_ms: 60000, thanks_ttl_ms: 60000 },
  media: { lease_renew_ms: 100, lease_expire_ms: 600, output_sample_rate: 48000, model_audio_format: 'from_model_manifest', startup_buffer_ms: 100, max_buffer_ms: 5000 },
  knowledge: { mode: 'local', fts_enabled: true, semantic_enabled: false, remote_source_refs: [], index_during_live: 'disabled' },
  budgets: { soft_threshold_ratio: 0.8, hard_stop_new_paid_requests: true, currency: 'CNY', per_session_limit_minor: '1000' },
  cache: { tts_max_bytes: 0, invalidate_on_fact_change: true },
  cloud: { control_enabled: false, relay_enabled: false, render_enabled: false },
}));

const exe = `target/debug/nai-engine${process.platform === 'win32' ? '.exe' : ''}`;
const base = ['--data-root', dataRoot, '--workspace', 'ws-smoke', '--actor', 'root-owner', '--profile', 'default'];

const provisioned = spawn(exe, [...base, '--provision', profile], { stdio: ['ignore', 'pipe', 'pipe'] });
let provisionOut = '';
provisioned.stdout.on('data', (data) => { provisionOut += data.toString(); });
provisioned.stderr.on('data', (data) => { provisionOut += data.toString(); });
const [provisionCode] = await once(provisioned, 'exit');
assert.equal(provisionCode, 0, `provisioning failed: ${provisionOut}`);
assert.match(provisionOut, /nai-engine provisioned/);

const child = spawn(exe, [...base, '--token-file', tokenFile, '--halt-file', halt, '--control-port', '0'], {
  stdio: ['ignore', 'pipe', 'pipe'],
});
let output = '';
let failure;
child.stdout.on('data', (data) => { output += data.toString(); });
child.stderr.on('data', (data) => { output += data.toString(); });
child.on('error', (error) => { failure = error; });
const exited = once(child, 'exit').catch(() => []);
const watchdog = setTimeout(() => child.kill(), 30000);
try {
  let match;
  for (let attempt = 0; attempt < 200; attempt++) {
    if (failure) throw failure;
    match = output.match(/http:\/\/127\.0\.0\.1:(\d+)\/api\/v1/);
    if (match) break;
    if (child.exitCode !== null) throw new Error(`engine exited before listening: ${output}`);
    await delay(50);
  }
  assert.ok(match, `engine did not start: ${output}`);
  const address = `http://127.0.0.1:${match[1]}`;
  const client = new ControlClient(address, token);
  const before = await client.status();
  assert.equal(before.session.lifecycle, 'created');
  assert.equal(before.session.generation_epoch, '0');
  assert.equal((await fetch(`${address}/api/v1/health`)).status, 401);
  assert.equal(output.includes(token), false);

  const mute = await client.command(before, 'mute', randomUUID());
  assert.equal(mute.executed, true);
  const muted = await client.status();
  assert.equal(muted.session.generation_epoch, '1');
  assert.equal(muted.session.media_state, 'muted');

  const takeover = await client.command(muted, 'takeover', randomUUID());
  assert.equal(takeover.generation_epoch, '2');
  assert.equal((await client.status()).last_command_seq, '2');

  await writeFile(halt, 'halt');
  const [code] = await exited;
  assert.equal(code, 0, 'the engine must halt cleanly');
  console.log('engine smoke passed: real engine assembly → loopback HTTP → desktop client mute/takeover → graceful halt; no hardware ACK claimed');
} finally {
  clearTimeout(watchdog);
  child.kill();
  await exited;
}
