"""Private signed protobuf control; raw PCM and frames remain outside control."""
import argparse
import contextlib
import hashlib
import hmac
import json
import os
import queue
import struct
import sys
import threading
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'packages/contracts-python/src/generated'))
from v561 import avatar_worker_pb2 as wire
from avatar_worker_core import Owner, InvalidInput, Cancelled, pcm_snapshot


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--config', type=Path, required=True)
    args = parser.parse_args()
    if args.config.stat().st_size > 65536:
        return 1
    config = json.loads(args.config.read_text(encoding='utf-8'))
    token = bytes.fromhex(os.environ.pop('NAI_V561_AVATAR_TOKEN'))
    if len(token) != 32:
        return 1
    owner = Owner(config['timeout_ms'] / 1000)
    from avatar_control_output import private_control_output
    output, lock, sequence = private_control_output(), threading.Lock(), 0
    portrait_hash = bytes.fromhex(config['portrait_sha256'])
    loads = 0

    def emit(kind, code, request=None, busy=False, frames=0, elapsed=0, frame=None, mapping='', stats=None, warmup=None):
        nonlocal sequence
        with lock:
            sequence += 1
            event = wire.Event(version=wire.WIRE_VERSION_CURRENT, sequence=sequence, kind=kind, code=code, busy=busy, model_loads=loads,
                portrait_sha256=portrait_hash, frames=frames, frames_acked=frames, elapsed_ms=elapsed)
            if frame is not None:
                event.frame.CopyFrom(frame)
                event.mapping_name=mapping
            if stats is not None:
                event.stats.CopyFrom(stats)
            if warmup is not None:
                event.warmup.CopyFrom(warmup)
            if request:
                event.request_sequence, event.task_id, event.epoch = request.sequence, request.task_id, request.epoch
                if request.HasField('render'):
                    event.samples, event.pcm_sha256 = request.render.samples, request.render.pcm_sha256
            event.mac = hmac.digest(token, event.SerializeToString(deterministic=True), 'sha256')
            data = event.SerializeToString(deterministic=True)
            output.write(struct.pack('<I', len(data)) + data)
            output.flush()

    def reader():
        previous = 0
        try:
            while True:
                prefix = sys.stdin.buffer.read(4)
                if len(prefix) != 4:
                    break
                size = struct.unpack('<I', prefix)[0]
                if not 0 < size <= 16384:
                    break
                payload = sys.stdin.buffer.read(size)
                if len(payload) != size:
                    break
                request = wire.Request.FromString(payload)
                signature = request.mac
                request.mac = b''
                if request.version != wire.WIRE_VERSION_CURRENT or request.sequence != previous + 1 or not hmac.compare_digest(signature, hmac.digest(token, request.SerializeToString(deterministic=True), 'sha256')):
                    break
                previous = request.sequence
                action = request.WhichOneof('action')
                if action == 'shutdown':
                    owner.stop()
                    break
                if action == 'release':
                    with owner.lock:
                        job=owner.active
                        if job is None or job.cancelled.is_set() or (job.request.task_id,job.request.epoch)!=(request.task_id,request.epoch):
                            continue
                        job.check()
                        job.releases.put_nowait(request.release)
                elif action == 'cancel':
                    accepted = owner.cancel(request.task_id, request.epoch)
                    emit(wire.KIND_CANCEL_REQUESTED if accepted else wire.KIND_REJECTED, 'CANCEL_REQUESTED' if accepted else 'STALE_TASK', request, accepted)
                elif action == 'render':
                    try:
                        job = owner.admit(request)
                    except InvalidInput as error:
                        emit(wire.KIND_REJECTED, str(error), request, owner.active is not None)
                        continue
                    emit(wire.KIND_ACCEPTED, 'ACCEPTED', request, True)
                    owner.queue.put_nowait(job)
                else:
                    break
        finally:
            owner.stop()

    failure_code = 'LOAD_FAILED'
    try:
        started = time.monotonic()
        with contextlib.redirect_stdout(sys.stderr):
            from flashhead_resident_backend import Backend
            backend = Backend(config)
            loads = 1
            failure_code = 'COMPILE_WARMUP_FAILED'
            from avatar_compiled_warmup import warmup
            warmed = warmup(backend, config, started)
        # Load native libraries before starting a thread blocked on Windows pipe I/O.
        # The Rust owner bounds startup and kills this child if warmup expires.
        threading.Thread(target=reader, daemon=True, name='avatar-control').start()
        with owner.lock:
            if not owner.stopping:
                owner.ready = True
        if owner.stopping:
            return 0
        emit(wire.KIND_READY, 'READY', elapsed=round((time.monotonic()-started)*1000),warmup=warmed)
        while not owner.stopping:
            try:
                job = owner.queue.get(timeout=0.05)
            except queue.Empty:
                continue
            started = time.monotonic()
            kind, code, frames = wire.KIND_FAILED, 'RENDER_FAILED', 0
            stats=None
            try:
                job.check()
                pcm = pcm_snapshot(job.request.render)
                with contextlib.redirect_stdout(sys.stderr):
                    frames,stats = backend.render(pcm, job, lambda request,mapping,frame: emit(wire.KIND_FRAME,'FRAME',request,True,frame=frame,mapping=mapping))
                job.check()
                kind, code = wire.KIND_COMPLETE, 'COMPLETE'
            except Cancelled:
                kind, code = wire.KIND_CANCELLED, 'CANCELLED'
            except (InvalidInput, OSError):
                code = 'PCM_IDENTITY'
            except BaseException:
                code = 'RENDER_FAILED'
            finally:
                owner.finish(job)
            emit(kind, code, job.request, False, frames if kind == wire.KIND_COMPLETE else 0, round((time.monotonic()-started)*1000),stats=stats if kind==wire.KIND_COMPLETE else None)
        emit(wire.KIND_STOPPED, 'STOPPED')
        return 0
    except BaseException:
        import traceback
        traceback.print_exc(file=sys.stderr)
        owner.stop()
        emit(wire.KIND_FAILED, failure_code)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
