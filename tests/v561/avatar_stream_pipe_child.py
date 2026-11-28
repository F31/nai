"""Explicit native transport fixture, not a production/GPU worker."""
import hashlib
import os
from pathlib import Path
import queue
import struct
import sys
import threading
import time

ROOT = Path(__file__).resolve().parents[2]
sys.path[:0] = [str(ROOT/'scripts'), str(ROOT/'packages/contracts-python/src/generated')]
from v561 import avatar_stream_pb2 as wire
from avatar_raw_channel import connect, receive
from avatar_stream_state import StreamState
from avatar_audio_stream import AudioInput


def main():
    key = bytes.fromhex(os.environ.pop('NAI_AVATAR_RAW_FIXTURE_KEY'))
    cancelled, failed = threading.Event(), threading.Event()
    deadline = time.monotonic()+5
    lock, output_lock = threading.Lock(), threading.Lock()
    requests = queue.Queue(maxsize=1)
    def check():
        if cancelled.is_set() or time.monotonic() >= deadline:
            raise RuntimeError('CANCELLED')
    state = StreamState(key, bytes([8])*32, 40, check, cancelled.set)
    channel = connect('127.0.0.1', int(sys.argv[1]), key, check)
    audio = AudioInput(40, 144000, check)
    def emit(event):
        data = event.SerializeToString(deterministic=True)
        with output_lock:
            sys.stdout.buffer.write(struct.pack('<I', len(data))+data)
            sys.stdout.buffer.flush()
    def reader():
        try:
            while not cancelled.is_set():
                prefix = sys.stdin.buffer.read(4)
                if len(prefix) != 4: raise EOFError()
                size, = struct.unpack('<I', prefix)
                if not 1 <= size <= 16384: raise ValueError('CONTROL_SIZE')
                data = sys.stdin.buffer.read(size)
                if len(data) != size: raise EOFError()
                request = wire.Request.FromString(data)
                with lock:
                    event = state.handle(request, pcm=None if request.WhichOneof('action') == 'chunk' else b'')
                if request.WhichOneof('action') == 'chunk':
                    requests.put_nowait(request)
                elif request.WhichOneof('action') == 'eos':
                    requests.put_nowait(event)
                    return
                else:
                    emit(event)
                    if request.WhichOneof('action') == 'cancel':
                        return
        except BaseException:
            failed.set(); cancelled.set()
    threading.Thread(target=reader, daemon=True).start()
    try:
        while not cancelled.is_set():
            check()
            try: item = requests.get(timeout=0.01)
            except queue.Empty: continue
            if isinstance(item, wire.Event):
                audio.finish(state.samples, lambda block: check())
                emit(item)
                return 0
            pcm = receive(channel, item.chunk, check)
            with lock: state.receive(item.chunk.raw_payload_id, pcm)
            audio.push(pcm, lambda block: check())
            with lock: event = state.consumed(item.chunk.raw_payload_id)
            emit(event)
    except BaseException:
        if not cancelled.is_set(): failed.set()
    finally:
        channel.close()
    return 1 if failed.is_set() else 0


if __name__ == '__main__': raise SystemExit(main())
