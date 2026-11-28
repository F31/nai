"""Private Python-only shared-memory diagnostic; not a production IPC protocol."""
import hashlib
import multiprocessing as mp
import mmap
import queue
import secrets
import time
from collections import deque
from multiprocessing import shared_memory


def _consumer(name, slot_bytes, slots, width, height, rate, fps, epoch, incoming, outgoing):
    memory = None
    try:
        memory = shared_memory.SharedMemory(name=name)
        logical_size = slot_bytes * slots
        if not logical_size <= memory.size < logical_size + mmap.ALLOCATIONGRANULARITY or slot_bytes != width * height * 4:
            raise ValueError('shared memory size mismatch')
        outgoing.put(('ready',), timeout=3)
        expected = 0
        while True:
            item = incoming.get(timeout=15)
            if item is None:
                outgoing.put(('done', expected), timeout=3)
                return
            msg_epoch, seq, slot, generation, sample, pts_ns, expected_hash = item
            if (msg_epoch != epoch or seq != expected or not 0 <= slot < slots or
                generation <= 0 or sample != seq * rate // fps or
                pts_ns != seq * 1_000_000_000 // fps):
                raise ValueError('invalid frame metadata')
            start = slot * slot_bytes
            view = memory.buf[start:start + slot_bytes]
            try:
                digest = hashlib.sha256(view).hexdigest()
            finally:
                view.release()
            if digest != expected_hash:
                raise ValueError('shared frame hash mismatch')
            outgoing.put(('ack', seq, slot, generation, digest), timeout=3)
            expected += 1
    except BaseException as exc:
        try:
            outgoing.put(('error', type(exc).__name__, str(exc)[:300]), timeout=1)
        except Exception:
            pass
        raise
    finally:
        if memory is not None:
            memory.close()


class DiagnosticTransport:
    """Bounded slots; no slot may be reused until its acknowledgement."""
    def __init__(self, width, height, slots, rate, fps, timeout):
        if not (1 <= slots <= 4 and 1 <= width <= 1920 and 1 <= height <= 1080 and rate > 0 and fps > 0 and 0 < timeout <= 15):
            raise ValueError('invalid transport bounds')
        self.width, self.height, self.slots = width, height, slots
        self.rate, self.fps, self.timeout = rate, fps, timeout
        self.slot_bytes = width * height * 4
        self.epoch = secrets.randbits(63) or 1
        self.memory = shared_memory.SharedMemory(create=True, size=self.slot_bytes * slots)
        self.free = deque(range(slots))
        self.generations = [0] * slots
        self.inflight = {}
        self.sent = self.acked = 0
        ctx = mp.get_context('spawn')
        self.incoming = ctx.Queue(maxsize=slots)
        self.outgoing = ctx.Queue(maxsize=slots + 1)
        self.process = ctx.Process(target=_consumer, args=(self.memory.name, self.slot_bytes, slots, width, height, rate, fps, self.epoch, self.incoming, self.outgoing))
        try:
            self.process.start()
            if self._get() != ('ready',):
                raise RuntimeError('consumer did not acknowledge readiness')
        except BaseException:
            self.abort()
            raise

    def _get(self):
        deadline = time.monotonic() + self.timeout
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError('consumer acknowledgement timed out')
            try:
                message = self.outgoing.get(timeout=min(remaining, 0.1))
                if message[0] == 'error':
                    raise RuntimeError('consumer error: ' + str(message[1:]))
                return message
            except queue.Empty:
                if not self.process.is_alive():
                    raise RuntimeError('consumer exited before acknowledgement')

    def _receive_ack(self):
        msg = self._get()
        if len(msg) != 5 or msg[0] != 'ack':
            raise RuntimeError('unexpected consumer response')
        _, seq, slot, generation, digest = msg
        if self.inflight.pop(slot, None) != (seq, generation, digest):
            raise RuntimeError('acknowledgement identity mismatch')
        self.acked += 1
        self.free.append(slot)

    def send_rgb(self, frame):
        import numpy as np
        if frame.dtype != np.uint8 or frame.shape != (self.height, self.width, 3):
            raise ValueError('expected uint8 RGB frame')
        if not self.free:
            self._receive_ack()
        slot = self.free.popleft()
        self.generations[slot] += 1
        generation = self.generations[slot]
        start = slot * self.slot_bytes
        target = np.ndarray((self.height, self.width, 4), dtype=np.uint8, buffer=self.memory.buf, offset=start)
        target[:, :, 0] = frame[:, :, 2]
        target[:, :, 1] = frame[:, :, 1]
        target[:, :, 2] = frame[:, :, 0]
        target[:, :, 3] = 255
        del target
        view = self.memory.buf[start:start + self.slot_bytes]
        try:
            digest = hashlib.sha256(view).hexdigest()
        finally:
            view.release()
        seq = self.sent
        self.inflight[slot] = (seq, generation, digest)
        self.incoming.put((self.epoch, seq, slot, generation, seq * self.rate // self.fps, seq * 1_000_000_000 // self.fps, digest), timeout=self.timeout)
        self.sent += 1

    def finish(self):
        while self.inflight:
            self._receive_ack()
        self.incoming.put(None, timeout=self.timeout)
        message = self._get()
        if message != ('done', self.sent) or self.acked != self.sent:
            raise RuntimeError('consumer completion mismatch')
        self.process.join(timeout=self.timeout)
        if self.process.is_alive() or self.process.exitcode != 0:
            raise RuntimeError('consumer did not exit cleanly')
        result = {'frames_sent':self.sent, 'frames_acked':self.acked, 'slots':self.slots, 'slot_bytes':self.slot_bytes, 'epoch':self.epoch}
        self.abort()
        return result

    def abort(self):
        if self.process.is_alive():
            self.process.terminate()
            self.process.join(timeout=3)
        self.incoming.cancel_join_thread()
        self.outgoing.cancel_join_thread()
        self.incoming.close()
        self.outgoing.close()
        self.memory.close()
        self.memory.unlink()
