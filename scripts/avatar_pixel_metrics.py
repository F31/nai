"""Streaming RGB error metrics; not a perceptual/mouth-sync acceptance gate."""
import hashlib
import math


def compare(left, right, width, height, frames, expected_hashes):
    import numpy as np
    size = width*height*4
    if (any(not isinstance(n, int) or not 1 <= n <= 1024 for n in (width, height))
            or not 1 <= frames <= 150 or size*frames > 268435456):
        raise ValueError('METRICS_LIMIT')
    sums, squares, maximum, identical = 0., 0., 0, 0
    hashes = [hashlib.sha256(), hashlib.sha256()]
    per_frame = []
    with left.open('rb') as a, right.open('rb') as b:
        for index in range(frames):
            raw = [a.read(size), b.read(size)]
            if any(len(value) != size for value in raw):
                raise ValueError('METRICS_TRUNCATED')
            for digest, value in zip(hashes, raw): digest.update(value)
            x, y = [np.frombuffer(value, dtype=np.uint8).reshape(height,width,4)[:,:,:3].astype(np.float32) for value in raw]
            delta = np.abs(x-y)
            absolute = float(delta.sum(dtype=np.float64))
            squared = float(np.square(delta).sum(dtype=np.float64))
            peak = int(delta.max())
            sums += absolute; squares += squared; maximum = max(maximum, peak)
            identical += raw[0] == raw[1]
            mse = squared/(width*height*3)
            per_frame.append({'frame': index, 'mae': absolute/(width*height*3), 'rmse': math.sqrt(mse),
                              'psnr_db': None if mse == 0 else 10*math.log10(255**2/mse), 'max_abs': peak})
        if a.read(1) or b.read(1): raise ValueError('METRICS_TRAILING')
    if [h.hexdigest() for h in hashes] != list(expected_hashes):
        raise ValueError('METRICS_HASH')
    count = width*height*3*frames
    mse = squares/count
    return {'scope': 'rgb_pixel_difference_not_visual_quality_pass', 'frames': frames, 'identical_frames': identical,
            'mae_0_255': sums/count, 'rmse_0_255': math.sqrt(mse), 'max_abs_0_255': maximum,
            'psnr_db': None if mse == 0 else 10*math.log10(255**2/mse), 'per_frame': per_frame}
