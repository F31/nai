"""Verify captured bytes, measure errors, mux existing PCM for local review."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
from avatar_pixel_metrics import compare


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--profile',type=Path,required=True)
    parser.add_argument('--pcm',type=Path,required=True)
    parser.add_argument('--ffmpeg',type=Path,required=True)
    parser.add_argument('--output-dir',type=Path,required=True)
    parser.add_argument('--width',type=int,required=True)
    parser.add_argument('--height',type=int,required=True)
    parser.add_argument('--baseline-mode',default='baseline-warm')
    parser.add_argument('--candidate-mode',default='compiled-warm')
    args=parser.parse_args()
    with args.profile.open('rb') as source: raw=source.read(131073)
    if len(raw)>131072: raise ValueError('PROFILE_LIMIT')
    report=json.loads(raw)
    with args.pcm.open('rb') as source: pcm=source.read(288001)
    if (len(pcm)!=report['samples']*2 or hashlib.sha256(pcm).hexdigest()!=report['pcm_sha256']
            or not args.ffmpeg.is_absolute() or not args.ffmpeg.is_file()
            or not args.output_dir.is_absolute()):
        raise ValueError('REVIEW_INPUT')
    captures=[next(r['capture'] for r in report['results'] if r['mode']==mode)
        for mode in (args.baseline_mode,args.candidate_mode)]
    frames=captures[0]['frames']
    if any(c['frames']!=frames or c['bytes']!=frames*args.width*args.height*4 for c in captures):
        raise ValueError('CAPTURE_GEOMETRY')
    paths=[Path(c['path']) for c in captures]
    metrics=compare(*paths,args.width,args.height,frames,[c['sha256'] for c in captures])
    args.output_dir.mkdir(exist_ok=False)
    for label,path in zip(('baseline','compiled'),paths):
        with (args.output_dir/(label+'-encode.log')).open('xb') as log:
            subprocess.run([str(args.ffmpeg),'-nostdin','-hide_banner','-n',
                '-f','rawvideo','-pixel_format','bgra','-video_size',f'{args.width}x{args.height}',
                '-framerate','25','-i',str(path),'-f','s16le','-ar','24000','-ac','1','-i',str(args.pcm),
                '-c:v','libx264','-preset','fast','-crf','16','-pix_fmt','yuv420p',
                '-c:a','aac','-b:a','128k','-shortest','-movflags','+faststart',
                str(args.output_dir/(label+'-speech-review.mp4'))],stdout=log,stderr=log,check=True,timeout=60)
    with (args.output_dir/'metrics.json').open('x',encoding='utf-8') as output:
        json.dump({'profile':str(args.profile),'pcm_sha256':report['pcm_sha256'],
                   'review_encoding':'h264-crf16-yuv420p-aac-not-lossless',
                   'raw_pixel_metrics':metrics,'human_quality':'unreviewed'},output,indent=2)
    print(json.dumps({k:v for k,v in metrics.items() if k!='per_frame'}))


if __name__=='__main__': main()
