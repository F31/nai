"""Single-flight task fences. No model imports or media playback authority."""
import hashlib
import queue
import threading
import time
from pathlib import Path


class InvalidInput(ValueError):
    pass


class Cancelled(RuntimeError):
    pass


def pcm_snapshot(render):
    path = Path(render.pcm_path)
    if not path.is_absolute() or len(str(path)) > 4096 or render.sample_rate != 24000 or not 1 <= render.samples <= 144000 or len(render.pcm_sha256) != 32:
        raise InvalidInput('PCM_IDENTITY')
    with path.open('rb') as source:
        data = source.read(288001)
    if len(data) != render.samples * 2 or hashlib.sha256(data).digest() != render.pcm_sha256:
        raise InvalidInput('PCM_IDENTITY')
    return data


class Job:
    def __init__(self, request, timeout):
        self.request = request
        self.cancelled = threading.Event()
        self.releases = queue.Queue(maxsize=1)
        self.deadline = time.monotonic() + timeout

    def check(self):
        if self.cancelled.is_set():
            raise Cancelled()
        if time.monotonic() >= self.deadline:
            self.cancelled.set()
            raise Cancelled()


class Owner:
    def __init__(self, timeout):
        if not 1 <= timeout <= 45:
            raise ValueError('timeout')
        self.timeout = timeout
        self.lock = threading.Lock()
        self.queue = queue.Queue(maxsize=1)
        self.active = None
        self.ready = False
        self.stopping = False
        self.epoch = 0
        self.task = 0

    def admit(self, request):
        with self.lock:
            if not self.ready or self.stopping:
                raise InvalidInput('NOT_READY')
            if self.active is not None:
                raise InvalidInput('BUSY')
            if request.epoch <= self.epoch or request.task_id <= self.task:
                raise InvalidInput('STALE_TASK')
            job = Job(request, self.timeout)
            self.epoch, self.task, self.active = request.epoch, request.task_id, job
            return job

    def cancel(self, task, epoch):
        with self.lock:
            if self.active is None or (self.active.request.task_id, self.active.request.epoch) != (task, epoch):
                return False
            self.active.cancelled.set()
            return True

    def stop(self):
        with self.lock:
            self.stopping = True
            self.ready = False
            if self.active:
                self.active.cancelled.set()

    def finish(self, job):
        with self.lock:
            if self.active is job:
                self.active = None

