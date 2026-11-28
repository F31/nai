"""Bounded cross-process frame diagnostic. Not an authenticated production IPC."""
import hashlib
import queue
import secrets
import struct
import subprocess
import sys
import threading
from multiprocessing import shared_memory
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'packages/contracts-python/src/generated'))
from v561.media_pb2 import FrameRef, FrameRelease, PIXEL_FORMAT_BGRA8

class DiagnosticTransport:
    def __init__(self, width, height, slots, rate, fps, timeout, receiver, helper, playback_pcm=None, prefill_frames=8, audio_executor=None, stop_after_ms=0, *, epoch=None):
        if not (1 <= slots <= 4 and 1 <= width <= 1920 and 1 <= height <= 1080 and rate > 0 and fps > 0 and 0 < timeout <= 15):
            raise ValueError('invalid transport bounds')
        if playback_pcm is not None and not 1 <= prefill_frames <= 96:
            raise ValueError('invalid playback prefill')
        if audio_executor is not None and playback_pcm is None:
            raise ValueError('independent audio requires source PCM')
        if not 0 <= stop_after_ms <= 20000 or (stop_after_ms and audio_executor is None):
            raise ValueError('invalid stop diagnostic')
        self.width, self.height, self.slots, self.rate, self.fps, self.timeout = width, height, slots, rate, fps, timeout
        # The receiver can buffer frames in both its input channel and scheduler.
        # Closing stdin only ends input; playback must still drain both bounded queues.
        queue_frames = prefill_frames + slots + 16
        self.finish_timeout = min(30.0, timeout + 2 * queue_frames / fps) if playback_pcm is not None else timeout
        self.slot_bytes = width * height * 4
        if epoch is not None and (type(epoch) is not int or not 0 < epoch < 2**64):
            raise ValueError('invalid root epoch')
        self.epoch = epoch if epoch is not None else (secrets.randbits(63) or 1)
        self.pool = secrets.token_hex(16)
        self.sent = self.acked = 0
        self.generations = [0] * slots
        self.memory = shared_memory.SharedMemory(create=True, size=self.slot_bytes * slots)
        self.messages = queue.Queue(maxsize=1)
        try:
            command = [str(receiver), str(helper), self.memory.name, str(self.slot_bytes), str(slots), str(width), str(height), str(rate), str(self.epoch)]
            if playback_pcm is not None: command += [str(playback_pcm), str(prefill_frames)]
            if audio_executor is not None: command += [str(audio_executor), str(fps), str(stop_after_ms)]
            self.process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE, creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
            threading.Thread(target=self._reader, daemon=True).start()
        except BaseException:
            self.memory.close(); self.memory.unlink()
            raise

    def _reader(self):
        try:
            while True:
                header = self.process.stdout.read(4)
                if len(header) != 4: raise RuntimeError('receiver exited or returned a partial response')
                size = struct.unpack('<I', header)[0]
                if not 0 < size <= 4096: raise RuntimeError('invalid receiver response length')
                payload = self.process.stdout.read(size)
                if len(payload) != size: raise RuntimeError('partial receiver response')
                self.messages.put(payload, timeout=self.timeout)
        except BaseException as exc:
            try: self.messages.put(exc, timeout=1)
            except queue.Full: pass

    def send_rgb(self, frame):
        import numpy as np
        if frame.dtype != np.uint8 or frame.shape != (self.height, self.width, 3):
            raise ValueError('expected uint8 RGB frame')
        slot = self.sent % self.slots
        self.generations[slot] += 1
        offset = slot * self.slot_bytes
        target = np.ndarray((self.height, self.width, 4), dtype=np.uint8, buffer=self.memory.buf, offset=offset)
        target[:, :, 0], target[:, :, 1], target[:, :, 2], target[:, :, 3] = frame[:, :, 2], frame[:, :, 1], frame[:, :, 0], 255
        del target
        view = self.memory.buf[offset:offset+self.slot_bytes]
        try: digest = hashlib.sha256(view).digest()
        finally: view.release()
        msg = FrameRef(pool_id=self.pool, epoch=self.epoch, sequence=self.sent, source_sample=self.sent*self.rate//self.fps, source_sample_rate=self.rate, pts_ns=self.sent*1_000_000_000//self.fps, width=self.width, height=self.height, stride_bytes=self.width*4, pixel_format=PIXEL_FORMAT_BGRA8, slot_index=slot, slot_generation=self.generations[slot], payload_bytes=self.slot_bytes, content_sha256=digest).SerializeToString()
        self.process.stdin.write(struct.pack('<I', len(msg)) + msg); self.process.stdin.flush()
        try: response = self.messages.get(timeout=self.timeout)
        except queue.Empty: raise TimeoutError('receiver acknowledgement timed out') from None
        if isinstance(response, BaseException): raise response
        ack = FrameRelease.FromString(response)
        if (ack.pool_id, ack.epoch, ack.sequence, ack.slot_index, ack.slot_generation) != (self.pool, self.epoch, self.sent, slot, self.generations[slot]):
            raise RuntimeError('receiver acknowledgement mismatch')
        self.sent += 1; self.acked += 1

    def finish(self):
        self.process.stdin.write(struct.pack('<IQ', 0, self.acked))
        self.process.stdin.close()
        if self.process.wait(timeout=self.finish_timeout) != 0: raise RuntimeError('receiver failed')
        result = {'frames_sent':self.sent, 'frames_acked':self.acked, 'slots':self.slots, 'slot_bytes':self.slot_bytes, 'epoch':self.epoch, 'receiver':'rust_with_readonly_native_helper'}
        self.abort()
        return result

    def request_stop(self):
        self.process.stdin.write(struct.pack('<I', 0xffffffff))
        self.process.stdin.flush()

    def stop(self):
        if self.process.poll() is None:
            self.process.kill(); self.process.wait(timeout=3)
        for stream in (self.process.stdin, self.process.stdout):
            if stream and not stream.closed:
                try: stream.close()
                except OSError: pass

    def abort(self):
        self.stop()
        self.memory.close(); self.memory.unlink()
