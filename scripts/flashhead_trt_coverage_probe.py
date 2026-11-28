"""Bounded, isolated TensorRT dry-run on one real FlashHead denoising input."""
import argparse
import json
import tempfile
import time
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for option in ('model-dir', 'wav2vec-dir', 'portrait', 'audio', 'output'):
        parser.add_argument('--' + option, type=Path, required=True)
    parser.add_argument('--size', type=int, default=768)
    parser.add_argument('--frames', type=int, default=49)
    parser.add_argument('--non-strict-export', action='store_true', help='use PyTorch non-strict export for compatibility diagnosis')
    parser.add_argument('--opaque-sage', action='store_true', help='wrap the unchanged SageAttention call as an opaque diagnostic custom op')
    parser.add_argument('--real-rope', action='store_true', help='diagnostic FP32 real-pair rotary encoding')
    parser.add_argument('--benchmark-ffn', action='store_true', help='compile and compare the first real FFN with a captured activation')
    parser.add_argument('--ffn-fp32-acc', action='store_true', help='diagnostic TensorRT FP32 accumulation')
    args = parser.parse_args()
    if not 256 <= args.size <= 1024 or args.size % 32 or not 17 <= args.frames <= 97:
        parser.error('invalid size or frame count')
    if args.output.exists() or not args.model_dir.is_dir() or not args.wav2vec_dir.is_dir() or not args.portrait.is_file() or not args.audio.is_file():
        parser.error('output exists or input missing')

    started = time.monotonic()
    with args.output.open('x', encoding='utf-8', buffering=1) as report:
        def write(event, **fields):
            report.write(json.dumps({'event': event, 'elapsed_s': round(time.monotonic() - started, 3), **fields}) + '\n')
        try:
            import librosa
            import numpy as np
            import torch
            import torch_tensorrt
            from flashhead_portrait import prepare_portrait
            from flash_head import inference
            from flash_head.src.pipeline import flash_head_pipeline
            from flash_head.src.modules import flash_head_model

            if not torch.cuda.is_available() or not flash_head_model.SAGE_ATTN_AVAILABLE:
                raise RuntimeError('CUDA and SageAttention are required for the selected baseline')
            if args.opaque_sage:
                original_sage = flash_head_model.sageattn

                @torch.library.custom_op('nai_diagnostic::sage_attention', mutates_args=())
                def opaque_sage(q: torch.Tensor, k: torch.Tensor, v: torch.Tensor) -> torch.Tensor:
                    return original_sage(q, k, v)

                @opaque_sage.register_fake
                def fake_sage(q, k, v):
                    return torch.empty_like(q)

                flash_head_model.sageattn = opaque_sage
            flash_head_pipeline.COMPILE_MODEL = False
            flash_head_pipeline.COMPILE_VAE = False
            inference.infer_params['width'] = args.size
            inference.infer_params['height'] = args.size
            pipeline = inference.get_pipeline(1, str(args.model_dir), 'lite', str(args.wav2vec_dir))
            if args.real_rope:
                from flashhead_real_rope_probe import rope_apply_real
                pipeline.model.freqs = torch.view_as_real(pipeline.model.freqs).float()
                flash_head_model.rope_apply = rope_apply_real
            stride = pipeline.config.vae_stride[0]
            if (args.frames - 1) % stride:
                raise ValueError('frame count incompatible with temporal stride')
            inference.infer_params['frame_num'] = args.frames
            portrait, composition = prepare_portrait(args.portrait, 2.8, 0.5, args.size)
            with tempfile.TemporaryDirectory(prefix='nai-trt-portrait-', dir=args.output.parent) as temp:
                portrait_path = Path(temp) / 'portrait.png'
                portrait.save(portrait_path, format='PNG')
                inference.get_base_data(pipeline, str(portrait_path), 42, False)
            params = inference.get_infer_params()
            rate, fps = params['sample_rate'], params['tgt_fps']
            audio, actual_rate = librosa.load(str(args.audio), sr=rate, mono=True)
            if actual_rate != rate or len(audio) == 0:
                raise ValueError('invalid audio')
            step = (args.frames - params['motion_frames_num']) * rate // fps
            cache = np.zeros(rate * params['cached_audio_duration'], dtype=np.float32)
            cache[-step:] = audio[np.arange(step) % len(audio)]
            end = params['cached_audio_duration'] * fps
            with torch.no_grad():
                context = inference.get_audio_embedding(pipeline, cache, end - args.frames, end).to(pipeline.device)
                model = pipeline.model.eval()
                model.freqs = model.freqs.to(pipeline.device)
                noise = torch.randn(pipeline.config.out_dim, (args.frames - 1) // stride + 1, pipeline.lat_h, pipeline.lat_w, dtype=pipeline.param_dtype, device=pipeline.device, generator=pipeline.generator)
                noise[:, :pipeline.latent_motion_frames.shape[1]] = pipeline.latent_motion_frames
                kwargs = {'x': noise.unsqueeze(0), 'timestep': pipeline.timesteps[0], 'context': context, 'y': pipeline.ref_img_latent.unsqueeze(0)}
                write('inputs', shapes={k: list(v.shape) for k, v in kwargs.items()}, dtypes={k: str(v.dtype) for k, v in kwargs.items()}, sage=True, composition=composition)
                if args.benchmark_ffn:
                    class Captured(Exception):
                        pass

                    captured = []
                    ffn = model.blocks[0].ffn

                    def capture(_module, inputs):
                        captured.append(inputs[0].detach().clone())
                        raise Captured()

                    hook = ffn.register_forward_pre_hook(capture)
                    try:
                        model(**kwargs)
                    except Captured:
                        pass
                    finally:
                        hook.remove()
                    if len(captured) != 1:
                        raise RuntimeError('FFN input capture failed')
                    sample = captured[0]
                    write('ffn_input', shape=list(sample.shape), dtype=str(sample.dtype))
                    reference = ffn(sample)
                    torch.cuda.synchronize()
                    exported_ffn = torch.export.export(ffn, (sample,))
                    build_start = time.monotonic()
                    engine = torch_tensorrt.dynamo.compile(exported_ffn, arg_inputs=[sample], min_block_size=1, require_full_compilation=True, disable_tf32=True, workspace_size=512 * 1024 * 1024, optimization_level=0, use_fp32_acc=args.ffn_fp32_acc)
                    modules = [type(m).__name__ for m in engine.modules()]
                    if not any('TensorRTModule' in name for name in modules):
                        raise RuntimeError('no actual TensorRT engine')
                    build_s = time.monotonic() - build_start
                    output = engine(sample)
                    torch.cuda.synchronize()
                    delta = (output.float() - reference.float()).abs()
                    if not torch.isfinite(delta).all().item():
                        raise RuntimeError('non-finite FFN output')
                    compiled_ffn = torch.compile(ffn)
                    compiled_output = compiled_ffn(sample)
                    torch.cuda.synchronize()
                    compiled_delta = (compiled_output.float() - reference.float()).abs()
                    for callable_ in (ffn, compiled_ffn, engine):
                        for _ in range(3):
                            callable_(sample)
                    torch.cuda.synchronize()
                    timings = {}
                    for name, callable_ in (('pytorch_eager', ffn), ('pytorch_compiled', compiled_ffn), ('tensorrt', engine)):
                        durations = []
                        for _ in range(5):
                            torch.cuda.synchronize()
                            tick = time.perf_counter()
                            callable_(sample)
                            torch.cuda.synchronize()
                            durations.append(round(time.perf_counter() - tick, 6))
                        timings[name] = durations
                    write('ffn_result', build_seconds=round(build_s, 3), engine_modules=modules, fp32_acc=args.ffn_fp32_acc, max_abs_error=delta.max().item(), mean_abs_error=delta.mean().item(), errors_above_0_01=(delta > 0.01).sum().item(), compiled_max_abs_error=compiled_delta.max().item(), compiled_mean_abs_error=compiled_delta.mean().item(), reference_max_abs=reference.float().abs().max().item(), gpu_peak_reserved_bytes=torch.cuda.max_memory_reserved(), seconds=timings)
                    write('complete', scope='single-ffn-only-not-full-model')
                    return
                exported = torch.export.export(model, (), kwargs=kwargs, strict=not args.non_strict_export)
                write('exported', graph_nodes=len(list(exported.graph.nodes)))
                torch_tensorrt.dynamo.compile(exported, arg_inputs=[], kwarg_inputs=kwargs, dryrun=True, min_block_size=1)
                write('complete', scope='dryrun-only-no-engine-no-performance')
        except BaseException as exc:
            write('failed', error_type=type(exc).__name__, message=str(exc)[:1000])
            raise


if __name__ == '__main__':
    main()
