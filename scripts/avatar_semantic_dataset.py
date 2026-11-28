"""Experimental semantic patches; not a FeatherTalk checkpoint/data compatibility layer."""
import json
from pathlib import Path

import numpy as np
from PIL import Image


def audio_window(features, source_frame, fps, feature_hz=50, half_window_frames=10):
    if fps <= 0 or feature_hz <= 0 or feature_hz % fps or source_frame < 0 or half_window_frames < 1:
        raise ValueError('invalid audio/video timebase')
    if features.ndim != 2 or features.shape[1] != 1024 or len(features) == 0:
        raise ValueError('expected finite [tokens,1024] audio features')
    stride = feature_hz // fps
    center = source_frame * stride
    if center >= len(features):
        raise ValueError('audio does not cover source frame')
    indices = np.arange(center-half_window_frames*stride, center+half_window_frames*stride)
    result = features[np.clip(indices, 0, len(features)-1)].astype(np.float32, copy=True)
    if not np.isfinite(result).all():
        raise ValueError('non-finite audio window')
    return result


class SemanticDataset:
    def __init__(self, directory, features, feature_hz=50, half_window_frames=10, exclude_rejected=False):
        self.directory = Path(directory).resolve()
        manifest = json.loads((self.directory / 'report.json').read_text(encoding='utf-8'))
        self.fps = manifest['fps']
        if int(self.fps) != self.fps:
            raise ValueError('non-integral fps unsupported by this diagnostic')
        self.fps = int(self.fps)
        self.records = manifest['samples']
        self.rejected = [r for r in self.records if 'semantic_patch' not in r]
        if exclude_rejected:
            self.records = [r for r in self.records if 'semantic_patch' in r]
        self.features = features
        self.feature_hz = feature_hz
        self.half_window_frames = half_window_frames
        previous = -1
        for record in self.records:
            patch = record.get('semantic_patch', {})
            frame = patch.get('source_frame', -1)
            if patch.get('version') != 1 or frame != record['frame'] or frame <= previous:
                raise ValueError('invalid semantic frame identity')
            if abs(patch['source_time_seconds'] - frame / self.fps) > 1e-6:
                raise ValueError('source timestamp mismatch')
            audio_window(features, frame, self.fps, feature_hz, half_window_frames)
            for name in ('target', 'blend_mask'):
                path = (self.directory / patch[name]).resolve()
                if not path.is_relative_to(self.directory) or not path.is_file():
                    raise ValueError('patch file outside dataset or missing')
            previous = frame

    def __len__(self):
        return len(self.records)

    def __getitem__(self, index):
        patch = self.records[index]['semantic_patch']
        with Image.open(self.directory / patch['target']) as image:
            target = np.asarray(image.convert('RGB'), dtype=np.float32) / 255
        with Image.open(self.directory / patch['blend_mask']) as image:
            blend = np.asarray(image.convert('L'), dtype=np.float32) / 255
        if target.shape != (patch['size'], patch['size'], 3) or blend.shape != target.shape[:2]:
            raise ValueError('patch dimensions mismatch')
        return {'source_frame': patch['source_frame'], 'box_xyxy': patch['box_xyxy'], 'target': target.transpose(2, 0, 1).copy(), 'blend_mask': blend[None].copy(), 'audio': audio_window(self.features, patch['source_frame'], self.fps, self.feature_hz, self.half_window_frames)}
