"""Create a face-scale-normalized comparison; not an automatic quality verdict."""
import argparse
import json
from pathlib import Path


def face_relative_box(points, width, height):
    """Use facial landmarks to crop a fixed fraction of face width around the mouth."""
    import math
    if width <= 0 or height <= 0 or len(points) <= 454:
        raise ValueError('invalid image or landmarks')
    face_width = abs(points[454].x - points[234].x) * width
    center_x = (points[61].x + points[291].x) * width / 2
    center_y = (points[13].y + points[14].y) * height / 2
    if not all(math.isfinite(x) for x in (face_width, center_x, center_y)) or face_width < 8:
        raise ValueError('invalid face geometry')
    left = round(center_x - face_width * 0.30)
    top = round(center_y - face_width * 0.16)
    right = round(center_x + face_width * 0.30)
    bottom = round(center_y + face_width * 0.16)
    if left < 0 or top < 0 or right > width or bottom > height or left >= right or top >= bottom:
        raise ValueError('mouth crop outside image')
    return left, top, right, bottom


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference', required=True, type=Path)
    parser.add_argument('--vae', required=True, type=Path)
    parser.add_argument('--generated', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--report', required=True, type=Path)
    args = parser.parse_args()
    if args.output.exists() or args.report.exists():
        parser.error('output already exists')
    import numpy as np
    import mediapipe as mp
    from PIL import Image, ImageDraw, ImageOps
    sources = [('REFERENCE', args.reference), ('VAE ONLY', args.vae), ('SPEAKING FRAME', args.generated)]
    sheet = Image.new('RGB', (960, 210), 'white')
    draw = ImageDraw.Draw(sheet)
    details = []
    with mp.solutions.face_mesh.FaceMesh(static_image_mode=True, max_num_faces=1, min_detection_confidence=0.5) as mesh:
        for index, (label, path) in enumerate(sources):
            with Image.open(path) as source:
                image = ImageOps.exif_transpose(source).convert('RGB')
            found = mesh.process(np.asarray(image)).multi_face_landmarks
            if not found or len(found) != 1:
                raise ValueError(f'face not found: {label}')
            box = face_relative_box(found[0].landmark, image.width, image.height)
            crop = image.crop(box).resize((320, 170), Image.Resampling.LANCZOS)
            sheet.paste(crop, (index * 320, 30))
            draw.text((index * 320 + 8, 8), label, fill='black')
            details.append({'label': label, 'path': str(path), 'mouth_crop': box, 'image_size': image.size})
    with args.output.open('xb') as output:
        sheet.save(output, format='PNG')
    with args.report.open('x', encoding='utf-8') as report:
        json.dump({'scope': 'single-frame-comparison-not-quality-verdict', 'items': details}, report, indent=2)


if __name__ == '__main__':
    main()
