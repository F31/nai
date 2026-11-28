"""One block, bounded CUDA events; read after the caller's existing sync.

CUDA stream elapsed time includes scheduling/idle/contended intervals, not
exclusive kernel occupancy. Host times describe calls, not kernel durations.
"""
import json
import math
import sys
import time
from contextlib import contextmanager


class BlockProfile:
    def __init__(self, cuda, clock=time.monotonic_ns):
        self.cuda = cuda
        self.stages = None
        self.clock = clock
        self.events = [cuda.Event(enable_timing=True) for _ in range(3)]
        self.begin = clock()
        self.events[0].record()
        self.middle = self.end = None

    def feature_done(self):
        self.events[1].record()
        self.middle = self.clock()

    def submitted(self):
        self.events[2].record()
        self.end = self.clock()

    @contextmanager
    def video_stages(self, pipeline, steps):
        if self.stages is not None or type(steps) is not int or steps < 1:
            raise ValueError('PROFILE_STAGE_STATE')
        self.stages = {}
        originals = []
        def instrument(target, attribute, name, limit):
            original = getattr(target, attribute)
            own = attribute in vars(target)
            rows = []
            self.stages[name] = (limit, rows)
            def measured(*args, **kwargs):
                if len(rows) >= limit:
                    raise ValueError('PROFILE_STAGE_BUDGET')
                start, end = [self.cuda.Event(enable_timing=True) for _ in range(2)]
                host_start = self.clock()
                start.record()
                value = original(*args, **kwargs)
                end.record()
                rows.append((start, end, self.clock()-host_start))
                return value
            originals.append((target, attribute, original, own))
            setattr(target, attribute, measured)
        try:
            instrument(pipeline.model, 'forward', 'denoise', steps)
            instrument(pipeline.vae, 'decode', 'vae_decode', 1)
            instrument(pipeline.vae, 'encode', 'motion_encode', 1)
            yield
        finally:
            for target, attribute, original, own in reversed(originals):
                if own:
                    setattr(target, attribute, original)
                else:
                    delattr(target, attribute)

    def result(self, epoch, block):
        if self.middle is None or self.end is None or epoch < 0 or block < 1:
            raise ValueError('PROFILE_INCOMPLETE')
        feature = self.events[0].elapsed_time(self.events[1])
        video = self.events[1].elapsed_time(self.events[2])
        if not all(math.isfinite(x) and x >= 0 for x in (feature, video)):
            raise ValueError('PROFILE_TIMING')
        stages = None
        if self.stages is not None:
            stages = {}
            for name, (limit, rows) in self.stages.items():
                if len(rows) != limit:
                    raise ValueError('PROFILE_STAGE_COUNT')
                elapsed = [start.elapsed_time(end) for start, end, _ in rows]
                if not all(math.isfinite(x) and x >= 0 for x in elapsed):
                    raise ValueError('PROFILE_TIMING')
                stages[name] = dict(calls=len(rows), cuda_stream_ms=sum(elapsed),
                    host_call_ms=sum(host for _, _, host in rows)/1e6)
        return dict(scope='avatar_block_compute', phase='warmup' if epoch == 0 else 'business',
            epoch=epoch, block=block, feature_host_call_ms=(self.middle-self.begin)/1e6,
            video_host_call_ms=(self.end-self.middle)/1e6,
            feature_cuda_stream_ms=feature, video_cuda_stream_ms=video, video_stages=stages)


def write_result(profile, epoch, block):
    print('NAI_AVATAR_BLOCK_PROFILE '+json.dumps(profile.result(epoch, block),
        separators=(',', ':'), allow_nan=False), file=sys.stderr, flush=True)
