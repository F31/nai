"""Bounded scratch training integration probe, never a production avatar checkpoint."""
import argparse
import json
import sys
from pathlib import Path


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--upstream', required=True, type=Path)
    p.add_argument('--dataset', required=True, type=Path)
    p.add_argument('--features', required=True, type=Path)
    p.add_argument('--output', required=True, type=Path)
    p.add_argument('--steps', type=int, default=20)
    p.add_argument('--checkpoint', type=Path, help='save an explicitly experimental semantic model, never upstream compatible')
    p.add_argument('--exclude-rejected', action='store_true', help='explicitly exclude failed geometry records while preserving source indices')
    p.add_argument('--normalization', choices=('batch','group'), default='batch')
    p.add_argument('--lip-control', action='store_true')
    a = p.parse_args()
    if a.output.exists() or not 1 <= a.steps <= 2000 or (a.checkpoint and a.checkpoint.exists()):
        p.error('existing output or step count invalid')
    import numpy as np
    import torch
    from avatar_semantic_dataset import SemanticDataset
    sys.path.insert(0, str(a.upstream.resolve()))
    from model import create_model
    if not torch.cuda.is_available():
        raise RuntimeError('CUDA required')
    torch.manual_seed(42)
    dataset = SemanticDataset(a.dataset, np.load(a.features).reshape(-1, 1024), exclude_rejected=a.exclude_rejected)
    if len(dataset) < 100:
        raise ValueError('at least 100 verified frames required')
    validation_start = int(len(dataset) * 0.8)
    train_end = validation_start - 20
    train_indices, validation_indices = list(range(train_end)), list(range(validation_start, len(dataset)))
    if dataset.records[validation_start]['frame'] - dataset.records[train_end-1]['frame'] <= 20:
        raise ValueError('audio guard does not separate train and validation')
    from avatar_semantic_normalization import apply_normalization
    model = apply_normalization(create_model('feathertalk', image_size=144), a.normalization).cuda().train()
    optimizer = torch.optim.Adam(model.parameters(), lr=1e-3)
    reference = torch.from_numpy(dataset[0]['target']).cuda()[None]

    def sample(index):
        row = dataset[index]
        target = torch.from_numpy(row['target']).cuda()[None]
        # Alpha is only used to define a diagnostic missing region, not perceptual quality.
        hole = torch.from_numpy((row['blend_mask'] > 0).astype(np.float32)).cuda()[None]
        image = torch.cat((reference, target * (1-hole)), dim=1)
        audio = torch.from_numpy(row['audio']).cuda()[None]
        left, right, upper, lower = np.asarray(dataset.records[index]['mouth_points'], dtype=float)
        x0, y0, x1, y1 = row['box_xyxy']
        width = np.linalg.norm(right-left)
        if not np.isfinite(width) or width <= 0:
            raise ValueError('invalid mouth landmarks')
        aperture = np.linalg.norm(lower-upper) / width
        yy, xx = np.ogrid[y0:y1:144j, x0:x1:144j]
        center = (upper+lower)/2
        mask = ((xx-center[0])/(width*.54))**2 + ((yy-center[1])/(width*.30))**2 <= 1
        lip = torch.from_numpy(mask).cuda()[None,None]
        if not mask.any() or not np.isfinite(aperture):
            raise ValueError('invalid lip supervision')
        return image, audio, target, lip, aperture

    def lip_error(prediction, target, mask):
        return (prediction-target).abs().masked_select(mask).mean()

    with a.output.open('x', encoding='utf-8', buffering=1) as out:
        def emit(**fields):
            out.write(json.dumps(fields) + '\n')
        emit(event='split', train=[0,train_end-1], guard=[train_end,validation_start-1], validation=[validation_start,len(dataset)-1], rejected_source_frames=[r['frame'] for r in dataset.rejected], scope='experimental-semantic-model')
        for step in range(a.steps):
            index = train_indices[(step*7) % (len(train_indices)-25)]
            image, audio, target, lip, aperture = sample(index)
            optimizer.zero_grad(set_to_none=True)
            prediction = model(image, audio)
            loss = (prediction-target).abs().mean()
            if a.lip_control:
                _, shifted_audio, _, _, shifted_aperture = sample(index+25)
                matched = lip_error(prediction,target,lip)
                loss = loss + 2 * matched
                if abs(aperture-shifted_aperture) >= .04:
                    shifted = lip_error(model(image,shifted_audio),target,lip)
                    loss = loss + (matched-shifted+.01).relu()
            if not torch.isfinite(loss):
                raise RuntimeError('non-finite training loss')
            loss.backward()
            optimizer.step()
            emit(event='step', step=step, loss=float(loss.detach()))
        model.eval()
        losses=[]
        silent_losses=[]
        matched_lips=[]
        shifted_lips=[]
        response=[]
        with torch.no_grad():
            for index in validation_indices:
                image, audio, target, lip, _ = sample(index)
                prediction=model(image,audio)
                value=(prediction-target).abs().mean()
                if not torch.isfinite(value):
                    raise RuntimeError('non-finite validation output')
                losses.append(float(value))
                silent_losses.append(float((model(image,torch.zeros_like(audio))-target).abs().mean()))
                if index+25 < len(dataset):
                    _, shifted_audio, _, _, _ = sample(index+25)
                    shifted=model(image,shifted_audio)
                    matched_lips.append(float(lip_error(prediction,target,lip)))
                    shifted_lips.append(float(lip_error(shifted,target,lip)))
                    response.append(float(lip_error(prediction,shifted,lip)))
        if a.checkpoint:
            torch.save({'model': model.state_dict(), 'normalization': a.normalization, 'interface': 'nai-semantic-mouth-v1-experimental', 'upstream_compatible': False, 'steps': a.steps, 'image_size': 144, 'reference_frame': dataset[0]['source_frame']}, a.checkpoint)
        emit(event='complete', validation_mae=float(np.mean(losses)), silent_audio_validation_mae=float(np.mean(silent_losses)), lip_control=a.lip_control, paired_frames=len(response), matched_lip_mae=float(np.mean(matched_lips)), shifted_lip_mae=float(np.mean(shifted_lips)), lip_response=float(np.mean(response)), quality_accepted=False, checkpoint_saved=bool(a.checkpoint))


if __name__ == '__main__':
    main()
