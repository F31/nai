"""Repeated real VAE encode/decode of a fixed portrait; not a live pipeline."""
import argparse
import json
import time
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--model-dir', required=True, type=Path)
    parser.add_argument('--portrait', required=True, type=Path)
    parser.add_argument('--output-dir', required=True, type=Path)
    parser.add_argument('--size', type=int, required=True)
    parser.add_argument('--rounds', type=int, default=4)
    parser.add_argument('--seed', type=int, default=42)
    args = parser.parse_args()
    if not 256 <= args.size <= 1024 or args.size % 32 or not 1 <= args.rounds <= 8 or not 0 <= args.seed <= 0xffffffff:
        parser.error('size, rounds or seed outside diagnostic bounds')
    if not args.model_dir.is_dir() or not args.portrait.is_file() or args.output_dir.exists():
        parser.error('missing input or output directory exists')
    import numpy as np
    import torch
    from PIL import Image, ImageOps
    from flash_head.ltx_video.ltx_vae import LtxVAE
    if not torch.cuda.is_available():
        raise RuntimeError('CUDA unavailable')
    args.output_dir.mkdir()
    torch.manual_seed(args.seed)
    with Image.open(args.portrait) as source:
        image = ImageOps.fit(ImageOps.exif_transpose(source).convert('RGB'), (args.size, args.size), method=Image.Resampling.LANCZOS)
    array = np.asarray(image).copy()
    with (args.output_dir / 'events.jsonl').open('x', encoding='utf-8', buffering=1) as events:
        def write(event, **fields):
            events.write(json.dumps({'event': event, **fields}, ensure_ascii=False) + '\n')
        try:
            vae = LtxVAE(str(args.model_dir / 'VAE_LTX'), dtype=torch.bfloat16, device='cuda')
            with torch.inference_mode():
                frame = torch.from_numpy(array).to('cuda', dtype=torch.bfloat16).permute(2, 0, 1)[None, :, None]
                video = ((frame / 255 - 0.5) * 2).repeat(1, 1, 9, 1, 1)
                write('ready', size=args.size, rounds=args.rounds, seed=args.seed, frames_per_input=9, scope='static-vae-only-no-color-correction')
                for index in range(args.rounds):
                    start = time.perf_counter()
                    latent = vae.encode(video)
                    video = vae.decode(latent)
                    torch.cuda.synchronize()
                    if video.ndim != 5 or video.shape[2] != 9 or not torch.isfinite(video).all().item():
                        raise RuntimeError('invalid VAE output')
                    pixels = (((video[0, :, -1] + 1) / 2).clamp(0, 1).permute(1, 2, 0) * 255).to(torch.uint8).cpu().numpy()
                    path = args.output_dir / f'round-{index + 1:02}.png'
                    with path.open('xb') as output:
                        Image.fromarray(pixels).save(output, format='PNG')
                    write('round', index=index + 1, seconds=round(time.perf_counter()-start, 4), latent_shape=list(latent.shape), output_shape=list(video.shape), file=path.name)
                write('complete', rounds=args.rounds)
        except BaseException as exc:
            write('failed', error_type=type(exc).__name__, message=str(exc)[:500])
            raise


if __name__ == '__main__':
    main()
