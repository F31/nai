"""Explicit native GPU worker diagnostic. Python parent is not the Rust engine."""
import argparse
import hashlib
import hmac
import json
import os
from pathlib import Path
import queue
import secrets
import socket
import struct
import subprocess
import sys
import threading
import time

ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'packages/contracts-python/src/generated'))
from v561 import avatar_stream_pb2 as wire
from v561.media_pb2 import FrameRelease


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for arg in ['config','pcm','reference','report']:parser.add_argument('--'+arg,type=Path,required=True)
    args=parser.parse_args()
    config=json.loads(args.config.read_text(encoding='utf-8'))
    pcm=args.pcm.read_bytes()*2
    if not pcm or len(pcm)%2 or len(pcm)>288000:raise ValueError('PCM_LIMIT')
    reference=json.loads(args.reference.read_text())['results'][0]['frame_digests']
    if not args.report.is_absolute() or args.report.exists():raise ValueError('REPORT_PATH')
    temp=Path(config['temp_dir']);env=dict(os.environ)
    for name in ['TEMP','TMP','HOME','USERPROFILE']:env[name]=str(temp)
    for name,folder in [('HF_HOME','huggingface'),('TORCH_HOME','torch'),('CUDA_CACHE_PATH','cuda'),
        ('TRITON_CACHE_DIR','triton'),('MPLCONFIGDIR','matplotlib'),('TORCHINDUCTOR_CACHE_DIR','inductor'),('TORCH_EXTENSIONS_DIR','extensions')]:env[name]=str(temp/folder)
    env.update(HF_HUB_OFFLINE='1',TRANSFORMERS_OFFLINE='1',USE_TF='0',MPLBACKEND='Agg',PYTHONDONTWRITEBYTECODE='1',
        PYTHONPATH=os.pathsep.join(config['dependency_dirs']+[config['upstream_dir']]))
    key=secrets.token_bytes(32);env['NAI_V561_AVATAR_TOKEN']=key.hex()
    listener=socket.socket();listener.bind(('127.0.0.1',0));listener.listen(1);listener.settimeout(4)
    log=args.report.with_suffix('.worker.log').open('xb')
    child=subprocess.Popen([config['python'],'-B',str(ROOT/'scripts/flashhead_stream_worker.py'),'--config',str(args.config),
        '--raw-port',str(listener.getsockname()[1])],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=log,env=env,cwd=config['upstream_dir'])
    events=queue.Queue(maxsize=8)
    def reader():
        try:
            while True:
                prefix=child.stdout.read(4)
                if len(prefix)!=4:raise EOFError('CONTROL_EOF')
                size,=struct.unpack('<I',prefix)
                if not 1<=size<=16384:raise ValueError('EVENT_SIZE')
                data=child.stdout.read(size)
                if len(data)!=size:raise EOFError('CONTROL_EOF')
                events.put(wire.Event.FromString(data))
        except BaseException as error:events.put(error)
    thread=threading.Thread(target=reader,daemon=True);thread.start()
    sequence=event_sequence=0
    def next_event(timeout=45):
        nonlocal event_sequence
        e=events.get(timeout=timeout)
        if isinstance(e,BaseException):raise e
        mac=e.mac;e.mac=b''
        if (e.version!=wire.WIRE_VERSION_CURRENT or e.sequence!=event_sequence+1 or e.model_loads!=1
            or e.portrait_sha256!=bytes.fromhex(config['portrait_sha256'])
            or not hmac.compare_digest(mac,hmac.digest(key,e.SerializeToString(deterministic=True),'sha256'))):raise ValueError('EVENT_AUTH')
        event_sequence=e.sequence
        return e
    def send(task,action):
        nonlocal sequence
        sequence+=1
        r=wire.Request(version=wire.WIRE_VERSION_CURRENT,sequence=sequence,task_id=task,epoch=task,**action)
        r.mac=hmac.digest(key,r.SerializeToString(deterministic=True),'sha256')
        data=r.SerializeToString(deterministic=True)
        child.stdin.write(struct.pack('<I',len(data))+data);child.stdin.flush()
        return sequence
    channel=None;results=[]
    try:
        ready=next_event(config['maintenance_timeout_ms']/1000)
        if ready.kind!=wire.KIND_READY or ready.warmup.frames_acked<80 or ready.warmup.block_frames!=config['frame_num']-ready.warmup.motion_frames:
            raise ValueError('READY_GEOMETRY')
        channel,peer=listener.accept();listener.close();channel.settimeout(4)
        nonce=secrets.token_bytes(32);channel.sendall(nonce)
        def exact(count):
            data=b''
            while len(data)<count:
                part=channel.recv(count-len(data))
                if not part:raise EOFError('RAW_EOF')
                data+=part
            return data
        proof=exact(32)
        if not hmac.compare_digest(proof,hmac.digest(key,b'nai.avatar.raw.v1\0'+nonce,'sha256')):raise ValueError('RAW_AUTH')
        channel.sendall(hmac.digest(key,b'nai.avatar.raw.server.v1\0'+nonce+proof,'sha256'))
        from multiprocessing import shared_memory
        for task,mode in enumerate(['complete','cancelled','recovered'],1):
            began=time.monotonic();digests=[];cancel_sent=False;released=0;first=None;eos_seen=False;before_eos=None;pool=None
            def until(kind):
                nonlocal cancel_sent,released,first,eos_seen,before_eos,pool
                while True:
                    e=next_event()
                    if e.task_id!=task or e.epoch!=task:raise ValueError('TASK_IDENTITY')
                    if e.kind==wire.KIND_FRAME:
                        f=e.frame;seq=len(digests);size=config['width']*config['height']*4
                        if (f.sequence!=seq or f.epoch!=task or f.source_sample!=seq*960 or f.source_sample_rate!=24000
                            or f.pts_ns!=seq*40000000 or f.width!=config['width'] or f.height!=config['height'] or f.stride_bytes!=config['width']*4
                            or f.payload_bytes!=size or f.slot_index!=seq%3 or f.slot_generation!=seq//3+1 or f.pixel_format!=1):raise ValueError('FRAME_METADATA')
                        identity=(e.mapping_name,f.pool_id)
                        if pool is not None and pool!=identity:raise ValueError('FRAME_POOL')
                        pool=identity
                        memory=shared_memory.SharedMemory(name=e.mapping_name)
                        try:
                            if len(memory.buf)!=size*3:raise ValueError('POOL_SIZE')
                            offset=f.slot_index*size;pixels=bytes(memory.buf[offset:offset+size])
                        finally:memory.close()
                        if hashlib.sha256(pixels).digest()!=f.content_sha256:raise ValueError('PIXEL_DIGEST')
                        if first is None:first=round((time.monotonic()-began)*1000)
                        digests.append(f.content_sha256.hex())
                        if mode=='cancelled' and len(digests)==10:
                            send(task,dict(cancel=wire.Cancel()));cancel_sent=True
                        elif not cancel_sent:
                            send(task,dict(release=FrameRelease(pool_id=f.pool_id,epoch=f.epoch,sequence=f.sequence,slot_index=f.slot_index,slot_generation=f.slot_generation)))
                            released+=1
                    elif e.kind==wire.KIND_FAILED:raise ValueError('WORKER_FAILED')
                    elif e.kind==wire.KIND_EOS_VERIFIED:
                        eos_seen=True;before_eos=len(digests)
                    if e.kind in (kind,wire.KIND_RENDER_CANCELLED):return e
            send(task,dict(begin=wire.Begin(portrait_sha256=bytes.fromhex(config['portrait_sha256']),block_frames=ready.warmup.block_frames,sample_rate=24000,max_samples=144000)))
            until(wire.KIND_INPUT_ACCEPTED)
            terminal=None
            for index,offset in enumerate(range(0,len(pcm),76800)):
                part=pcm[offset:offset+76800]
                chunk=wire.Chunk(raw_payload_id=index+1,start_sample=offset//2,samples=len(part)//2,sha256=hashlib.sha256(part).digest())
                send(task,dict(chunk=chunk))
                header=chunk.SerializeToString(deterministic=True)
                channel.sendall(struct.pack('<I',len(header))+header+part)
                terminal=until(wire.KIND_CHUNK_CONSUMED)
                if terminal.kind==wire.KIND_RENDER_CANCELLED:break
                if terminal.raw_payload_id!=index+1:raise ValueError('RAW_ACK')
            if not cancel_sent:
                send(task,dict(eos=wire.Eos(samples=len(pcm)//2,sha256=hashlib.sha256(pcm).digest(),provider_eos_verified=True)))
                terminal=until(wire.KIND_RENDER_COMPLETE)
                checks=dict(samples=terminal.samples,expected_samples=len(pcm)//2,frames=terminal.frames,
                    expected_frames=len(reference),retained_frames=terminal.stats.retained_frames,eos_seen=eos_seen,
                    matching_pixel_hashes=sum(a==b for a,b in zip(digests,reference)),frames_read=len(digests),
                    source_hash_matches=terminal.sha256==hashlib.sha256(pcm).digest())
                print(json.dumps(dict(completion_checks=checks)),flush=True)
                if (terminal.samples!=len(pcm)//2 or terminal.sha256!=hashlib.sha256(pcm).digest() or terminal.frames!=len(reference)
                    or terminal.stats.retained_frames!=len(reference) or not eos_seen or digests!=reference):
                    args.report.with_suffix('.failure.json').write_text(json.dumps(checks,indent=2),encoding='utf-8')
                    raise ValueError('COMPLETE_IDENTITY')
            else:
                if terminal.kind!=wire.KIND_RENDER_CANCELLED or len(digests)!=10:raise ValueError('CANCEL_IDENTITY')
            result=dict(mode=mode,frames_read=len(digests),frame_releases=released,eos_verified=eos_seen,frames_at_eos=before_eos,
                first_frame_ms=first,elapsed_ms=round((time.monotonic()-began)*1000),cancelled=cancel_sent,
                pixels_equal_reference=None if cancel_sent else digests==reference)
            results.append(result);print(json.dumps(result),flush=True)
        # A cancelled partial raw packet taints framing: stop, never reuse it as
        # a new task's prefix. This is distinct from recoverable GPU cancellation.
        task=4;mode='partial_raw_cancelled';digests=[];released=0;first=None;cancel_sent=False;pool=None
        send(task,dict(begin=wire.Begin(portrait_sha256=bytes.fromhex(config['portrait_sha256']),
            block_frames=ready.warmup.block_frames,sample_rate=24000,max_samples=144000)))
        until(wire.KIND_INPUT_ACCEPTED)
        chunk=wire.Chunk(raw_payload_id=1,start_sample=0,samples=2,sha256=hashlib.sha256(bytes([1,0,2,0])).digest())
        send(task,dict(chunk=chunk));header=chunk.SerializeToString(deterministic=True)
        channel.sendall(struct.pack('<I',len(header))+header+bytes([1]))
        began=time.monotonic();send(task,dict(cancel=wire.Cancel()));cancel_sent=True
        terminal=until(wire.KIND_RENDER_CANCELLED)
        if digests or terminal.kind!=wire.KIND_RENDER_CANCELLED:raise ValueError('PARTIAL_CANCEL')
        results.append(dict(mode=mode,frames_read=0,render_cancelled_ms=round((time.monotonic()-began)*1000),channel_must_restart=True))
        child.stdin.close()
        stopped=next_event()
        if stopped.kind!=wire.KIND_STOPPED:raise ValueError('NOT_STOPPED')
        if child.wait(timeout=5)!=0:raise ValueError('WORKER_EXIT')
        report=dict(scope='native_python_parent_real_gpu_worker_not_rust_device_playback',protocol=wire.WIRE_VERSION_CURRENT,model_loads=1,
            model_api_calls=0,samples=len(pcm)//2,ready_ms=ready.elapsed_ms,results=results)
        with args.report.open('x',encoding='utf-8') as out:json.dump(report,out,indent=2)
    finally:
        if channel is not None:channel.close()
        listener.close()
        if child.poll() is None:child.kill()
        child.wait(timeout=10);log.close()


if __name__=='__main__':main()
