"""Small, isolated GPU compiler capability probe; does not load FlashHead."""
import argparse
import json
import time
from pathlib import Path

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--size', type=int, default=256)
    args = parser.parse_args()
    if not 16 <= args.size <= 2048 or args.output.exists():
        parser.error('invalid size or output already exists')
    import torch
    if not torch.cuda.is_available(): raise RuntimeError('CUDA unavailable')
    compiled = torch.compile(lambda a, b: a @ b)
    a = torch.randn((args.size, args.size), device='cuda')
    b = torch.randn((args.size, args.size), device='cuda')
    start = time.perf_counter()
    result = compiled(a, b)
    torch.cuda.synchronize()
    expected = a @ b
    error = float((result - expected).abs().max())
    finite = bool(torch.isfinite(result).all())
    report = {'torch_version':torch.__version__, 'gpu':torch.cuda.get_device_name(0), 'size':args.size, 'finite':finite, 'max_absolute_error':error, 'elapsed_seconds':round(time.perf_counter()-start,3)}
    with args.output.open('x', encoding='utf8') as stream: json.dump(report, stream, indent=2)
    if not finite or error > 1e-3: raise RuntimeError('compiled output differs from eager output')
    print(json.dumps(report))

if __name__ == '__main__': main()
