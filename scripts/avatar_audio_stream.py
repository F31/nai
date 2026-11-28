"""Bounded stateful model-input resampling. No IPC or playback authority.

Caller must validate provider EOS before finish; full blocks are provisional.
No model state is reset here. The production worker is not connected yet.
"""
from dataclasses import dataclass
import math

import numpy as np
import soxr


@dataclass(frozen=True)
class AudioBlock:
    start: int
    valid: int
    samples: np.ndarray


class AudioInput:
    def __init__(self, block_frames, max_samples, check):
        if type(block_frames) is not int or not 1 <= block_frames <= 97:
            raise ValueError('BLOCK_GEOMETRY')
        if type(max_samples) is not int or not 1 <= max_samples <= 144000:
            raise ValueError('SOURCE_LIMIT')
        self.check = check
        check()
        self.block_size = block_frames * 16000 // 25
        self.limit = max_samples
        self.source_samples = self.feature_samples = self.delivered = 0
        self.pending = np.empty(0, dtype=np.float32)
        self.resampler = soxr.ResampleStream(24000, 16000, 1, dtype='float32', quality='HQ')
        self.ended = False

    def _guard(self):
        if self.ended:
            raise ValueError('STREAM_ENDED')
        self.check()

    def _abort(self):
        self.ended = True
        self.pending = np.empty(0, dtype=np.float32)
        self.resampler = None

    def _emit(self, valid, emit):
        self.check()
        block = self.pending
        self.pending = np.empty(0, dtype=np.float32)
        if len(block) < self.block_size:
            block = np.pad(block, (0, self.block_size-len(block)))
        emit(AudioBlock(self.delivered, valid, block))
        self.check()
        self.delivered += valid

    def _accept(self, features, emit):
        self.feature_samples += len(features)
        while len(features):
            self.check()
            take = min(self.block_size-len(self.pending), len(features))
            self.pending = np.concatenate((self.pending, features[:take]))
            features = features[take:]
            if len(self.pending) == self.block_size:
                self._emit(self.block_size, emit)

    def push(self, pcm, emit):
        try:
            self._guard()
            if not isinstance(pcm, bytes) or not pcm or len(pcm) % 2:
                raise ValueError('PCM_FORMAT')
            count = len(pcm)//2
            if count > 38400 or self.source_samples+count > self.limit:
                raise ValueError('SOURCE_LIMIT')
            self.source_samples += count
            source = np.frombuffer(pcm, dtype='<i2').astype(np.float32)/32768.0
            self._accept(self.resampler.resample_chunk(source, last=False), emit)
            self._guard()
        except BaseException:
            self._abort()
            raise

    def finish(self, verified_samples, emit):
        try:
            self._guard()
            if not self.source_samples or verified_samples != self.source_samples:
                raise ValueError('EOS_SAMPLES')
            tail = self.resampler.resample_chunk(np.empty(0, dtype=np.float32), last=True)
            expected = math.ceil(self.source_samples*16000/24000)
            missing = expected-self.feature_samples-len(tail)
            # librosa fixes soxr's rounded output length to ceil. Only this final
            # rounding sample may be repaired; arbitrary missing audio is fatal.
            if missing not in (0, 1):
                raise ValueError('RESAMPLE_COUNT')
            if missing:
                tail = np.pad(tail, (0, missing))
            self._accept(tail, emit)
            if len(self.pending):
                self._emit(len(self.pending), emit)
            self._guard()
            if self.delivered != expected:
                raise ValueError('RESAMPLE_COUNT')
            self.ended = True
            self.resampler = None
        except BaseException:
            self._abort()
            raise
