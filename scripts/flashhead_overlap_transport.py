"""Bounded diagnostic overlap: a single owner sends frames while inference runs."""
import queue
import threading
import time

class OverlapTransport:
    def __init__(self, transport, timeout):
        self.transport = transport
        self.timeout = timeout
        self.queue = queue.Queue(maxsize=1)
        self.error = None
        self.send_seconds = 0.0
        self.thread = threading.Thread(target=self._run, daemon=True)
        self.thread.start()

    def _run(self):
        try:
            while True:
                block = self.queue.get()
                if block is None: return
                start = time.perf_counter()
                for frame in block: self.transport.send_rgb(frame)
                self.send_seconds += time.perf_counter() - start
        except BaseException as exc:
            self.error = exc

    def _put(self, block):
        deadline = time.monotonic() + self.timeout
        while True:
            if self.error: raise RuntimeError('overlap sender failed') from self.error
            if not self.thread.is_alive(): raise RuntimeError('overlap sender exited')
            remaining = deadline - time.monotonic()
            if remaining <= 0: raise TimeoutError('overlap queue timed out')
            try:
                self.queue.put(block, timeout=min(remaining, 0.1))
                return
            except queue.Full: pass

    def submit(self, block):
        self._put(block)

    def finish(self):
        self._put(None)
        self.thread.join(timeout=self.timeout)
        if self.thread.is_alive(): raise TimeoutError('overlap sender did not finish')
        if self.error: raise RuntimeError('overlap sender failed') from self.error
        result = self.transport.finish()
        result['overlap_send_seconds'] = round(self.send_seconds, 4)
        return result

    def abort(self):
        try: self.queue.put_nowait(None)
        except queue.Full: pass
        self.transport.stop()
        self.thread.join(timeout=self.timeout)
        if self.thread.is_alive():
            raise TimeoutError('overlap sender could not stop; shared memory retained')
        self.transport.abort()
