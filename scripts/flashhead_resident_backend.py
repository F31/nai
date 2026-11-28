"""Real FlashHead Lite backend, initialized once. No test backend selection."""
import hashlib
import math
import tempfile
from collections import deque
from pathlib import Path


class Backend:
    def __init__(self, config):
        from avatar_step_override import config_sample_steps
        steps = config_sample_steps(config)
        import sys
        print('avatar load: importing runtime', file=sys.stderr, flush=True)
        import numpy as np
        print('avatar load: numpy imported', file=sys.stderr, flush=True)
        import torch
        print('avatar load: torch imported', file=sys.stderr, flush=True)
        from flash_head import inference
        from flash_head.src.pipeline import flash_head_pipeline
        from flashhead_portrait import prepare_portrait
        self.np, self.torch, self.inference, self.config = np, torch, inference, config
        print('avatar load: runtime imported', file=sys.stderr, flush=True)
        if not torch.cuda.is_available():
            raise RuntimeError('CUDA_UNAVAILABLE')
        portrait = Path(config['portrait'])
        if not config['portrait_authorized'] or portrait.stat().st_size > 10485760 or hashlib.sha256(portrait.read_bytes()).hexdigest() != config['portrait_sha256']:
            raise ValueError('PORTRAIT_IDENTITY')
        flash_head_pipeline.COMPILE_MODEL = False
        flash_head_pipeline.COMPILE_VAE = False
        inference.infer_params.update(width=config['width'], height=config['height'])
        from avatar_weight_loader import weight_loader
        with weight_loader(config.get('safetensors_backend', 'mmap')):
            self.pipeline = inference.get_pipeline(1, config['model_dir'], 'lite', config['wav2vec_dir'])
        motion = inference.infer_params['motion_frames_num']
        if (config['frame_num'] - 1) % self.pipeline.config.vae_stride[0] or config['frame_num'] <= motion:
            raise ValueError('FRAME_GEOMETRY')
        inference.infer_params['frame_num'] = config['frame_num']
        image, _ = prepare_portrait(portrait, config['face_ratio'], 0.5, config['width'])
        with tempfile.TemporaryDirectory(prefix='nai-avatar-', dir=config['temp_dir']) as directory:
            path = Path(directory) / 'portrait.png'
            image.save(path)
            self._prepare_base_data(str(path))
        self.params = inference.get_infer_params()
        if steps is not None:
            from avatar_step_override import override_sample_steps
            override_sample_steps(self, steps)
        self.pipeline.audio_encoder.eval()

    def _prepare_base_data(self, path):
        # LTX portrait posterior.sample() uses the global generator, before the
        # per-task seed reset. Bind it too so independent workers share identity.
        self.torch.manual_seed(self.config['seed'])
        self.inference.get_base_data(self.pipeline, path, self.config['seed'], False)

    def _infer_block(self, history):
        """Shared GPU algorithm; callers own no-grad/SDPA and task lifetime."""
        from avatar_compute_profile import BlockProfile
        profile = BlockProfile(self.torch.cuda)
        params = self.params
        end = params['cached_audio_duration'] * params['tgt_fps']
        embedding = self.inference.get_audio_embedding(self.pipeline,
            self.np.asarray(history, dtype=self.np.float32), end-params['frame_num'], end)
        profile.feature_done()
        with profile.video_stages(self.pipeline, params['sample_steps']):
            frames = self.inference.run_pipeline(self.pipeline, embedding)[params['motion_frames_num']:]
        profile.submitted()
        self._compute_profile = profile
        return frames

    def report_compute_profile(self, epoch, block):
        from avatar_compute_profile import write_result
        write_result(self._compute_profile, epoch, block)
        self._compute_profile = None

    def begin_stream(self, job, emit_frame, max_samples):
        from avatar_stream_render import StreamRender
        return StreamRender(self, job, emit_frame, max_samples)

    def render(self, pcm, job, emit_frame):
        import time
        started=time.monotonic_ns()
        import librosa
        from v561 import avatar_worker_pb2 as wire
        from avatar_frame_prefix import to_host_prefix
        from torch.nn.attention import SDPBackend, sdpa_kernel
        from avatar_owner_frames import OwnerFrames
        from avatar_delivery import Delivery, intersection_ns
        np, torch, params, c = self.np, self.torch, self.params, self.config
        job.check()
        if getattr(self, '_stream_active', False):
            raise ValueError('GPU_STREAM_BUSY')
        # Reset motion history without reloading weights or recomputing portrait features.
        self.pipeline.reset_person_name(self.pipeline.person_name)
        # LTX posterior sampling consumes the global RNG as well. This worker
        # owns the only model task, matching the reviewed fixed-seed candidate.
        torch.manual_seed(c['seed'])
        self.pipeline.generator.manual_seed(c['seed'])
        rate, fps, motion = params['sample_rate'], params['tgt_fps'], params['motion_frames_num']
        source = np.frombuffer(pcm, dtype='<i2').astype(np.float32) / 32768.0
        source = librosa.resample(source, orig_sr=24000, target_sr=rate)
        step = (params['frame_num'] - motion) * rate // fps
        history = deque([0.0] * (rate * params['cached_audio_duration']), maxlen=rate * params['cached_audio_duration'])
        total_frames = math.ceil(job.request.render.samples * fps / 24000)
        stats=wire.RenderStats(block_frames=params['frame_num']-motion,motion_frames=motion,
            feature_sample_rate=rate,feature_samples=len(source),inference_steps=params['sample_steps'])
        stats.setup_ms=(time.monotonic_ns()-started)//1000000
        compute_ns=transfer_ns=delivery_ns=0
        sent = 0
        transport = None
        delivery = None
        compute_intervals = []
        try:
            for offset in range(0, len(source), step):
                job.check()
                block = source[offset:offset + step]
                stats.padded_feature_samples+=step-len(block)
                history.extend(np.pad(block, (0, step-len(block))).tolist())
                compute_at=time.monotonic_ns()
                with torch.no_grad(), sdpa_kernel(SDPBackend.CUDNN_ATTENTION):
                    frames = self._infer_block(history)
                    torch.cuda.synchronize()
                    job.check()
                    if frames.shape != (params['frame_num']-motion, c['height'], c['width'], 3) or not torch.isfinite(frames).all().item():
                        raise ValueError('INVALID_FRAMES')
                    compute_end=time.monotonic_ns()
                    compute_ns+=compute_end-compute_at
                    compute_intervals.append((compute_at,compute_end))
                    self.report_compute_profile(job.request.epoch, stats.blocks+1)
                    stats.blocks+=1;stats.generated_frames+=int(frames.shape[0])
                    if delivery is not None:
                        delivery.wait()
                    transfer_at=time.monotonic_ns()
                    images = to_host_prefix(frames,total_frames-sent,torch.uint8)
                    transfer_ns+=time.monotonic_ns()-transfer_at
                    stats.transferred_frames+=int(images.shape[0])
                if transport is None:
                    if fps!=25:
                        raise ValueError('FRAME_RATE')
                    transport = OwnerFrames(c['width'],c['height'],job,emit_frame)
                    delivery = Delivery(job,transport)
                delivery.submit(images)
                sent += int(images.shape[0])
                del images, frames
            delivery.wait()
            job.check()
            result = transport.finish()
            if sent != total_frames or result['frames_acked'] != sent:
                raise ValueError('FRAME_COUNT')
            stats.retained_frames=sent;stats.tail_frames=stats.generated_frames-sent
            delivery_ns=sum(b-a for a,b in delivery.intervals)
            stats.compute_ms=compute_ns//1000000;stats.transfer_ms=transfer_ns//1000000;stats.delivery_ms=delivery_ns//1000000
            stats.overlap_ms=intersection_ns(compute_intervals,delivery.intervals)//1000000
            return sent,stats
        finally:
            if delivery is not None:
                delivery.close()
            if transport is not None:
                transport.abort()
