"""Bounded real-GPU stream diagnostic; local shared-memory ACK, no public AV."""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import sys
import time
from types import SimpleNamespace as NS


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--config',type=Path,required=True)
    parser.add_argument('--pcm',type=Path,required=True)
    parser.add_argument('--report',type=Path,required=True)
    parser.add_argument('--first-packet-samples',type=int,default=0)
    parser.add_argument('--capture-dir',type=Path)
    args=parser.parse_args()
    if args.config.stat().st_size>65536 or args.pcm.stat().st_size>144000:
        raise ValueError('DIAGNOSTIC_LIMIT')
    config=json.loads(args.config.read_text(encoding='utf-8'))
    pcm=args.pcm.read_bytes()*2
    if not pcm or len(pcm)%2 or len(pcm)>288000:
        raise ValueError('PCM_LIMIT')
    if not args.report.is_absolute() or args.report.exists():
        raise ValueError('REPORT_PATH')
    if not 0<=args.first_packet_samples< len(pcm)//2 or args.first_packet_samples>38400:
        raise ValueError('FIRST_PACKET_LIMIT')
    if args.capture_dir:
        if not args.capture_dir.is_absolute() or args.capture_dir.parent!=args.report.parent:
            raise ValueError('CAPTURE_PATH')
        args.capture_dir.mkdir(exist_ok=False)
    temp=Path(config['temp_dir'])
    if not temp.is_absolute() or not temp.is_dir():raise ValueError('CACHE_PATH')
    for name in ['TEMP','TMP','HOME','USERPROFILE']:os.environ[name]=str(temp)
    for name,directory in [('HF_HOME','huggingface'),('TORCH_HOME','torch'),('CUDA_CACHE_PATH','cuda'),
        ('TRITON_CACHE_DIR','triton'),('MPLCONFIGDIR','matplotlib'),('TORCHINDUCTOR_CACHE_DIR','inductor'),('TORCH_EXTENSIONS_DIR','extensions')]:
        os.environ[name]=str(temp/directory)
    os.environ.update(HF_HUB_OFFLINE='1',TRANSFORMERS_OFFLINE='1',USE_TF='0',MPLBACKEND='Agg',PYTHONDONTWRITEBYTECODE='1')
    root=Path(__file__).resolve().parents[1]
    sys.path[:0]=config['dependency_dirs']+[config['upstream_dir'],str(root/'packages/contracts-python/src/generated')]
    os.chdir(config['upstream_dir'])
    from flashhead_resident_backend import Backend
    from avatar_compiled_warmup import warmup
    from avatar_worker_core import Job,Cancelled
    from v561.media_pb2 import FrameRelease
    from multiprocessing import shared_memory
    startup=time.monotonic();backend=Backend(config);warmed=warmup(backend,config,startup)
    ready_ms=round((time.monotonic()-startup)*1000)
    results=[]
    for task,mode in enumerate(['whole-reference','streamed','cancelled','recovered'],1):
        job=Job(NS(epoch=task,render=NS(samples=len(pcm)//2)),45)
        digests=[];first_frame=None;started=time.monotonic();pool=None
        capture=None;capture_result=None
        if args.capture_dir and mode=='streamed':
            from avatar_pixel_capture import Capture
            capture=Capture(args.capture_dir/'candidate.bgra',config['width'],config['height'],len(pcm)//2,task)
        def ack(request,mapping,frame):
            nonlocal first_frame,pool
            job.check();seq=len(digests)
            if frame.epoch!=task or frame.sequence!=seq or frame.source_sample!=seq*960 or frame.pts_ns!=seq*40000000:
                raise ValueError('FRAME_IDENTITY')
            identity=(mapping,frame.pool_id)
            if pool is not None and pool!=identity:raise ValueError('FRAME_POOL')
            pool=identity
            memory=shared_memory.SharedMemory(name=mapping)
            try:
                offset=frame.slot_index*frame.payload_bytes
                data=bytes(memory.buf[offset:offset+frame.payload_bytes])
            finally:memory.close()
            if hashlib.sha256(data).digest()!=frame.content_sha256:raise ValueError('FRAME_DIGEST')
            job.check()
            if capture is not None:capture.accept(mapping,frame,job)
            if first_frame is None:first_frame=round((time.monotonic()-started)*1000)
            digests.append(frame.content_sha256.hex())
            if mode=='cancelled' and len(digests)==10:
                job.cancelled.set();return
            job.releases.put_nowait(FrameRelease(pool_id=frame.pool_id,epoch=frame.epoch,sequence=frame.sequence,
                slot_index=frame.slot_index,slot_generation=frame.slot_generation))
        before_eos=0;stats=None;count=0;cancelled=False;first_packet_frames=0
        if mode=='whole-reference':
            count,stats=backend.render(pcm,job,ack)
        else:
            stream=backend.begin_stream(job,ack,144000)
            try:
                first=args.first_packet_samples*2
                if first:
                    stream.push(pcm[:first])
                    if stream.delivery is not None:stream.delivery.wait()
                    first_packet_frames=len(digests)
                    if first_packet_frames==0:raise ValueError('FIRST_PACKET_NO_GPU_FRAMES')
                for offset in range(first,len(pcm),76800):stream.push(pcm[offset:offset+76800])
                before_eos=len(digests)
                count,stats=stream.finish(len(pcm)//2)
            except Cancelled:
                if mode!='cancelled':raise
                cancelled=True
            finally:
                stream.close()
                if capture is not None:
                    try:
                        if count:capture_result=capture.finish()
                    finally:capture.__exit__(None,None,None)
        if mode=='cancelled':
            if not cancelled or len(digests)!=10 or count:raise ValueError('CANCEL_RESULT')
        elif count!=math.ceil(len(pcm)/2/960):raise ValueError('FRAME_COUNT')
        if mode in ('streamed','recovered') and before_eos==0:raise ValueError('NO_EARLY_FRAMES')
        if getattr(backend,'_stream_active',False):raise ValueError('STREAM_LEAK')
        result=dict(mode=mode,frames=count,frames_acked=len(digests),frames_before_eos=before_eos,
            first_frame_ms=first_frame,elapsed_ms=round((time.monotonic()-started)*1000),cancelled=cancelled,
            frame_digests=digests,blocks=stats.blocks if stats else None,generated_frames=stats.generated_frames if stats else None,
            retained_frames=stats.retained_frames if stats else None,tail_frames=stats.tail_frames if stats else None,
            first_packet_frames=first_packet_frames,capture=capture_result)
        results.append(result)
        print(json.dumps({k:v for k,v in result.items() if k!='frame_digests'}),flush=True)
    reference=results[0]['frame_digests']
    equal=all(result['frame_digests']==reference for result in [results[1],results[3]])
    report=dict(scope='real_gpu_incremental_session_local_ack_not_worker_rpc_or_playback',model_loads=1,model_api_calls=0,
        source_kind='explicit_existing_pcm_not_provider_eos',samples=len(pcm)//2,pcm_sha256=hashlib.sha256(pcm).hexdigest(),
        worker_ready_ms=ready_ms,warmup_frames=warmed.frames_acked,block_frames=warmed.block_frames,
        frame_num=config['frame_num'],first_packet_samples=args.first_packet_samples,timings_include_capture_io=bool(args.capture_dir),
        candidate_pixels_equal_reference=equal,results=results)
    with args.report.open('x',encoding='utf-8') as file:json.dump(report,file,indent=2)
    if not equal:raise ValueError('STREAM_PIXELS_DIFFER')


if __name__=='__main__':main()
