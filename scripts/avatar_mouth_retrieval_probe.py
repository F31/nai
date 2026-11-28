"""Measure same-person mouth patch retrieval on a held-out sequence."""
import argparse
import json
from pathlib import Path

import numpy as np
from PIL import Image


def geometry(row):
    left, right, top, bottom = np.asarray(row['mouth_points'], dtype=float)
    axis = right-left
    width = np.linalg.norm(axis)
    return np.array([np.linalg.norm(bottom-top)/width, width, np.arctan2(axis[1],axis[0])])


def load_patch(root, row):
    path = (root/row['semantic_patch']['target']).resolve()
    if not path.is_relative_to(root.resolve()):
        raise ValueError('patch outside dataset')
    with Image.open(path) as image:
        return np.asarray(image.convert('RGB'), dtype=np.float32)/255


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('dataset','sequence','output'):
        parser.add_argument('--'+name,type=Path,required=True)
    args = parser.parse_args()
    manifest = json.loads((args.dataset/'report.json').read_text(encoding='utf-8'))
    records = {r['frame']:r for r in manifest['samples'] if 'semantic_patch' in r}
    sequence = json.loads(args.sequence.read_text(encoding='utf-8'))['rows']
    # Same guarded training region as the audio aperture probe.
    frames = sorted(records)
    cutoff = frames[int(len(frames)*.8)]-20
    catalog = [frame for frame in frames if frame < cutoff]
    values = np.stack([geometry(records[frame]) for frame in catalog])
    args.output.mkdir()
    stats = []
    previous = previous_source = None
    for i,row in enumerate(sequence):
        frame = row['frame']
        if frame-10 <= catalog[-1]+10:
            raise ValueError('holdout lacks a training guard interval')
        source = load_patch(args.dataset,records[frame])
        _, width, roll = geometry(records[frame])
        target = max(0,row['predicted_aperture'])
        needed = row['outside_tested_range'] or row['near_closed_expansion_held']
        aperture = abs(values[:,0]-target)
        scale = abs(values[:,1]/width-1)
        angle = abs(values[:,2]-roll)
        eligible = (aperture<=.015) & (scale<=.10) & (angle<=np.deg2rad(5))
        donor = None
        replaced = source
        if needed and eligible.any():
            score = aperture/.015 + scale/.10 + angle/np.deg2rad(5)
            choice = int(np.argmin(np.where(eligible,score,np.inf)))
            donor = catalog[choice]
            replaced = load_patch(args.dataset,records[donor])
        center = np.s_[45:100,28:116]
        delta = None if previous is None else float(np.abs(replaced[center]-previous[center]).mean())
        original_delta = None if previous_source is None else float(np.abs(source[center]-previous_source[center]).mean())
        stats.append({'frame':frame,'needed':needed,'donor':donor,'candidates':int(eligible.sum()),
                      'patch_delta':delta,'source_delta':original_delta})
        previous,previous_source = replaced,source
        Image.fromarray(np.rint(np.concatenate((source,replaced),axis=1)*255).astype(np.uint8)).save(args.output/f'frame-{i:04d}.png')
    result = {'scope':'unblended_patch_and_2d_pose_proxies','frames':len(stats),
              'needed':sum(r['needed'] for r in stats),'covered':sum(r['donor'] is not None for r in stats),
              'max_patch_delta':max(r['patch_delta'] for r in stats if r['patch_delta'] is not None),
              'max_source_delta':max(r['source_delta'] for r in stats if r['source_delta'] is not None),'rows':stats}
    (args.output/'report.json').write_text(json.dumps(result,indent=2,allow_nan=False),encoding='utf-8')
    print(json.dumps({k:v for k,v in result.items() if k!='rows'}))


if __name__ == '__main__':
    main()
