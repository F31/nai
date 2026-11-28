"""One real GPU task with continuous audio/motion history. No worker RPC loop.

Consumes canonical PCM incrementally; caller must verify EOS before finish.
Produced frames remain provisional until source/execution owners authorize them.
"""
from collections import deque
import math
import time
import threading

from avatar_audio_stream import AudioInput
from avatar_delivery import Delivery, intersection_ns
from avatar_frame_prefix import to_host_prefix
from avatar_owner_frames import OwnerFrames
from v561 import avatar_worker_pb2 as wire


class StreamRender:
    def __init__(self, backend, job, emit_frame, max_samples):
        job.check()
        if getattr(backend, '_stream_active', False):
            raise ValueError('GPU_STREAM_BUSY')
        p = backend.params
        if p['sample_rate'] != 16000 or p['tgt_fps'] != 25:
            raise ValueError('STREAM_FORMAT')
        self.backend, self.job, self.emit_frame = backend, job, emit_frame
        self.owner = threading.get_ident()
        self.frames = p['frame_num']-p['motion_frames_num']
        self.audio = AudioInput(self.frames, max_samples, job.check)
        self.history = deque([0.0]*(16000*p['cached_audio_duration']), maxlen=16000*p['cached_audio_duration'])
        self.stats = wire.RenderStats(block_frames=self.frames, motion_frames=p['motion_frames_num'],
            feature_sample_rate=16000, inference_steps=p['sample_steps'])
        self.transport = self.delivery = None
        self.sent = self.compute_ns = self.transfer_ns = 0
        self.compute_intervals = []
        self.finished = self.closed = False
        started = time.monotonic_ns()
        backend._stream_active = True
        try:
            backend.pipeline.reset_person_name(backend.pipeline.person_name)
            backend.torch.manual_seed(backend.config['seed'])
            backend.pipeline.generator.manual_seed(backend.config['seed'])
            job.check()
            self.stats.setup_ms = (time.monotonic_ns()-started)//1000000
        except BaseException:
            self.close()
            raise

    def _guard(self):
        if threading.get_ident() != self.owner:
            self.job.cancelled.set()
            raise RuntimeError('GPU_STREAM_OWNER')
        if self.closed or self.finished:
            raise ValueError('GPU_STREAM_ENDED')
        self.job.check()

    def _block(self, block):
        from torch.nn.attention import SDPBackend, sdpa_kernel
        self._guard()
        b = self.backend
        np, torch = b.np, b.torch
        if block.start != self.stats.feature_samples or len(block.samples) != self.audio.block_size:
            raise ValueError('FEATURE_OFFSET')
        self.history.extend(block.samples.tolist())
        compute_at = time.monotonic_ns()
        with torch.no_grad(), sdpa_kernel(SDPBackend.CUDNN_ATTENTION):
            frames = b._infer_block(self.history)
            torch.cuda.synchronize()
            self.job.check()
            if frames.shape != (self.frames, b.config['height'], b.config['width'], 3) or not torch.isfinite(frames).all().item():
                raise ValueError('INVALID_FRAMES')
            compute_end = time.monotonic_ns()
            self.compute_ns += compute_end-compute_at
            self.compute_intervals.append((compute_at, compute_end))
            b.report_compute_profile(self.job.request.epoch, self.stats.blocks+1)
            if self.delivery is not None:
                self.delivery.wait()
            transfer_at = time.monotonic_ns()
            # Full blocks need no future source; only EOS can emit a short block.
            images = to_host_prefix(frames, math.ceil(block.valid/640), torch.uint8)
            self.transfer_ns += time.monotonic_ns()-transfer_at
        if self.transport is None:
            self.transport = OwnerFrames(b.config['width'], b.config['height'], self.job, self.emit_frame)
            self.delivery = Delivery(self.job, self.transport)
        self.delivery.submit(images)
        count = int(images.shape[0])
        self.sent += count
        self.stats.blocks += 1
        self.stats.feature_samples += block.valid
        self.stats.padded_feature_samples += self.audio.block_size-block.valid
        self.stats.generated_frames += self.frames
        self.stats.transferred_frames += count

    def push(self, pcm):
        try:
            self._guard()
            self.audio.push(pcm, self._block)
        except BaseException:
            self.close()
            raise

    def finish(self, verified_samples):
        try:
            self._guard()
            self.audio.finish(verified_samples, self._block)
            if self.delivery is None:
                raise ValueError('NO_FRAMES')
            self.delivery.wait()
            result = self.transport.finish()
            if self.sent != math.ceil(verified_samples/960) or result['frames_acked'] != self.sent:
                raise ValueError('FRAME_COUNT')
            self.job.check()
            self.stats.retained_frames = self.sent
            self.stats.tail_frames = self.stats.generated_frames-self.sent
            self.stats.compute_ms = self.compute_ns//1000000
            self.stats.transfer_ms = self.transfer_ns//1000000
            self.stats.delivery_ms = sum(end-start for start, end in self.delivery.intervals)//1000000
            self.stats.overlap_ms = intersection_ns(self.compute_intervals, self.delivery.intervals)//1000000
            self.finished = True
            self.close()
            return self.sent, self.stats
        except BaseException:
            self.close()
            raise

    def close(self):
        # Control may set job.cancelled from any thread. Only the GPU owner may
        # join delivery/unmap pixels or release exclusivity after kernels return.
        if threading.get_ident() != self.owner:
            self.job.cancelled.set()
            raise RuntimeError('GPU_STREAM_OWNER')
        if self.closed:
            return
        self.closed = True
        if not self.finished:
            self.job.cancelled.set()
        try:
            if self.delivery is not None:
                self.delivery.close()
        finally:
            try:
                if self.transport is not None:
                    self.transport.abort()
            finally:
                self.backend._stream_active = False
