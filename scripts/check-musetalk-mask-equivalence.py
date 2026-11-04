"""Check current-process per-frame masks with the real local parser; no synthesis."""
import json
import os
from pathlib import Path
import time
import cv2
import numpy as np
import torch
import musetalk_reference_cache as cache

root = Path(__file__).resolve().parents[1] / "target-windows/realistic-avatar-research"
source = root / "MuseTalk"
import sys
sys.path[:0] = [str(source), str(source / "musetalk/utils")]
os.chdir(root / "offline-reference")
from musetalk.utils.blending import get_image
from musetalk.utils.face_parsing import FaceParsing
from musetalk.utils.face_parsing.resnet import Resnet18
import musetalk_reference_masks as masks

video = root / "video-inspection/authorized-clip-18s-5s.mp4"
runner = Path(__file__).resolve().parent / "run-musetalk-reference-sample.py"
cache_dir = root / "video-inspection/location-cache"
runner_hash, video_hash = cache.file_sha256(runner), cache.file_sha256(video)
identities = []
for path in cache_dir.glob("*.json"):
    try:
        item = json.loads(path.read_bytes())
        identity = item["identity"]
        if identity.get("runner_sha256") == runner_hash and identity.get("video_sha256") == video_hash:
            boxes = cache.load(cache_dir, identity, 960, 540)
            if boxes is not None: identities.append(boxes)
    except (OSError, ValueError, KeyError, TypeError):
        continue
if len(identities) != 1:
    raise RuntimeError("unique verified current-runner location cache unavailable")
boxes = identities[0]
cap = cv2.VideoCapture(str(video))
frames = []
for i in range(120):
    ok, frame = cap.read()
    if not ok or frame.shape != (540, 960, 3): raise RuntimeError(f"bad frame {i}")
    frames.append(frame)
cap.release()
original = Resnet18.init_weight
try:
    Resnet18.init_weight = lambda self, model_path: None
    parser = FaceParsing(left_cheek_width=90, right_cheek_width=90)
finally:
    Resnet18.init_weight = original
torch.cuda.synchronize()
start = time.monotonic()
materials, size = masks.prepare_all(frames, boxes, parser)
torch.cuda.synchronize()
preparation = time.monotonic() - start
start = time.monotonic()
blend_seconds = 0.0
reference_seconds = 0.0
for frame, box, material in zip(frames, boxes, materials):
    x, y, x1, y1 = box
    face = 255 - frame[y:y1, x:x1]  # deterministic stress input, not a generated avatar
    part = time.monotonic()
    actual = masks.blend(frame, face, box, material)
    blend_seconds += time.monotonic() - part
    part = time.monotonic()
    expected = get_image(frame, face, box, mode="jaw", fp=parser)
    torch.cuda.synchronize()
    reference_seconds += time.monotonic() - part
    if not np.array_equal(actual, expected): raise RuntimeError("mask blend differs")
torch.cuda.synchronize()
print(json.dumps({"frames": len(frames), "exact": True, "mask_bytes": size,
                  "mask_prepare_seconds": round(preparation, 3),
                  "comparison_seconds": round(time.monotonic()-start, 3),
                  "reuse_blend_seconds": round(blend_seconds, 3), "original_blend_seconds": round(reference_seconds, 3),
                  "note": "real segmentation; artificial face input; no UNet, no speech or output sample"}))
