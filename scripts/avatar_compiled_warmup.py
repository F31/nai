"""Internal silent GPU warmup. No frames/PCM cross the control channel."""
import math
import time
from types import SimpleNamespace as NS


def warmup(backend, config, startup_started):
    from avatar_compile import install, require_evidence
    from avatar_worker_core import Job
    from v561 import avatar_worker_pb2 as wire
    from v561.media_pb2 import FrameRelease
    samples = config['warmup_samples']
    if (not isinstance(samples, int) or not 1 <= samples <= 144000
            or math.ceil(samples/960) < config['frame_num']):
        raise ValueError('WARMUP_SAMPLES')
    remaining = config['maintenance_timeout_ms']/1000 - (time.monotonic()-startup_started)
    if remaining <= 0:
        raise TimeoutError('MAINTENANCE_DEADLINE')
    job = Job(NS(epoch=0, render=NS(samples=samples)), remaining)
    started = time.monotonic()
    install(backend.torch, backend.pipeline, eager_sdpa=True, blocks=True)
    def release(request, mapping, frame):
        job.check()
        if frame.epoch != 0:
            raise ValueError('WARMUP_EPOCH')
        job.releases.put_nowait(FrameRelease(pool_id=frame.pool_id, epoch=frame.epoch,
            sequence=frame.sequence, slot_index=frame.slot_index, slot_generation=frame.slot_generation))
    frames, stats = backend.render(bytes(samples*2), job, release)
    job.check()
    from torch._dynamo.utils import counters
    from torch._inductor import metrics
    require_evidence(counters['stats'], metrics.generated_kernel_count)
    if frames != math.ceil(samples/960) or stats.retained_frames != frames or stats.blocks < 2:
        raise ValueError('WARMUP_FRAMES')
    return wire.WarmupStats(samples=samples, frames_acked=frames, blocks=stats.blocks,
        unique_graphs=counters['stats']['unique_graphs'], generated_kernels=metrics.generated_kernel_count,
        elapsed_ms=round((time.monotonic()-started)*1000),block_frames=stats.block_frames,motion_frames=stats.motion_frames)
