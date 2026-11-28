"""Ephemeral bounded completion receipts; no audio, disk cache or playback grants."""
import re
import secrets
import threading
import time


class Receipts:
    def __init__(self, capacity=16, ttl_seconds=120):
        if not 1 <= capacity <= 64 or not 1 <= ttl_seconds <= 600:
            raise ValueError("receipt bounds")
        self.instance = secrets.token_hex(16)
        self.capacity = capacity
        self.ttl = ttl_seconds
        self.lock = threading.Lock()
        self.records = {}

    def begin(self):
        with self.lock:
            now = time.monotonic()
            self.records = {k: v for k, v in self.records.items() if now - v[0] < self.ttl}
            if len(self.records) >= self.capacity:
                # Do not evict an in-flight receipt. Refuse before generation.
                done = [k for k, (_, v) in self.records.items() if v['state'] != 'running']
                if not done:
                    raise RuntimeError("completion capacity exhausted")
                del self.records[done[0]]
            key = secrets.token_hex(16)
            self.records[key] = (now, dict(protocol="nai.cosyvoice.completion.v1",
                request_id=key, instance_id=self.instance, state="running",
                samples=0, sha256="0" * 64))
            return key

    def finish(self, key, completion):
        with self.lock:
            entry = self.records.get(key)
            if entry is None or entry[1]['state'] != 'running':
                return
            state = completion['state']
            samples = completion['samples']
            digest = completion['sha256']
            if (state not in ('completed', 'failed', 'cancelled', 'expired')
                    or not isinstance(samples, int) or not 0 <= samples <= 480000
                    or re.fullmatch('[0-9a-f]{64}', digest) is None):
                state, samples, digest = 'failed', 0, '0' * 64
            self.records[key] = (entry[0], {**entry[1], 'state': state,
                                           'samples': samples, 'sha256': digest})

    def discard(self, key):
        with self.lock:
            self.records.pop(key, None)

    def get(self, key):
        if re.fullmatch('[0-9a-f]{32}', key) is None:
            return None
        with self.lock:
            entry = self.records.get(key)
            if entry is None:
                return None
            if time.monotonic() - entry[0] >= self.ttl:
                del self.records[key]
                return None
            return dict(entry[1])


def install(app, gate, generate_data, get_sample_rate):
    """Return the replacement pcm_response; called once per service process."""
    from fastapi import HTTPException
    from fastapi.responses import StreamingResponse
    from starlette.background import BackgroundTask
    receipts = Receipts()

    @app.get('/nai/tts/status/{request_id}')
    async def completion_status(request_id: str):
        result = receipts.get(request_id)
        if result is None:
            raise HTTPException(status_code=404, detail="Unknown or expired completion")
        return result

    def pcm_response(model_output, stream):
        from nai_request_gate import BusyError
        try:
            key = receipts.begin()
        except RuntimeError:
            raise HTTPException(status_code=409, detail="Completion capacity exhausted")
        try:
            job = gate.start(lambda: generate_data(model_output),
                             on_complete=lambda result: receipts.finish(key, result))
        except BusyError:
            receipts.discard(key)
            raise HTTPException(status_code=409, detail="TTS busy; request was not queued")
        except BaseException:
            receipts.discard(key)
            raise
        return StreamingResponse(job.iter_bytes(), background=BackgroundTask(job.cancel),
            media_type="application/octet-stream", headers={
                'X-Sample-Rate': str(get_sample_rate()), 'X-Audio-Channels': '1',
                'X-Audio-Encoding': 'pcm_s16le', 'X-Model-Stream': str(stream).lower(),
                'X-Nai-Request': key, 'X-Nai-Instance': receipts.instance})
    return pcm_response
