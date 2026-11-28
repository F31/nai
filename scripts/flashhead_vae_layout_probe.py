"""Compare LTX VAE execution options on one reference latent, not a live pipeline."""
import argparse
import json
import math
import time
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--model-dir', required=True, type=Path)
    parser.add_argument('--portrait', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--size', required=True, type=int)
    parser.add_argument('--repeats', type=int, default=3)
    parser.add_argument('--frames', type=int, default=33)
    parser.add_argument('--candidate', choices=('channels-last-3d', 'cudnn-benchmark', 'compile-reduce-overhead'), default='channels-last-3d')
    parser.add_argument('--baseline-compile', action='store_true', help='compare the already-compiled default decoder against the candidate')
    args = parser.parse_args()
    if not 256 <= args.size <= 1024 or args.size % 32 or not 2 <= args.repeats <= 6 or not 1 <= args.frames <= 97:
        parser.error('size or repeat count out of bounds')
    if args.output.exists() or not args.model_dir.is_dir() or not args.portrait.is_file():
        parser.error('output exists or input missing')
    with (args.model_dir / 'Model_Lite' / 'config.json').open(encoding='utf-8') as config_file:
        stride = json.load(config_file)['vae_stride'][0]
    if stride <= 0 or (args.frames - 1) % stride:
        parser.error('frames must match model temporal stride')

    import numpy as np
    import torch
    from PIL import Image, ImageOps
    from flash_head.ltx_video.ltx_vae import LtxVAE
    if not torch.cuda.is_available():
        raise RuntimeError('CUDA unavailable')
    torch.manual_seed(42)
    torch.backends.cudnn.benchmark = False
    with Image.open(args.portrait) as source:
        image = ImageOps.fit(ImageOps.exif_transpose(source).convert('RGB'), (args.size, args.size), method=Image.Resampling.LANCZOS)
    pixels = np.asarray(image).copy()
    report = {'scope': 'vae-reference-latent-only', 'size': args.size, 'frames': args.frames, 'repeats': args.repeats, 'candidate': args.candidate, 'baseline_compile': args.baseline_compile, 'gpu': torch.cuda.get_device_name(), 'torch': torch.__version__}
    with args.output.open('x', encoding='utf-8', buffering=1) as out:
        def write(event, **fields):
            out.write(json.dumps({'event': event, **fields}, ensure_ascii=False) + '\n')
        try:
            vae = LtxVAE(str(args.model_dir / 'VAE_LTX'), dtype=torch.bfloat16, device='cuda')
            with torch.inference_mode():
                x = torch.from_numpy(pixels).to('cuda', dtype=torch.bfloat16).permute(2, 0, 1).unsqueeze(0).unsqueeze(2)
                x = (x / 255 - 0.5) * 2
                latent = vae.encode(x.repeat(1, 1, args.frames, 1, 1))
                torch.cuda.synchronize()
                write('ready', **report, latent_shape=list(latent.shape))

                def measure(label):
                    warm_start = time.perf_counter()
                    result = vae.decode(latent)
                    torch.cuda.synchronize()
                    warmup = time.perf_counter() - warm_start
                    durations = []
                    for _ in range(args.repeats):
                        start = time.perf_counter()
                        result = vae.decode(latent)
                        torch.cuda.synchronize()
                        durations.append(time.perf_counter() - start)
                    if not torch.isfinite(result).all().item():
                        raise RuntimeError('decoder produced non-finite values')
                    write('timing', layout=label, warmup_seconds=round(warmup, 6), seconds=[round(v, 6) for v in durations])
                    return result.float().cpu()

                original_decode = vae.decode
                if args.baseline_compile:
                    vae.decode = torch.compile(original_decode)
                baseline = measure('compiled-default' if args.baseline_compile else 'contiguous')
                count = sum(isinstance(m, torch.nn.Conv3d) for m in vae.model.modules())
                if args.candidate == 'channels-last-3d':
                    torch.nn.utils.convert_conv3d_weight_memory_format(vae.model, torch.channels_last_3d)
                    original = vae.un_normalize_latents
                    vae.un_normalize_latents = lambda value: original(value).contiguous(memory_format=torch.channels_last_3d)
                elif args.candidate == 'cudnn-benchmark':
                    torch.backends.cudnn.benchmark = True
                else:
                    vae.decode = torch.compile(original_decode, mode='reduce-overhead')
                candidate = measure(args.candidate)
                if candidate.shape != baseline.shape:
                    raise RuntimeError('decoder output shape mismatch')
                delta = (candidate - baseline).abs()
                max_error, mean_error = float(delta.max()), float(delta.mean())
                if not math.isfinite(max_error) or not math.isfinite(mean_error):
                    raise RuntimeError('decoder comparison non-finite')
                write('comparison', conv3d_modules=count, shape=list(candidate.shape), max_absolute_error=max_error, mean_absolute_error=mean_error, gpu_peak_reserved_bytes=torch.cuda.max_memory_reserved())
            write('complete')
        except BaseException as exc:
            write('failed', error_type=type(exc).__name__, message=str(exc)[:500])
            raise


if __name__ == '__main__':
    main()
