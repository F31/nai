"""Pointwise conversion of only the already validated, retained tensor prefix."""
def to_host_prefix(frames, remaining, dtype):
    if not isinstance(remaining, int) or remaining <= 0 or len(frames.shape) != 4:
        raise ValueError('FRAME_PREFIX')
    count = min(remaining, int(frames.shape[0]))
    if count <= 0:
        raise ValueError('FRAME_PREFIX')
    return frames[:count].to(dtype=dtype).cpu().numpy()
