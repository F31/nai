"""Exact per-frame MuseTalk jaw mask preparation for bounded offline research.

Masks exist only in the current process and must stay paired with the frame and
box that produced them. This does not implement an authorized production cache.
"""
import cv2
import numpy as np
from PIL import Image
from musetalk.utils.blending import face_seg, get_crop_box, get_image_blending

MAX_MASK_BYTES = 64 * 1024 * 1024


def prepare_mask(image, box, parser):
    body = Image.fromarray(image[:, :, ::-1])
    x, y, x1, y1 = box
    crop_box, _ = get_crop_box(box, 1.5)
    x_s, y_s, _, _ = crop_box
    size = body.crop(crop_box).size
    mask = face_seg(body.crop(crop_box), mode="jaw", fp=parser)
    if mask is None:
        raise RuntimeError("face segmentation returned no mask")
    small = mask.crop((x - x_s, y - y_s, x1 - x_s, y1 - y_s))
    mask = Image.new("L", size, 0)
    mask.paste(small, (x - x_s, y - y_s, x1 - x_s, y1 - y_s))
    width, height = size
    top = int(height * 0.5)
    modified = Image.new("L", size, 0)
    modified.paste(mask.crop((0, top, width, height)), (0, top))
    kernel = int(0.05 * width // 2 * 2) + 1
    return cv2.GaussianBlur(np.array(modified), (kernel, kernel), 0), crop_box


def prepare_all(frames, boxes, parser):
    if len(frames) != len(boxes) or not 0 < len(frames) <= 120:
        raise ValueError("expected paired bounded frames and boxes")
    materials, total = [], 0
    for image, box in zip(frames, boxes):
        mask, crop = prepare_mask(image, box, parser)
        total += mask.nbytes
        if total > MAX_MASK_BYTES:
            raise ValueError("mask memory budget exceeded")
        materials.append((mask, crop))
    return materials, total


def blend(image, face, box, material):
    mask, crop = material
    return get_image_blending(image, face, box, mask, crop)
