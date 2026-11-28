"""Apply held-out audio aperture predictions to a continuous source patch segment."""
import argparse
import json
from pathlib import Path

import numpy as np
from PIL import Image

from avatar_aperture_render_probe import render


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('dataset', 'predictions', 'output'):
        parser.add_argument('--'+name, type=Path, required=True)
    args = parser.parse_args()
    manifest = json.loads((args.dataset/'report.json').read_text(encoding='utf-8'))
    records = {row['frame']:row for row in manifest['samples'] if 'semantic_patch' in row}
    predictions = json.loads(args.predictions.read_text(encoding='utf-8'))
    if manifest['fps'] != 25:
        raise ValueError('expected 25 fps')
    for i in range(len(predictions)-99):
        segment = predictions[i:i+100]
        if all(row['frame']==segment[0]['frame']+j for j,row in enumerate(segment)):
            break
    else:
        raise ValueError('no continuous 100-frame holdout')
    args.output.mkdir()
    rows = []
    for i, row in enumerate(segment):
        record = records[row['frame']]
        patch = record['semantic_patch']
        x0,y0,x1,y1 = patch['box_xyxy']
        path = (args.dataset/patch['target']).resolve()
        if not path.is_relative_to(args.dataset.resolve()):
            raise ValueError('target outside dataset')
        with Image.open(path) as image:
            source = np.array(image.convert('RGB'))
        left,right,upper,lower = (np.asarray(record['mouth_points'])-[x0,y0])*[144/(x1-x0),144/(y1-y0)]
        width = np.linalg.norm(right-left)
        current = row['source_aperture']
        target = max(0,row['predicted_aperture'])
        ratio = target/max(current,1e-6)
        near_closed_expansion = current < .03 and target > current
        factor = 1. if near_closed_expansion else float(np.clip(ratio,.65,1.45))
        out,_ = render(source,(upper+lower)/2,(width*.63,width*.39),factor)
        Image.fromarray(np.concatenate((source,out),axis=1)).save(args.output/f'frame-{i:04d}.png')
        rows.append({**row,'raw_ratio':ratio,'factor':factor,'outside_tested_range':not .65<=ratio<=1.45,
                     'near_closed_expansion_held':near_closed_expansion})
    report = {'fps':manifest['fps'],'frames':len(rows),'outside_tested_range':sum(r['outside_tested_range'] for r in rows),
              'near_closed_expansion_held':sum(r['near_closed_expansion_held'] for r in rows),'rows':rows}
    (args.output/'report.json').write_text(json.dumps(report,indent=2,allow_nan=False),encoding='utf-8')


if __name__ == '__main__':
    main()
