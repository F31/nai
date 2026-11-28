"""Read-only sparse geometry diagnostic; does not produce FeatherTalk landmarks."""
import argparse
import json
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--video', type=Path, required=True)
    parser.add_argument('--output-dir', type=Path, required=True)
    parser.add_argument('--samples', type=int, default=8)
    parser.add_argument('--consecutive-frames', type=int, default=0, help='bounded continuous geometry check, at most 6000 frames')
    parser.add_argument('--start-frame', type=int, default=0)
    parser.add_argument('--roundtrip', action='store_true', help='same-source patch resize/blend diagnostic, not generated lips')
    parser.add_argument('--patch-size', type=int, default=144)
    parser.add_argument('--mouth-padding', type=float, default=1.8)
    parser.add_argument('--save-training-patches', action='store_true', help='export semantic targets and blending masks; not an upstream FeatherTalk dataset')
    args = parser.parse_args()
    if not args.video.is_file() or args.output_dir.exists() or not 2 <= args.samples <= 16:
        parser.error('missing video, existing output or invalid sample count')
    if not 0 <= args.consecutive_frames <= 6000 or args.start_frame < 0:
        parser.error('invalid continuous range')
    if not 64 <= args.patch_size <= 512 or not 1.2 <= args.mouth_padding <= 3:
        parser.error('invalid patch configuration')
    if args.save_training_patches and not args.roundtrip:
        parser.error('patch export requires roundtrip geometry')
    import cv2
    import mediapipe as mp
    import numpy as np
    from PIL import Image, ImageDraw, ImageFilter
    args.output_dir.mkdir(parents=True)
    cap = cv2.VideoCapture(str(args.video))
    count = int(cap.get(cv2.CAP_PROP_FRAME_COUNT))
    fps = cap.get(cv2.CAP_PROP_FPS)
    if count < args.samples or fps <= 0:
        raise ValueError('invalid video metadata')
    records, tiles = [], []
    indices = np.linspace(0, count - 1, args.samples, dtype=int)
    if args.consecutive_frames:
        if args.start_frame + args.consecutive_frames > count:
            raise ValueError('continuous range exceeds video')
        indices = range(args.start_frame, args.start_frame + args.consecutive_frames)
        cap.set(cv2.CAP_PROP_POS_FRAMES, args.start_frame)
    try:
        with mp.solutions.face_mesh.FaceMesh(static_image_mode=True, max_num_faces=2, min_detection_confidence=0.6) as mesh:
            for index in indices:
                if not args.consecutive_frames:
                    cap.set(cv2.CAP_PROP_POS_FRAMES, int(index))
                ok, frame = cap.read()
                if not ok:
                    records.append({'frame': int(index), 'status': 'rejected_decode'})
                    continue
                rgb = cv2.cvtColor(frame, cv2.COLOR_BGR2RGB)
                result = mesh.process(rgb)
                faces = result.multi_face_landmarks or []
                record = {'frame': int(index), 'seconds': round(index / fps, 3), 'faces': len(faces)}
                image = Image.fromarray(rgb)
                draw = ImageDraw.Draw(image)
                if len(faces) == 1:
                    h, w = rgb.shape[:2]
                    points = [(p.x * w, p.y * h) for p in faces[0].landmark]
                    mouth = [points[i] for i in (61, 291, 13, 14)]
                    record['mouth_points'] = mouth
                    record['face_bounds'] = [min(p[0] for p in points), min(p[1] for p in points), max(p[0] for p in points), max(p[1] for p in points)]
                    left, top, right, bottom = record['face_bounds']
                    side = max(right - left, bottom - top)
                    cx, cy = (left + right) / 2, (top + bottom) / 2
                    roi = [int(cx-side/2), int(cy-side/2), int(cx+side/2), int(cy+side/2)]
                    record['roi_xyxy'] = roi
                    record['roi_in_bounds'] = roi[0] >= 0 and roi[1] >= 0 and roi[2] <= w and roi[3] <= h
                    record['mouth_roi_normalized'] = [[(x-roi[0])/(roi[2]-roi[0]), (y-roi[1])/(roi[3]-roi[1])] for x, y in mouth]
                    if args.roundtrip:
                        lip_indices = sorted({i for edge in mp.solutions.face_mesh.FACEMESH_LIPS for i in edge})
                        lips = [points[i] for i in lip_indices]
                        mx = (min(p[0] for p in lips) + max(p[0] for p in lips)) / 2
                        my = (min(p[1] for p in lips) + max(p[1] for p in lips)) / 2
                        side = max(max(p[0] for p in lips)-min(p[0] for p in lips), max(p[1] for p in lips)-min(p[1] for p in lips)) * args.mouth_padding
                        box = (int(mx-side/2), int(my-side/2), int(mx+side/2), int(my+side/2))
                        if not (0 <= box[0] < box[2] <= w and 0 <= box[1] < box[3] <= h):
                            raise ValueError('mouth patch outside frame')
                        original = image.copy()
                        patch = original.crop(box)
                        patch = patch.resize((args.patch_size, args.patch_size), Image.Resampling.LANCZOS).resize(patch.size, Image.Resampling.LANCZOS)
                        mask = Image.new('L', patch.size)
                        inset = max(2, patch.width // 8)
                        ImageDraw.Draw(mask).ellipse((inset, inset, patch.width-inset-1, patch.height-inset-1), fill=255)
                        mask = mask.filter(ImageFilter.GaussianBlur(max(1, patch.width / 24)))
                        if args.save_training_patches:
                            target_name = f'{int(index):06d}-target.png'
                            mask_name = f'{int(index):06d}-blend.png'
                            original.crop(box).resize((args.patch_size, args.patch_size), Image.Resampling.LANCZOS).save(args.output_dir / target_name)
                            mask.resize((args.patch_size, args.patch_size), Image.Resampling.LANCZOS).save(args.output_dir / mask_name)
                            record['semantic_patch'] = {'version': 1, 'target': target_name, 'blend_mask': mask_name, 'size': args.patch_size, 'box_xyxy': box, 'source_frame': int(index), 'source_time_seconds': index / fps, 'upstream_dataset_compatible': False}
                        composite = original.copy()
                        composite.paste(patch, box[:2], mask)
                        delta = np.abs(np.asarray(composite).astype(np.int16)-rgb.astype(np.int16))
                        outside = delta.copy()
                        outside[box[1]:box[3], box[0]:box[2]] = 0
                        if np.any(outside):
                            raise RuntimeError('pixels outside patch modified')
                        record['roundtrip'] = {'scope': 'same-source-not-lipsync', 'box': box, 'patch_size': args.patch_size, 'outside_max_error': int(outside.max()), 'inside_mean_error': float(delta[box[1]:box[3], box[0]:box[2]].mean())}
                        if index == indices[0]:
                            preview = Image.new('RGB', (w * 2, h))
                            preview.paste(original, (0, 0))
                            preview.paste(composite, (w, 0))
                            preview.save(args.output_dir / 'roundtrip-comparison.png')
                    record['status'] = 'geometry_only_not_pose_or_occlusion_accepted'
                    draw.rectangle(record['face_bounds'], outline='lime', width=3)
                    for x, y in mouth:
                        draw.ellipse((x-5, y-5, x+5, y+5), fill='red')
                else:
                    record['status'] = 'rejected_face_count'
                image.thumbnail((480, 270))
                tile = Image.new('RGB', (480, 294), 'black')
                tile.paste(image, (0, 24))
                ImageDraw.Draw(tile).text((8, 5), f'{record["seconds"]}s faces={len(faces)}', fill='white')
                if len(tiles) < 16 and (not args.consecutive_frames or (index - args.start_frame) % max(1, args.consecutive_frames // 12) == 0):
                    tiles.append(tile)
                records.append(record)
    finally:
        cap.release()
    if not tiles:
        raise RuntimeError('no samples decoded')
    sheet = Image.new('RGB', (480 * 2, 294 * ((len(tiles) + 1) // 2)), 'black')
    for i, tile in enumerate(tiles):
        sheet.paste(tile, ((i % 2) * 480, (i // 2) * 294))
    sheet.save(args.output_dir / 'geometry-review.jpg')
    (args.output_dir / 'report.json').write_text(json.dumps({'scope': 'sparse-geometry-only', 'fps': fps, 'frames': count, 'mediapipe': mp.__version__, 'samples': records}, ensure_ascii=False, indent=2), encoding='utf-8')


if __name__ == '__main__':
    main()
