"""One CPU delivery block; never invokes a GPU/model or queues another block."""
from concurrent.futures import ThreadPoolExecutor
import time


def intersection_ns(left, right):
    return sum(max(0, min(b, d) - max(a, c)) for a, b in left for c, d in right)


class Delivery:
    def __init__(self, job, transport):
        self.job, self.transport = job, transport
        self.executor = ThreadPoolExecutor(max_workers=1, thread_name_prefix='avatar-delivery')
        self.future = None
        self.intervals = []

    def wait(self):
        if self.future is not None:
            future, self.future = self.future, None
            future.result()
        self.job.check()

    def submit(self, images):
        if self.future is not None:
            raise RuntimeError('DELIVERY_OCCUPIED')
        self.job.check()
        self.future = self.executor.submit(self._send, images)

    def _send(self, images):
        started = time.monotonic_ns()
        try:
            for image in images:
                self.job.check()
                self.transport.send_rgb(image)
        except BaseException:
            self.job.cancelled.set()
            raise
        finally:
            self.intervals.append((started, time.monotonic_ns()))

    def close(self):
        # OwnerFrames waits in bounded job.check loops. Never close its mapping
        # underneath the sender, even when the producer's GPU operation failed.
        if self.future is not None and not self.future.done():
            self.job.cancelled.set()
        self.executor.shutdown(wait=True, cancel_futures=True)
