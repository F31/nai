"""One resident voice's conditioning only. No response text or disk cache."""
import functools
import hashlib
import json
import sys
import threading


class ConditionCache:
    def __init__(self, frontend, max_bytes=16 * 1024 * 1024, enabled=True):
        if type(max_bytes) is not int or not 1 <= max_bytes <= 16 * 1024 * 1024:
            raise ValueError('CONDITION_CACHE_LIMIT')
        self.frontend = frontend
        self.original = frontend.frontend_zero_shot
        self.max_bytes = max_bytes
        self.enabled = enabled
        self.lock = threading.RLock()
        self.key = None
        self.conditions = None
        self.bytes = 0
        self.hits = self.misses = self.bypasses = 0

    def stats(self):
        with self.lock:
            return dict(hits=self.hits, misses=self.misses, bypasses=self.bypasses,
                        entries=int(self.conditions is not None), bytes=self.bytes)

    def identity(self, prompt, wav, rate):
        if not isinstance(prompt, str) or len(prompt) > 8192 or type(rate) is not int:
            return None
        size = wav.numel() * wav.element_size()
        if size <= 0 or size > self.max_bytes:
            return None
        header = json.dumps([prompt, rate, str(wav.dtype), list(wav.shape)],
                            ensure_ascii=False, separators=(',', ':')).encode('utf-8')
        digest = hashlib.sha256(header)
        digest.update(wav.detach().cpu().contiguous().numpy().tobytes())
        return digest.digest()

    @staticmethod
    def clone(conditions):
        return {k: v.detach().clone() for k, v in conditions.items()}

    def call(self, tts_text, prompt_text, prompt_wav, resample_rate, zero_shot_spk_id):
        with self.lock:
            if not self.enabled or zero_shot_spk_id:
                self.bypasses += 1
                return self.original(tts_text, prompt_text, prompt_wav, resample_rate, zero_shot_spk_id)
            key = self.identity(prompt_text, prompt_wav, resample_rate)
            if key is None:
                self.bypasses += 1
                return self.original(tts_text, prompt_text, prompt_wav, resample_rate, zero_shot_spk_id)
            if key == self.key and self.conditions is not None:
                text, length = self.frontend._extract_text_token(tts_text)
                result = self.clone(self.conditions)
                result.update(text=text, text_len=length)
                self.hits += 1
                return result
            self.misses += 1
            result = self.original(tts_text, prompt_text, prompt_wav, resample_rate, zero_shot_spk_id)
            if 'text' not in result or 'text_len' not in result:
                raise ValueError('CONDITION_CACHE_FORMAT')
            conditions = {k: v for k, v in result.items() if k not in ('text', 'text_len')}
            size = sum(v.numel() * v.element_size() for v in conditions.values())
            if conditions and size <= self.max_bytes:
                copy = self.clone(conditions)
                self.key, self.conditions, self.bytes = key, copy, size
            return result


def install(model, max_bytes=16 * 1024 * 1024, enabled=True):
    frontend = model.frontend
    if getattr(frontend, '_nai_condition_cache', None) is not None:
        raise ValueError('CONDITION_CACHE_ALREADY_INSTALLED')
    cache = ConditionCache(frontend, max_bytes, enabled)
    @functools.wraps(cache.original)
    def cached(*args, **kwargs):
        try:
            return cache.call(*args, **kwargs)
        finally:
            try:
                print('NAI_CONDITION_CACHE ' + json.dumps(cache.stats()), file=sys.stderr, flush=True)
            except OSError:
                pass
    frontend.frontend_zero_shot = cached
    frontend._nai_condition_cache = cache
    return cache
