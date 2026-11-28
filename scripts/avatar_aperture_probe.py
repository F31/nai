"""CPU probe for audio-only mouth aperture supervision on a contiguous holdout."""
import argparse
import json
from pathlib import Path

import numpy as np

from avatar_semantic_dataset import audio_window


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--dataset', type=Path, required=True)
    parser.add_argument('--features', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--predictions', type=Path)
    args = parser.parse_args()
    manifest = json.loads((args.dataset / 'report.json').read_text(encoding='utf-8'))
    if manifest['fps'] != 25:
        raise ValueError('expected 25 fps')
    features = np.load(args.features, allow_pickle=False).reshape(-1, 1024)
    rows = []
    for record in manifest['samples']:
        if 'semantic_patch' not in record:
            continue
        left, right, upper, lower = np.asarray(record['mouth_points'], dtype=float)
        width = np.linalg.norm(right - left)
        if not np.isfinite(width) or width <= 0:
            raise ValueError('invalid lip width')
        aperture = np.linalg.norm(lower - upper) / width
        if not np.isfinite(aperture):
            raise ValueError('invalid lip aperture')
        rows.append((int(record['frame']), aperture))
    if len(rows) < 100 or len(rows) > 10000:
        raise ValueError('unexpected dataset size')
    split = int(len(rows) * .8)
    boundary = rows[split][0]
    train = [(frame, y) for frame, y in rows[:split] if frame < boundary - 20]
    valid = [(frame, y) for frame, y in rows[split:] if frame + 25 <= rows[-1][0]]

    def matrix(items, offset=0):
        return np.stack([audio_window(features, frame + offset, 25)[16:24].mean(axis=0)
                         for frame, _ in items]).astype(np.float64)

    x = matrix(train)
    target = np.array([y for _, y in train])
    mean = x.mean(axis=0)
    scale = x.std(axis=0).clip(min=1e-6)
    x = (x - mean) / scale
    center = target.mean()
    ridge = x.T @ x / len(x)
    ridge.flat[::ridge.shape[0]+1] += 1
    weights = np.linalg.solve(ridge, x.T @ (target - center) / len(x))
    actual = np.array([y for _, y in valid])

    def predict(offset):
        return ((matrix(valid, offset) - mean) / scale) @ weights + center

    def assess(offset):
        predicted = predict(offset)
        return {'mae': float(np.abs(predicted - actual).mean()),
                'correlation': float(np.corrcoef(predicted, actual)[0, 1])}

    result = {'scope': 'audio_only_aperture_not_renderer', 'train_frames': len(train),
              'holdout_frames': len(valid), 'first_holdout_frame': boundary,
              'matched': assess(0), 'shifted_25_frames': assess(25),
              'train_mean_mae': float(np.abs(center - actual).mean())}
    with args.output.open('x', encoding='utf-8') as out:
        json.dump(result, out, indent=2, allow_nan=False)
    if args.predictions:
        with args.predictions.open('x', encoding='utf-8') as out:
            json.dump([{'frame': frame, 'source_aperture': aperture, 'predicted_aperture': float(value)}
                       for (frame, aperture), value in zip(valid, predict(0), strict=True)], out, allow_nan=False)
    print(json.dumps(result, allow_nan=False))


if __name__ == '__main__':
    main()
