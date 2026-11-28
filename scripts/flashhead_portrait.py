"""Explicit single-face composition for local FlashHead diagnostics."""
import math

def resize_crop_preserving_aspect(image, size):
    """Pad a clipped crop with edge pixels before square resize; never stretch a face."""
    import numpy as np
    from PIL import Image
    pixels = np.asarray(image.convert('RGB'))
    height, width = pixels.shape[:2]
    if not width or not height or not isinstance(size, int) or size <= 0:
        raise ValueError('invalid crop image or output size')
    side = max(width, height)
    top = (side - height) // 2
    left = (side - width) // 2
    padded = np.pad(pixels, ((top, side-height-top), (left, side-width-left), (0, 0)), mode='edge')
    return Image.fromarray(padded).resize((size, size), Image.Resampling.LANCZOS)

def crop_box(boxes, scores, width, height, ratio, confidence):
    if width <= 0 or height <= 0 or not math.isfinite(ratio) or not 1.5 <= ratio <= 4.0 or not math.isfinite(confidence) or not 0 <= confidence <= 1:
        raise ValueError('invalid portrait composition parameters')
    if len(boxes) != len(scores):
        raise ValueError('face detector result mismatch')
    selected = [(box, score) for box, score in zip(boxes, scores) if math.isfinite(float(score)) and score >= confidence]
    if len(selected) != 1:
        raise ValueError(f'expected one qualified face, found {len(selected)}')
    box, score = selected[0]
    if len(box) != 4 or any(not math.isfinite(float(value)) for value in box):
        raise ValueError('invalid face box')
    x1, y1, x2, y2 = (float(v) for v in box)
    if not (0 <= x1 < x2 <= 1 and 0 <= y1 < y2 <= 1):
        raise ValueError('face box extends outside portrait')
    face_width = (x2 - x1) * width
    center_x, center_y = (x1 + x2) * width / 2, (y1 + y2) * height / 2
    side = face_width * ratio
    result = (int(max(0, center_x-side*0.5)), int(max(0, center_y-side*0.55)), int(min(width, center_x+side*0.5)), int(min(height, center_y+side*0.45)))
    if result[2] <= result[0] or result[3] <= result[1]:
        raise ValueError('empty portrait crop')
    return result, float(score)

def prepare_portrait(path, ratio, confidence, size):
    if not isinstance(size, int) or not 256 <= size <= 1024:
        raise ValueError('invalid crop output size')
    import numpy as np
    from PIL import Image, ImageOps
    from flash_head.utils.cpu_face_handler import CPUFaceHandler
    with Image.open(path) as image:
        image = ImageOps.exif_transpose(image).convert('RGB')
        detector = CPUFaceHandler(min_detection_confidence=confidence)
        boxes, scores = detector(np.asarray(image))
        box, score = crop_box(boxes, scores, image.width, image.height, ratio, confidence)
        return resize_crop_preserving_aspect(image.crop(box), size), {'crop_box': box, 'confidence': round(score, 4), 'ratio': ratio, 'crop_size': size, 'resize_mode': 'edge_pad_preserve_aspect'}
