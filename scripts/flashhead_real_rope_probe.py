"""Diagnostic real-valued RoPE; requires caller to supply real/imag frequency pairs."""
import torch


def rope_apply_real(x, freqs, grid_sizes, use_usp=False, sp_size=1, sp_rank=0):
    s, n, c = x.size(1), x.size(2), x.size(3) // 2
    split = freqs.split([c - 2 * (c // 3), c // 3, c // 3], dim=1)
    f, h, w = grid_sizes
    seq_len = f * h * w
    factors = torch.cat([
        split[0][:f].view(f, 1, 1, -1, 2).expand(f, h, w, -1, 2),
        split[1][:h].view(1, h, 1, -1, 2).expand(f, h, w, -1, 2),
        split[2][:w].view(1, 1, w, -1, 2).expand(f, h, w, -1, 2),
    ], dim=-2).reshape(seq_len, 1, -1, 2)
    if use_usp:
        padding = torch.zeros((s * sp_size - seq_len, 1, c, 2), dtype=factors.dtype, device=factors.device)
        padding[..., 0] = 1
        factors = torch.cat([factors, padding], dim=0)[sp_rank * s:(sp_rank + 1) * s]
    pairs = x[0, :s].float().reshape(s, n, -1, 2)
    real = pairs[..., 0] * factors[..., 0] - pairs[..., 1] * factors[..., 1]
    imag = pairs[..., 0] * factors[..., 1] + pairs[..., 1] * factors[..., 0]
    result = torch.stack((real, imag), dim=-1).flatten(2)
    result = torch.cat((result, x[0, s:] if use_usp else x[0, seq_len:]))
    return result.unsqueeze(0).to(x.dtype)
