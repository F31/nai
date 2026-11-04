"""Research-only GPU-side byte packing matching MuseTalk's CPU conversion."""
import torch

def pack(decoded):
    image = (decoded / 2 + 0.5).clamp(0, 1)
    image = image.detach().permute(0, 2, 3, 1).float()
    return (image * 255).round().to(torch.uint8).cpu().numpy()[..., ::-1]

def decode(vae, latents):
    scaled = (1 / vae.scaling_factor) * latents
    return pack(vae.vae.decode(scaled.to(vae.vae.dtype)).sample)
