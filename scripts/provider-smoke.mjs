// Real-endpoint smoke for the two configured providers.
//
// This run talks to live, billable endpoints. It exists to produce
// **connectivity facts**, not acceptance: a green run must not move B01 or O01,
// and it must not be read as "the 100-case set passes".
//
// Credentials come from the process environment and never reach any file, log
// or evidence record — not even their prefix. When a required variable is
// absent the run stops with CAPABILITY_NOT_READY instead of falling back to a
// rehearsal stub; a stub that makes the smoke green is the thing §30.1 forbids.
//
//   node --env-file=.env scripts/provider-smoke.mjs
//
// Exit codes: 0 both links produced a real result; 1 a required variable is
// missing (nothing is written); 2 the run completed but at least one link did
// not produce a result (the facts are still written, and the refusal is a fact).
import { mkdir, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const EVIDENCE_DIR = join(ROOT, 'docs', 'reports', 'evidence');
const EVIDENCE_FILE = join(EVIDENCE_DIR, 'live-provider-facts.json');

const TIMEOUT_MS = 90_000;

const REQUIRED = [
  'NAI_LLM_BASE_URL',
  'NAI_LLM_API_KEY',
  'NAI_LLM_MODEL',
  'NAI_TTS_BASE_URL',
  'NAI_TTS_API_KEY',
  'NAI_TTS_MODEL',
  'NAI_TTS_VOICE',
];

// An empty value is as absent as an unset one: copying `.env.example` and
// leaving a blank must not produce a run that "half works".
const read = (name) => {
  const raw = process.env[name];
  return raw === undefined || raw.trim() === '' ? undefined : raw.trim();
};

const missing = REQUIRED.filter((name) => read(name) === undefined);
if (missing.length > 0) {
  console.error(`CAPABILITY_NOT_READY: missing ${missing.join(', ')}`);
  console.error('no evidence written; a rehearsal stub cannot stand in for a live call');
  process.exit(1);
}

const settings = Object.fromEntries(REQUIRED.map((name) => [name, read(name)]));

// The wire scheme is recorded by name only. The credential itself never leaves
// this function, so it cannot reach a record, a filename or a log line.
const authHeaders = (key) => ({
  authorization: `Bearer ${key}`,
  'content-type': 'application/json',
});

// Facts that describe the response, not the request. The request line is never
// reproduced: it would carry the credential.
const responseFacts = (response) => {
  const url = new URL(response.url);
  return {
    host: url.host,
    path: url.pathname,
    method: 'POST',
    protocol: 'openai-compatible',
    auth_scheme: 'Bearer',
    status: response.status,
  };
};

const refusal = async (response) => {
  const text = await response.text().catch(() => '');
  if (text === '') return {};
  try {
    const parsed = JSON.parse(text);
    const error = parsed.error ?? parsed;
    return {
      gateway_error_code: typeof error.code === 'string' ? error.code : undefined,
      gateway_message: typeof error.message === 'string' ? error.message.slice(0, 200) : undefined,
    };
  } catch {
    return { gateway_message: text.slice(0, 200) };
  }
};

const guard = async (body) => {
  try {
    return await body();
  } catch (error) {
    // A transport failure is a host fact, not an endpoint verdict: the record
    // says the call did not complete rather than that the endpoint refused.
    return { status: null, transport_error: String(error?.cause ?? error).slice(0, 300) };
  }
};

async function probeText({ base, key, model, stream }) {
  return guard(async () => {
    const response = await fetch(`${base}/chat/completions`, {
      method: 'POST',
      headers: authHeaders(key),
      body: JSON.stringify({
        model,
        messages: [{ role: 'user', content: '只回答两个字：收到' }],
        max_tokens: 32,
        stream,
      }),
      signal: AbortSignal.timeout(TIMEOUT_MS),
    });
    const record = { ...responseFacts(response), model, streaming: stream };
    if (!response.ok) return { ...record, ...(await refusal(response)) };
    if (stream) {
      // The adapter consumes chat-completions SSE, so the frame shape is
      // checked here rather than assumed from the non-streaming body.
      const text = await response.text();
      const lines = text.split('\n').filter((line) => line.startsWith('data: '));
      const payloads = lines.map((line) => line.slice(6).trim());
      const parsed = [];
      let unparsable = 0;
      for (const payload of payloads) {
        // A stream ends with a sentinel frame that is not JSON. It is recorded
        // as a contract fact rather than fed to the parser and swallowed.
        if (payload === '[DONE]') continue;
        try {
          parsed.push(JSON.parse(payload));
        } catch {
          unparsable += 1;
        }
      }
      const deltas = parsed
        .map((event) => event.choices?.[0]?.delta)
        .filter((delta) => delta !== undefined);
      return {
        ...record,
        sse_data_lines: lines.length,
        sse_terminator_done: payloads.includes('[DONE]'),
        sse_unparsable_frames: unparsable,
        sse_delta_events: deltas.length,
        sse_delta_keys: deltas.length > 0 ? Object.keys(deltas[0]) : [],
      };
    }
    const payload = await response.json();
    return {
      ...record,
      usage_fields: payload.usage ? Object.keys(payload.usage).sort() : [],
      usage: payload.usage ?? null,
      completion_chars: (payload.choices?.[0]?.message?.content ?? '').length,
      finish_reason: payload.choices?.[0]?.finish_reason ?? null,
    };
  });
}

async function probeSpeech({ base, key, model, voice }) {
  return guard(async () => {
    const response = await fetch(`${base}/audio/speech`, {
      method: 'POST',
      headers: authHeaders(key),
      body: JSON.stringify({
        model,
        input: '这是一次语音链路连通测试。',
        voice,
        response_format: 'mp3',
      }),
      signal: AbortSignal.timeout(TIMEOUT_MS),
    });
    const record = { ...responseFacts(response), model, voice };
    const mediaType = response.headers.get('content-type') ?? '';
    const bytes = Buffer.from(await response.arrayBuffer());
    // Byte count and media type come from the response itself, never from a
    // local guess or a default.
    record.media_type = mediaType;
    record.bytes = bytes.length;
    if (!response.ok || !mediaType.startsWith('audio/') || bytes.length === 0) {
      record.produced_audio = false;
      const body = bytes.toString('utf8').slice(0, 200);
      try {
        const parsed = JSON.parse(body);
        const error = parsed.error ?? parsed;
        record.gateway_error_code = typeof error.code === 'string' ? error.code : undefined;
        record.gateway_message = typeof error.message === 'string' ? error.message.slice(0, 200) : undefined;
      } catch {
        if (body !== '') record.gateway_message = body;
      }
      return { record, audio: undefined };
    }
    record.produced_audio = true;
    return { record, audio: bytes };
  });
}

const textOnce = await probeText({
  base: settings.NAI_LLM_BASE_URL,
  key: settings.NAI_LLM_API_KEY,
  model: settings.NAI_LLM_MODEL,
  stream: false,
});
const textStream = await probeText({
  base: settings.NAI_LLM_BASE_URL,
  key: settings.NAI_LLM_API_KEY,
  model: settings.NAI_LLM_MODEL,
  stream: true,
});
const speech = await probeSpeech({
  base: settings.NAI_TTS_BASE_URL,
  key: settings.NAI_TTS_API_KEY,
  model: settings.NAI_TTS_MODEL,
  voice: settings.NAI_TTS_VOICE,
});

await mkdir(EVIDENCE_DIR, { recursive: true });
const audioName = `voice-${settings.NAI_TTS_MODEL.replace(/[^A-Za-z0-9]+/g, '-')}.mp3`;
if (speech.audio !== undefined) {
  await writeFile(join(EVIDENCE_DIR, audioName), speech.audio);
}

const succeeded = (record) => record.status !== null && record.status >= 200 && record.status < 300;

const evidence = {
  run: 'live-provider-smoke',
  generated_by: 'scripts/provider-smoke.mjs',
  // Endpoint facts and host facts are kept apart on purpose: this machine's
  // certificate revocation check is offline, which says nothing about an
  // endpoint's availability.
  boundary: [
    'connectivity facts only; a successful run does not accept B01 or O01',
    'credentials are read from the environment and are absent from this record',
    'byte count and media type come from the response, not from a local default',
  ],
  text_generation: textOnce,
  text_streaming: textStream,
  speech_synthesis: speech.record,
  audio_artifact: speech.audio === undefined ? null : `docs/reports/evidence/${audioName}`,
};
await writeFile(EVIDENCE_FILE, `${JSON.stringify(evidence, null, 2)}\n`);

const summarise = (label, record) =>
  `${label}: status=${record.status ?? 'none'}${record.gateway_error_code ? ` code=${record.gateway_error_code}` : ''}${record.transport_error ? ` transport=${record.transport_error}` : ''}`;

console.log(summarise('text', textOnce));
console.log(summarise('text(stream)', textStream));
console.log(summarise('speech', speech.record));
console.log(`evidence: docs/reports/evidence/live-provider-facts.json`);

const textOk = succeeded(textOnce);
const speechOk = speech.record.produced_audio === true;
if (!textOk || !speechOk) {
  console.error(
    `LIVE_LINK_INCOMPLETE: text=${textOk ? 'ok' : 'no result'} speech=${speechOk ? 'ok' : 'no audio'}`,
  );
  process.exit(2);
}
console.log('both links produced a real result');
