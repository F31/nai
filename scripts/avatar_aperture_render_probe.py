"""Explicit aperture control probe on existing annotated portrait patches."""
import argparse
import json
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw


def render(source, center, radii, factor):
    yy, xx = np.indices(source.shape[:2], dtype=float)
    cx, cy = center
    rx, ry = radii
    dx, dy = (xx-cx)/rx, (yy-cy)/ry
    active = (np.abs(dx) < 1) & (np.abs(dy) < 1)
    # A piecewise linear inverse maps the controlled central strip back to
    # its source extent and keeps the outer boundary stationary.
    weight = np.maximum(0, 1-dx**2)**2
    old_core = .36
    new_core = old_core * (1 + (factor-1)*weight)
    distance = np.abs(dy)
    mapped = np.where(distance <= new_core, distance*old_core/new_core,
                      old_core + (distance-new_core)*(1-old_core)/(1-new_core))
    sample_y = np.clip(cy + np.sign(dy)*mapped*ry, 0, source.shape[0]-1)
    lo = sample_y.astype(int)
    hi = np.minimum(lo+1, source.shape[0]-1)
    blend = (sample_y-lo)[..., None]
    columns = xx.astype(int)
    sampled = np.rint(source[lo,columns]*(1-blend) + source[hi,columns]*blend).astype(np.uint8)
    out = source.copy()
    out[active] = sampled[active]
    assert np.array_equal(out[~active],source[~active])
    return out, int(np.count_nonzero(np.any(out != source, axis=2)))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--dataset', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    records = json.loads((args.dataset/'report.json').read_text(encoding='utf-8'))['samples']
    valid = [row for row in records if 'semantic_patch' in row]
    indices = [int(len(valid)*.8)+offset for offset in (25,75,125,175)]
    factors = (.65, 1., 1.45)
    args.output.mkdir()
    sheet = Image.new('RGB',(432,168*len(indices)))
    draw = ImageDraw.Draw(sheet)
    report = []
    for y,index in enumerate(indices):
        record = valid[index]
        patch = record['semantic_patch']
        x0,y0,x1,y1 = patch['box_xyxy']
        path = (args.dataset/patch['target']).resolve()
        if not path.is_relative_to(args.dataset.resolve()):
            raise ValueError('target outside dataset')
        with Image.open(path) as image:
            source = np.array(image.convert('RGB'))
        if source.shape != (144,144,3):
            raise ValueError('expected 144px RGB patch')
        left,right,upper,lower = (np.asarray(record['mouth_points'], dtype=float)-[x0,y0])*[144/(x1-x0),144/(y1-y0)]
        center = (upper+lower)/2
        width = np.linalg.norm(right-left)
        radii = (width*.63,width*.39)
        if not np.isfinite([*center,width]).all() or width <= 0:
            raise ValueError('invalid lip geometry')
        for x,factor in enumerate(factors):
            out,changed=render(source,center,radii,factor)
            if factor == 1 and changed:
                raise ValueError('neutral factor changed pixels')
            sheet.paste(Image.fromarray(out),(x*144,y*168+24))
            draw.text((x*144+2,y*168+4),f'{record["frame"]}  aperture x{factor:g}',fill='white')
            report.append({'source_frame':record['frame'],'factor':factor,'changed_pixels':changed,'outside_changed_pixels':0})
    sheet.save(args.output/'review.png')
    (args.output/'report.json').write_text(json.dumps(report,indent=2),encoding='utf-8')


if __name__ == '__main__':
    main()
