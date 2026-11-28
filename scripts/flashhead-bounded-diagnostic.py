"""Bounded FlashHead resource diagnostic; never a live output or M0 pass."""
import argparse
import json
import math
import os
import time
from collections import deque
from contextlib import nullcontext
from pathlib import Path


def positive_seconds(value):
    number = float(value)
    if not math.isfinite(number) or not 0 < number <= 600:
        raise argparse.ArgumentTypeError('seconds must be finite and in (0, 600]')
    return number

def diagnostic_deadline(value):
    number = float(value)
    if not math.isfinite(number) or not 0 < number <= 1200:
        raise argparse.ArgumentTypeError('deadline must be finite and in (0, 1200]')
    return number


def validate_required_attention(require_sage, require_flash, sage, flash2, flash3):
    if require_sage and not sage:
        raise RuntimeError('requested SageAttention was not imported by the model')
    if require_flash and (not flash2 or sage or flash3):
        raise RuntimeError('requested FlashAttention 2 is unavailable or a higher-priority backend is imported')


def validate_frame_number(frame_num, temporal_stride, motion_frames):
    if temporal_stride <= 0 or (frame_num - 1) % temporal_stride or frame_num <= motion_frames:
        raise ValueError('frame number must match VAE temporal stride and exceed motion frames')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--model-dir', type=Path, required=True)
    parser.add_argument('--wav2vec-dir', type=Path, required=True)
    parser.add_argument('--portrait', type=Path, required=True)
    parser.add_argument('--audio', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--audio-seconds', type=positive_seconds, default=60)
    parser.add_argument('--frame-num', type=int, help='diagnostic-only generated frames per block; validated against model stride')
    parser.add_argument('--deadline-seconds', type=diagnostic_deadline, default=180)
    parser.add_argument('--attention-backend', choices=('default', 'cudnn'), default='default')
    parser.add_argument('--cudnn-benchmark', action='store_true', help='diagnostic-only cuDNN convolution algorithm search')
    parser.add_argument('--global-seed', type=int, help='optional diagnostic seed for model and VAE random state')
    parser.add_argument('--profile-trace', type=Path, help='diagnostic trace of the second block only; timing includes profiler overhead')
    parser.add_argument('--transport-slots', type=int, choices=range(5), default=0)
    parser.add_argument('--transport-timeout-seconds', type=float, default=5)
    parser.add_argument('--rust-receiver', type=Path)
    parser.add_argument('--mapping-reader', type=Path)
    parser.add_argument('--playback-pcm', type=Path, help='diagnostic 24 kHz mono signed 16-bit little-endian PCM')
    parser.add_argument('--audio-executor', type=Path, help='explicit independent audio executor; use AV clock-bridge receiver')
    parser.add_argument('--prefill-frames', type=int, default=8)
    parser.add_argument('--overlap-transfer', action='store_true', help='diagnostic bounded GPU inference / frame transport overlap')
    parser.add_argument('--render-width', type=int, help='diagnostic-only model rendering width')
    parser.add_argument('--render-height', type=int, help='diagnostic-only model rendering height')
    parser.add_argument('--sample-frame', type=Path, help='save the first generated frame for visual quality review')
    parser.add_argument('--raw-video', type=Path, help='diagnostic-only RGB24 raw frames; requested audio at most 7 seconds, rounded to full chunks')
    parser.add_argument('--face-crop', action='store_true', help='explicit single-face composition; never silently fall back')
    parser.add_argument('--face-ratio', type=float, default=2.8)
    parser.add_argument('--face-confidence', type=float, default=0.5)
    parser.add_argument('--face-crop-size', type=int, default=512)
    parser.add_argument('--require-compile', action='store_true', help='fail unless PyTorch compilation is enabled and the model is wrapped')
    parser.add_argument('--require-sage-attention', action='store_true', help='fail unless the upstream model imported SageAttention')
    parser.add_argument('--require-flash-attention', action='store_true', help='fail unless upstream selects imported FlashAttention 2')
    parser.add_argument('--compile-scope', choices=('all', 'model', 'vae', 'none'), default='all', help='diagnostic-only compilation scope')
    parser.add_argument('--compile-mode', choices=('default', 'reduce-overhead'), default='default', help='diagnostic-only visual model compilation mode')
    parser.add_argument('--preload-rope', action='store_true', help='diagnostic-only pre-positioning of model rotary frequencies')
    args = parser.parse_args()
    if args.profile_trace and args.profile_trace.exists():
        parser.error('profile trace already exists')
    if args.global_seed is not None and not 0 <= args.global_seed <= 0xffffffff:
        parser.error('global seed must be between 0 and 2^32-1')
    if args.frame_num is not None and not 17 <= args.frame_num <= 97:
        parser.error('frame number must be between 17 and 97')
    if args.require_compile and args.compile_scope == 'none':
        parser.error('require-compile cannot be used with compile-scope none')
    if args.compile_mode != 'default' and args.compile_scope not in ('all', 'model'):
        parser.error('non-default compile mode requires model compilation')
    if bool(args.rust_receiver) != bool(args.mapping_reader) or (args.rust_receiver and not args.transport_slots):
        parser.error('rust receiver and mapping reader must be provided together with transport slots')
    if args.playback_pcm and (not args.rust_receiver or not args.playback_pcm.is_file() or not 1 <= args.prefill_frames <= 96):
        parser.error('playback requires receiver, existing PCM and prefill 1..96')
    if args.overlap_transfer and not args.rust_receiver:
        parser.error('overlap requires the Rust receiver')
    if args.audio_executor and (not args.playback_pcm or not args.audio_executor.is_file()):
        parser.error('independent audio requires existing executor and source PCM')
    if (args.render_width is None) != (args.render_height is None):
        parser.error('render width and height must be specified together')
    if args.render_width is not None and any(value < 256 or value > 1024 or value % 32 for value in (args.render_width, args.render_height)):
        parser.error('diagnostic dimensions must be multiples of 32 between 256 and 1024')
    if args.sample_frame and args.sample_frame.exists():
        parser.error('sample frame already exists')
    if args.sample_frame and not args.transport_slots:
        parser.error('sample frame requires transport slots')
    if args.raw_video and (args.raw_video.exists() or args.audio_seconds > 7 or args.render_width is None):
        parser.error('raw video requires a new file, explicit dimensions and at most 7 seconds')
    if not math.isfinite(args.face_ratio) or not 1.5 <= args.face_ratio <= 4 or not math.isfinite(args.face_confidence) or not 0 <= args.face_confidence <= 1 or not 256 <= args.face_crop_size <= 1024:
        parser.error('invalid face composition parameters')
    if not math.isfinite(args.transport_timeout_seconds) or not 0 < args.transport_timeout_seconds <= 15:
        parser.error('transport timeout must be in (0, 15] seconds')
    for path in (args.model_dir, args.wav2vec_dir, args.portrait, args.audio):
        if not path.exists():
            parser.error(f'missing input: {path}')
    if args.output.exists():
        parser.error('output already exists; choose a new path')
    # Imports are delayed so argument validation never loads a model or GPU.
    import librosa
    import numpy as np
    import soundfile as sf
    import torch
    torch.backends.cudnn.benchmark = args.cudnn_benchmark
    if args.global_seed is not None:
        torch.manual_seed(args.global_seed)
    try:
        import psutil
        process = psutil.Process()
    except ImportError:
        process = None
    from flash_head import inference
    from flash_head.inference import get_pipeline, get_base_data, get_infer_params, get_audio_embedding, run_pipeline
    from flash_head.src.pipeline import flash_head_pipeline
    from flash_head.src.modules import flash_head_model
    flash_head_pipeline.COMPILE_MODEL = args.compile_scope in ('all', 'model')
    flash_head_pipeline.COMPILE_VAE = args.compile_scope in ('all', 'vae')
    if args.render_width is not None:
        inference.infer_params['width'] = args.render_width
        inference.infer_params['height'] = args.render_height

    info = sf.info(str(args.audio))
    if info.frames <= 0 or info.duration > 60:
        parser.error('source audio must be nonempty and at most 60 seconds')
    started = time.monotonic()
    with args.output.open('x', encoding='utf-8', buffering=1) as report:
        transport = None
        raw_video = None
        profiler = None
        def write(event, **fields):
            report.write(json.dumps({'event':event, 'elapsed_s':round(time.monotonic()-started, 3), 'unix_ms':int(time.time()*1000), **fields}, ensure_ascii=False)+'\n')
        try:
            if args.raw_video:
                raw_video = args.raw_video.open('xb')
            write('attention_import', sage_available=flash_head_model.SAGE_ATTN_AVAILABLE, flash2_available=flash_head_model.FLASH_ATTN_2_AVAILABLE, flash3_available=flash_head_model.FLASH_ATTN_3_AVAILABLE)
            validate_required_attention(args.require_sage_attention, args.require_flash_attention, flash_head_model.SAGE_ATTN_AVAILABLE, flash_head_model.FLASH_ATTN_2_AVAILABLE, flash_head_model.FLASH_ATTN_3_AVAILABLE)
            if not torch.cuda.is_available():
                raise RuntimeError('CUDA unavailable')
            portrait = None
            if args.face_crop:
                from flashhead_portrait import prepare_portrait
                portrait, composition = prepare_portrait(args.portrait, args.face_ratio, args.face_confidence, args.face_crop_size)
                write('portrait_composition', **composition)
            torch.cuda.reset_peak_memory_stats()
            pipeline = get_pipeline(world_size=1, ckpt_dir=str(args.model_dir), wav2vec_dir=str(args.wav2vec_dir), model_type='lite')
            if args.frame_num is not None:
                stride = pipeline.config.vae_stride[0]
                motion_frames = inference.infer_params['motion_frames_num']
                validate_frame_number(args.frame_num, stride, motion_frames)
                inference.infer_params['frame_num'] = args.frame_num
            compile_disabled = bool(torch._dynamo.config.disable)
            model_wrapped = hasattr(pipeline.model, '_orig_mod')
            if args.preload_rope:
                model = pipeline.model._orig_mod if model_wrapped else pipeline.model
                original = model.freqs
                moved = original.to(pipeline.device)
                if not torch.equal(original.cpu(), moved.cpu()):
                    raise RuntimeError('rotary frequencies changed during device transfer')
                model.freqs = moved
                write('rotary_preload', source=str(original.device), target=str(moved.device), dtype=str(moved.dtype), elements=moved.numel(), identical=True)
            if args.compile_mode != 'default':
                if compile_disabled or not model_wrapped:
                    raise RuntimeError('non-default compile mode requires an enabled compiled model')
                pipeline.model = torch.compile(pipeline.model._orig_mod, mode=args.compile_mode)
                model_wrapped = hasattr(pipeline.model, '_orig_mod')
            vae = pipeline.vae.model if pipeline.use_ltx else pipeline.vae
            vae_wrapped = all(hasattr(getattr(vae, name), '_torchdynamo_orig_callable') for name in ('encode', 'decode'))
            write('compile_status', scope=args.compile_scope, mode=args.compile_mode, disabled=compile_disabled, model_wrapped=model_wrapped, vae_wrapped=vae_wrapped, model_type=type(pipeline.model).__name__)
            if args.require_compile and (compile_disabled or (flash_head_pipeline.COMPILE_MODEL and not model_wrapped) or (flash_head_pipeline.COMPILE_VAE and not vae_wrapped)):
                raise RuntimeError('requested compilation is disabled or component is not wrapped')
            if args.face_crop:
                import tempfile
                with tempfile.TemporaryDirectory(prefix='nai-v561-face-', dir=args.output.parent) as temp:
                    portrait_path = Path(temp) / 'portrait.png'
                    portrait.save(portrait_path, format='PNG')
                    get_base_data(pipeline, cond_image_path_or_dir=str(portrait_path), base_seed=42, use_face_crop=False)
            else:
                get_base_data(pipeline, cond_image_path_or_dir=str(args.portrait), base_seed=42, use_face_crop=False)
            params = get_infer_params()
            rate, fps = params['sample_rate'], params['tgt_fps']
            frame_num, motion = params['frame_num'], params['motion_frames_num']
            step = (frame_num - motion) * rate // fps
            if step <= 0 or params['cached_audio_duration'] <= 0:
                raise RuntimeError('invalid upstream frame/audio configuration')
            source, actual_rate = librosa.load(str(args.audio), sr=rate, mono=True)
            if actual_rate != rate or len(source) == 0:
                raise RuntimeError('invalid decoded audio')
            cache_len = rate * params['cached_audio_duration']
            audio_cache = deque([0.0] * cache_len, maxlen=cache_len)
            number = math.ceil(args.audio_seconds * rate / step)
            if args.playback_pcm:
                playback_samples = number * (frame_num - motion) * 24000
                if playback_samples % fps or args.playback_pcm.stat().st_size != playback_samples // fps * 2:
                    raise ValueError('playback PCM length does not match generated audio timeline')
            position = 0
            write('ready', sample_rate=rate, fps=fps, frame_num=frame_num, motion_frames=motion, samples_per_chunk=step, frames_per_chunk=frame_num-motion, audio_cache_samples=cache_len, planned_chunks=number, source_seconds=round(len(source)/rate,3), model_training=pipeline.audio_encoder.training, attention_backend=args.attention_backend, cudnn_benchmark=args.cudnn_benchmark, global_seed=args.global_seed, render_width=params['width'], render_height=params['height'])
            for index in range(number):
                if time.monotonic() - started > args.deadline_seconds:
                    raise TimeoutError('diagnostic deadline exceeded between chunks')
                indices = (np.arange(step, dtype=np.int64) + position) % len(source)
                block = source[indices]
                position = (position + step) % len(source)
                audio_cache.extend(block.tolist())
                audio_array = np.asarray(audio_cache, dtype=np.float32)
                end = params['cached_audio_duration'] * fps
                if index == 1 and args.profile_trace:
                    profiler = torch.profiler.profile(activities=[torch.profiler.ProfilerActivity.CPU, torch.profiler.ProfilerActivity.CUDA], record_shapes=True)
                    profiler.start()
                tick = time.perf_counter()
                with torch.no_grad():
                    embedding = get_audio_embedding(pipeline, audio_array, end-frame_num, end)
                    torch.cuda.synchronize()
                    audio_done = time.perf_counter()
                    if args.attention_backend == 'cudnn':
                        from torch.nn.attention import SDPBackend, sdpa_kernel
                        backend_scope = sdpa_kernel(SDPBackend.CUDNN_ATTENTION)
                    else:
                        backend_scope = nullcontext()
                    with backend_scope:
                        frames = run_pipeline(pipeline, embedding)[motion:]
                    torch.cuda.synchronize()
                    render_done = time.perf_counter()
                    valid = frames.shape[0] == frame_num-motion and torch.isfinite(frames).all().item()
                    check_done = time.perf_counter()
                if not valid:
                    raise RuntimeError('invalid generated frames')
                if raw_video is not None:
                    raw_video.write(frames.to(dtype=torch.uint8).cpu().numpy().tobytes())
                transport_s = None
                if args.transport_slots:
                    if frames.ndim != 4 or frames.shape[3] != 3:
                        raise RuntimeError('unexpected RGB frame shape')
                    if transport is None:
                        if args.rust_receiver:
                            from flashhead_rust_transport import DiagnosticTransport
                            transport = DiagnosticTransport(int(frames.shape[2]), int(frames.shape[1]), args.transport_slots, rate, fps, args.transport_timeout_seconds, args.rust_receiver, args.mapping_reader, args.playback_pcm, args.prefill_frames, audio_executor=args.audio_executor)
                        else:
                            from flashhead_shm_diagnostic import DiagnosticTransport
                            transport = DiagnosticTransport(int(frames.shape[2]), int(frames.shape[1]), args.transport_slots, rate, fps, args.transport_timeout_seconds)
                        write('transport_ready', slots=args.transport_slots, width=int(frames.shape[2]), height=int(frames.shape[1]), pixel_format='bgra8', epoch=transport.epoch, frame_timeline='source_samples', playback_clock='independent_signed_device' if args.audio_executor else ('local_device_observed' if args.playback_pcm else 'not_connected'))
                        if args.overlap_transfer:
                            from flashhead_overlap_transport import OverlapTransport
                            transport = OverlapTransport(transport, args.transport_timeout_seconds)
                    transfer_start = time.perf_counter()
                    rgb = frames.to(dtype=torch.uint8).cpu().numpy()
                    if index == 0 and args.sample_frame:
                        from PIL import Image
                        with args.sample_frame.open('xb') as sample_file:
                            Image.fromarray(rgb[0]).save(sample_file, format='PNG')
                    if args.overlap_transfer:
                        transport.submit(rgb)
                    else:
                        for frame in rgb:
                            transport.send_rgb(frame)
                    transport_s = round(time.perf_counter() - transfer_start, 4)
                    del rgb
                rss = process.memory_info().rss if process else None
                write('chunk', seq=index, duration_s=round(render_done-tick,4), audio_s=round(audio_done-tick,4), render_s=round(render_done-audio_done,4), check_s=round(check_done-render_done,4), transport_s=transport_s, transport_mode='overlap_enqueue' if args.overlap_transfer else 'synchronous', frames=int(frames.shape[0]), frame_bytes=int(frames.numel()*frames.element_size()), rss_bytes=rss, gpu_allocated_bytes=torch.cuda.memory_allocated(), gpu_reserved_bytes=torch.cuda.memory_reserved())
                del frames, embedding, audio_array, block, indices
                if profiler is not None:
                    profiler.stop()
                    profiler.export_chrome_trace(str(args.profile_trace))
                    profiler = None
                    write('profile_exported', path=str(args.profile_trace), scope='second-block-profiler-overhead-included')
            if transport is not None:
                transport_result = transport.finish()
                transport = None
                write('transport_complete', **transport_result)
            write('complete', chunks=number, source_audio_seconds=round(number*step/rate,3), gpu_peak_allocated_bytes=torch.cuda.max_memory_allocated(), gpu_peak_reserved_bytes=torch.cuda.max_memory_reserved())
        except BaseException as exc:
            write('failed', error_type=type(exc).__name__, message=str(exc)[:500])
            raise
        finally:
            if profiler is not None:
                profiler.stop()
            if transport is not None:
                transport.abort()
            if raw_video is not None:
                raw_video.close()


if __name__ == '__main__':
    main()
