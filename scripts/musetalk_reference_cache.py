"""Bounded location cache for offline experiments, never an authorization."""
import hashlib
import json
from pathlib import Path

MAX_BYTES = 32768
FRAME_COUNT = 120


def file_sha256(path):
    digest = hashlib.sha256()
    with Path(path).open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def key(identity):
    encoded = json.dumps(identity, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode()
    return hashlib.sha256(encoded).hexdigest()


def valid_boxes(boxes, width, height):
    if not isinstance(boxes, list) or len(boxes) != FRAME_COUNT:
        return False
    for box in boxes:
        if not isinstance(box, list) or len(box) != 4 or any(type(x) is not int for x in box):
            return False
        x1, y1, x2, y2 = box
        if not (0 <= x1 < x2 <= width and 0 <= y1 < y2 <= height):
            return False
    return True


def boxes_digest(boxes):
    return hashlib.sha256(json.dumps(boxes, separators=(",", ":")).encode()).hexdigest()


def load(cache_dir, identity, width, height):
    path = Path(cache_dir) / (key(identity) + ".json")
    try:
        if path.is_symlink() or path.stat().st_size > MAX_BYTES:
            return None
        value = json.loads(path.read_bytes())
        if not isinstance(value, dict) or set(value) != {"identity", "boxes", "boxes_sha256"}:
            return None
        if (value["identity"] != identity or not valid_boxes(value["boxes"], width, height)
                or value["boxes_sha256"] != boxes_digest(value["boxes"])):
            return None
        return value["boxes"]
    except (OSError, ValueError, TypeError):
        return None


def save(cache_dir, identity, boxes, width, height):
    if not valid_boxes(boxes, width, height):
        raise ValueError("invalid boxes")
    directory = Path(cache_dir)
    directory.mkdir(parents=True, exist_ok=True)
    path = directory / (key(identity) + ".json")
    payload = json.dumps({"identity": identity, "boxes": boxes, "boxes_sha256": boxes_digest(boxes)}, sort_keys=True, separators=(",", ":")).encode()
    if len(payload) > MAX_BYTES:
        raise ValueError("cache too large")
    try:
        # Exclusive creation means a concurrent or damaged record is never replaced.
        # An interrupted write is rejected on read and never used as a hit.
        with path.open("xb") as stream:
            stream.write(payload)
            stream.flush()
            os_fsync(stream)
        return True
    except FileExistsError:
        return False


def os_fsync(stream):
    import os
    os.fsync(stream.fileno())
