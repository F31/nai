"""Offline reference-image VAE reconstruction probe, not a live quality gate."""
import argparse
import json
import time
from pathlib import Path

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--model-dir', required=True, type=Path)
    parser.add_argument('--wav2vec-dir', required=True, type=Path)
    parser.add_argument('--portrait', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--report', required=True, type=Path)
    parser.add_argument('--size', required=True, type=int)
    args = parser.parse_args()
    if not 256 <= args.size <= 1024 or args.size % 32:
        parser.error('size must be a multiple of 32 between 256 and 1024')
    if args.output.exists() or args.report.exists():
        parser.error('output already exists')
    for path in (args.model_dir, args.wav2vec_dir, args.portrait):
        if not path.exists(): parser.error(f'missing input: {path}')
    import torch
    from PIL import Image
    from flash_head import inference
    inference.infer_params['width'] = args.size
    inference.infer_params['height'] = args.size
    start = time.perf_counter()
    if not torch.cuda.is_available(): raise RuntimeError('CUDA unavailable')
    pipeline = inference.get_pipeline(world_size=1, ckpt_dir=str(args.model_dir), model_type='lite', wav2vec_dir=str(args.wav2vec_dir))
    inference.get_base_data(pipeline, cond_image_path_or_dir=str(args.portrait), base_seed=42, use_face_crop=False)
    with torch.no_grad():
        decoded = pipeline.vae.decode(pipeline.ref_img_latent)
        torch.cuda.synchronize()
        pixels = (((decoded[0, :, 0] + 1) / 2).clamp(0, 1).permute(1, 2, 0) * 255).to(torch.uint8).cpu().numpy()
    with args.output.open('xb') as stream: Image.fromarray(pixels).save(stream, format='PNG')
    report = {'size': args.size, 'elapsed_s': round(time.perf_counter()-start, 3), 'frame_shape': list(pixels.shape), 'scope': 'reference_vae_roundtrip_only'}
    with args.report.open('x', encoding='utf8') as stream: json.dump(report, stream, ensure_ascii=False, indent=2)
    print(json.dumps(report))

if __name__ == '__main__': main()
