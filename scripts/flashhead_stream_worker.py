"""Real resident incremental worker2. Independent signed control/raw/frame paths."""
import argparse
import hmac
import json
import os
from pathlib import Path
import queue
import struct
import sys
import threading
import time
import traceback

sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'packages/contracts-python/src/generated'))
from v561 import avatar_stream_pb2 as wire
from avatar_stream_state import StreamState
from avatar_worker_core import Job, Cancelled


def validate_budgets(config):
    for field,maximum in [('timeout_ms',45000),('maintenance_timeout_ms',240000)]:
        value=config[field]
        if type(value) is not int or not 1<=value<=maximum:
            raise ValueError('WORKER_BUDGET')


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--config',type=Path,required=True)
    parser.add_argument('--raw-port',type=int,required=True)
    args=parser.parse_args()
    if args.config.stat().st_size>65536:raise ValueError('CONFIG_SIZE')
    config=json.loads(args.config.read_text(encoding='utf-8'))
    validate_budgets(config)
    token=bytes.fromhex(os.environ.pop('NAI_V561_AVATAR_TOKEN'))
    if len(token)!=32:raise ValueError('TOKEN_SIZE')
    from avatar_control_output import private_control_output
    output=private_control_output()
    output_lock,state_lock=threading.Lock(),threading.Lock()
    work=queue.Queue(maxsize=1);stopping=threading.Event()
    current=None;previous=event_sequence=task_floor=epoch_floor=0
    portrait=bytes.fromhex(config['portrait_sha256'])
    started=time.monotonic()
    def emit(event):
        nonlocal event_sequence
        with output_lock:
            event_sequence+=1
            event.version=wire.WIRE_VERSION_CURRENT;event.sequence=event_sequence
            event.portrait_sha256=portrait;event.model_loads=1;event.mac=b''
            event.mac=hmac.digest(token,event.SerializeToString(deterministic=True),'sha256')
            data=event.SerializeToString(deterministic=True)
            output.write(struct.pack('<I',len(data))+data);output.flush()
    try:
        from flashhead_resident_backend import Backend
        from avatar_compiled_warmup import warmup
        backend=Backend(config)
        warmed=warmup(backend,config,started)
    except Exception as error:
        from avatar_prepare_failure import classify
        traceback.print_exc()
        emit(wire.Event(kind=wire.KIND_FAILED,code=classify(error)))
        return 1
    emit(wire.Event(kind=wire.KIND_READY,warmup=warmed,elapsed_ms=round((time.monotonic()-started)*1000)))
    from avatar_raw_channel import connect,receive
    deadline=time.monotonic()+4
    def startup_check():
        if time.monotonic()>deadline:raise TimeoutError('RAW_START_TIMEOUT')
    channel=connect('127.0.0.1',args.raw_port,token,startup_check)

    def reader():
        nonlocal current,previous,task_floor,epoch_floor
        try:
            while not stopping.is_set():
                prefix=sys.stdin.buffer.read(4)
                if len(prefix)!=4:raise EOFError('CONTROL_EOF')
                size,=struct.unpack('<I',prefix)
                if not 1<=size<=16384:raise ValueError('CONTROL_SIZE')
                data=sys.stdin.buffer.read(size)
                if len(data)!=size:raise EOFError('CONTROL_EOF')
                request=wire.Request.FromString(data)
                action=request.WhichOneof('action')
                with state_lock:
                    if action=='begin':
                        if current is not None:raise ValueError('BUSY')
                        job=Job(request,config['timeout_ms']/1000)
                        state=StreamState(token,portrait,warmed.block_frames,job.check,job.cancelled.set,epoch_floor,task_floor)
                        state.sequence=previous
                        event=state.handle(request)
                        current=dict(job=job,state=state,request=request,accepted=event,raw_pending=False,raw_tainted=False)
                        task_floor,epoch_floor=request.task_id,request.epoch
                        work.put_nowait(('begin',current))
                    else:
                        if current is None:raise ValueError('NO_TASK')
                        state,job=current['state'],current['job']
                        event=state.handle(request,pcm=None if action=='chunk' else b'')
                        if action=='release':job.releases.put_nowait(request.release)
                        elif action in ('chunk','eos'):
                            if action=='chunk':current['raw_pending']=True
                            work.put_nowait((action,request))
                        else:
                            if action=='cancel' and current['raw_pending']:current['raw_tainted']=True
                            emit(event)
                    previous=request.sequence
        except BaseException:
            stopping.set()
            with state_lock:
                if current is not None:current['job'].cancelled.set()
    threading.Thread(target=reader,daemon=True,name='stream-control').start()
    stream=None;ctx=None;task_started=None
    try:
        while not stopping.is_set():
            try:
                action,item=work.get(timeout=0.01)
            except queue.Empty:
                if ctx is None:continue
                action,item='poll',None
            try:
                if action=='begin':
                    ctx=item;job=ctx['job'];task_started=time.monotonic()
                    def frame(request,mapping,ref):
                        job.check()
                        emit(wire.Event(kind=wire.KIND_FRAME,task_id=request.task_id,epoch=request.epoch,
                            request_sequence=request.sequence,frame=ref,mapping_name=mapping))
                    stream=backend.begin_stream(job,frame,ctx['request'].begin.max_samples)
                    emit(ctx['accepted'])
                else:
                    if ctx is None:raise ValueError('NO_GPU_TASK')
                    job=ctx['job'];job.check()
                    if action=='poll':continue
                    if action=='chunk':
                        pcm=receive(channel,item.chunk,job.check)
                        with state_lock:
                            ctx['state'].receive(item.chunk.raw_payload_id,pcm)
                            ctx['raw_pending']=False
                        stream.push(pcm)
                        with state_lock:event=ctx['state'].consumed(item.chunk.raw_payload_id)
                        emit(event)
                    elif action=='eos':
                        # Input receipt is distinct from tail inference and completion.
                        state=ctx['state']
                        emit(state._reply(wire.KIND_EOS_VERIFIED,item.sequence))
                        frames,stats=stream.finish(item.eos.samples)
                        with state_lock:
                            emit(wire.Event(kind=wire.KIND_RENDER_COMPLETE,task_id=job.request.task_id,epoch=job.request.epoch,
                                request_sequence=job.request.sequence,samples=state.samples,sha256=state.hash.digest(),
                                frames=frames,stats=stats,elapsed_ms=round((time.monotonic()-task_started)*1000)))
                            current=None
                        ctx=stream=None
            except BaseException as error:
                if ctx is None:raise
                job=ctx['job'];job.cancelled.set()
                if stream is not None:stream.close()
                if ctx['raw_tainted']:channel.close();stopping.set()
                with state_lock:
                    emit(wire.Event(kind=wire.KIND_RENDER_CANCELLED if isinstance(error,Cancelled) else wire.KIND_FAILED,
                        task_id=job.request.task_id,epoch=job.request.epoch,request_sequence=job.request.sequence,
                        code=('CANCELLED' if channel.fileno()>=0 else 'RAW_RESTART_REQUIRED') if isinstance(error,Cancelled) else 'STREAM_FAILED'))
                    current=None
                ctx=stream=None
                # Partial raw frames cannot be reused; restart requires a fresh channel.
                if not isinstance(error,Cancelled):traceback.print_exc();stopping.set()
                if channel.fileno()<0:stopping.set()
        return 0
    finally:
        if ctx is not None:ctx['job'].cancelled.set()
        if stream is not None:stream.close()
        channel.close()
        emit(wire.Event(kind=wire.KIND_STOPPED))


if __name__=='__main__':raise SystemExit(main())
