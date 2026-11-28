"""Render experimental checkpoint on held-out real source audio, not novel speech."""
import argparse
import json
import sys
from pathlib import Path


def main():
    p=argparse.ArgumentParser(description=__doc__)
    for name in ('upstream','dataset','features','checkpoint','output-dir'):
        p.add_argument('--'+name, type=Path, required=True)
    p.add_argument('--calibrate-bn', type=int, choices=(0,64), default=0)
    p.add_argument('--shift-control', action='store_true', help='Use real held-out audio shifted by at least 25 source frames')
    a=p.parse_args()
    if a.output_dir.exists():
        p.error('output already exists')
    import numpy as np
    import torch
    from PIL import Image,ImageDraw
    from avatar_semantic_dataset import SemanticDataset
    sys.path.insert(0,str(a.upstream.resolve()))
    from model import create_model
    state=torch.load(a.checkpoint,map_location='cpu',weights_only=True)
    if state['interface']!='nai-semantic-mouth-v1-experimental':
        raise ValueError('unexpected checkpoint interface')
    from avatar_semantic_normalization import apply_normalization
    model=apply_normalization(create_model('feathertalk',image_size=state['image_size']),state.get('normalization','batch')).cuda().eval()
    model.load_state_dict(state['model'])
    dataset=SemanticDataset(a.dataset,np.load(a.features).reshape(-1,1024),exclude_rejected=True)
    reference=torch.from_numpy(dataset[0]['target']).cuda()[None]
    # Recompute only BatchNorm statistics on training samples; never validation frames.
    bn_layers=[m for m in model.modules() if isinstance(m,torch.nn.modules.batchnorm._BatchNorm)] if a.calibrate_bn else []
    for m in bn_layers:
        m.reset_running_stats()
        m.momentum=None
    model.train()
    with torch.no_grad():
        train_end=int(len(dataset)*0.8)-20
        for idx in np.linspace(0,train_end-1,a.calibrate_bn,dtype=int):
            item=dataset[int(idx)]
            target=torch.from_numpy(item['target']).cuda()[None]
            hole=torch.from_numpy((item['blend_mask']>0).astype(np.float32)).cuda()[None]
            audio=torch.from_numpy(item['audio']).cuda()[None]
            model(torch.cat((reference,target*(1-hole)),dim=1),audio)
    model.eval()
    a.output_dir.mkdir(parents=True)
    records=[]
    heldout=int(len(dataset)*0.8)
    indices=[heldout+offset for offset in (25,75,125,175)] if a.shift_control else [0,250,heldout,len(dataset)-100]
    sheet=Image.new('RGB',(144*4,168*len(indices)))
    with torch.no_grad():
        for row,index in enumerate(indices):
            item=dataset[index]
            target=torch.from_numpy(item['target']).cuda()[None]
            hole=torch.from_numpy((item['blend_mask']>0).astype(np.float32)).cuda()[None]
            masked=target*(1-hole)
            audio=torch.from_numpy(item['audio']).cuda()[None]
            output=model(torch.cat((reference,masked),dim=1),audio)
            silent=model(torch.cat((reference,masked),dim=1),torch.zeros_like(audio))
            buffers=[(b,b.clone()) for b in model.buffers()]
            model.train()
            train_output=model(torch.cat((reference,masked),dim=1),audio)
            model.eval()
            for buffer,saved in buffers:
                buffer.copy_(saved)
            if not torch.isfinite(output).all():
                raise ValueError('non-finite prediction')
            records.append({'index':index,'source_frame':item['source_frame'],'min':float(output.min()),'max':float(output.max()),'mae':float((output-target).abs().mean()),'train_mode_mae':float((train_output-target).abs().mean()),'audio_vs_silence_mae':float((output-silent).abs().mean())})
            if a.shift_control:
                shifted=dataset[index+25]
                offset=int(shifted['source_frame'])-int(item['source_frame'])
                if offset < 25:
                    raise ValueError('insufficient source-frame separation')
                shifted_audio=torch.from_numpy(shifted['audio']).cuda()[None]
                shifted_output=model(torch.cat((reference,masked),dim=1),shifted_audio)
                region=hole.expand_as(output).bool()
                records[-1].update(source_frame_offset=offset,
                    feature_delta=float((audio-shifted_audio).abs().mean()),
                    output_delta=float((output-shifted_output).abs().mean()),
                    masked_output_delta=float((output-shifted_output).abs()[region].mean()),
                    matched_masked_mae=float((output-target).abs()[region].mean()),
                    shifted_masked_mae=float((shifted_output-target).abs()[region].mean()))
                train_output=shifted_output
            for col,(name,value) in enumerate((('target',target),('masked',masked),('eval',output),('shifted' if a.shift_control else 'train-mode',train_output))):
                pixels=(value[0].clamp(0,1).permute(1,2,0).cpu().numpy()*255).round().astype(np.uint8)
                sheet.paste(Image.fromarray(pixels),(col*144,row*168+24))
                ImageDraw.Draw(sheet).text((col*144+2,row*168+3),f'{index} {name}',fill='white')
    sheet.save(a.output_dir/'review.png')
    (a.output_dir/'report.json').write_text(json.dumps(records,indent=2),encoding='utf-8')


if __name__=='__main__':
    main()
