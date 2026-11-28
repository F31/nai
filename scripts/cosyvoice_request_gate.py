"""Single-flight admission for an external CosyVoice service (stdlib only).

Disconnect stops delivery. The non-preemptible backend is drained under the
same admission until it actually exits; this is not GPU cancellation.
"""
import asyncio
import hashlib
import math
import queue
import threading
import time


class BusyError(RuntimeError):
    pass


class GenerationError(RuntimeError):
    pass


class RequestGate:
    def __init__(self, deadline_seconds, queue_chunks=2, max_chunk_bytes=4194304,
                 max_samples=480000):
        if not math.isfinite(deadline_seconds) or deadline_seconds <= 0 or queue_chunks < 1 or max_chunk_bytes < 2 or max_samples < 1:
            raise ValueError("positive bounds required")
        self.deadline = deadline_seconds
        self.queue_chunks = queue_chunks
        self.max_chunk_bytes = max_chunk_bytes
        self.max_samples = max_samples
        self._lock = threading.Lock()
        self._active = None

    def start(self, factory, on_complete=None):
        with self._lock:
            if self._active is not None:
                raise BusyError("TTS busy; request was not queued")
            job = Job(self, factory, on_complete)
            self._active = job
            try:
                job.thread.start()
            except BaseException:
                self._active = None
                raise
            return job

    def _finish(self, job):
        with self._lock:
            if self._active is job:
                self._active = None

    def status(self):
        with self._lock:
            job = self._active
            elapsed = time.monotonic() - job.started if job else 0
            return {"busy": job is not None, "delivery_cancelled": bool(job and job.cancelled.is_set()),
                    "elapsed_ms": round(elapsed * 1000),
                    "deadline_exceeded": bool(job and elapsed >= self.deadline),
                    "gpu_cancellation_supported": False}


class Job:
    def __init__(self, gate, factory, on_complete=None):
        self.gate = gate
        self.factory = factory
        self.on_complete = on_complete
        self.started = time.monotonic()
        self.cancelled = threading.Event()
        self.finished = threading.Event()
        self.queue = queue.Queue(maxsize=gate.queue_chunks)
        self.error = None
        self._outcome_lock = threading.Lock()
        self._completion = None
        self.thread = threading.Thread(target=self._run, daemon=True, name="cosyvoice-single-flight")

    def cancel(self):
        with self._outcome_lock:
            # ASGI background cleanup after successful EOS must not rewrite it.
            if self._completion is None:
                self.cancelled.set()

    def completion(self):
        """Immutable generation evidence; never a playback or cache permission.

        None means the model/cleanup has not exited. A copy prevents callers
        from rewriting the latched result. Transport delivery is independent.
        """
        with self._outcome_lock:
            return dict(self._completion) if self._completion is not None else None

    def _run(self):
        iterator = None
        samples = 0
        digest = hashlib.sha256()
        try:
            iterator = iter(self.factory())
            for chunk in iterator:
                if not isinstance(chunk, bytes) or len(chunk) > self.gate.max_chunk_bytes or len(chunk) % 2:
                    raise GenerationError("invalid or oversized PCM chunk")
                samples += len(chunk) // 2
                if samples > self.gate.max_samples:
                    raise GenerationError("PCM sample limit exceeded")
                digest.update(chunk)
                while not self.cancelled.is_set():
                    if time.monotonic() - self.started >= self.gate.deadline:
                        self.error = "generation deadline exceeded; backend is draining"
                        self.cancelled.set()
                        break
                    try:
                        self.queue.put(chunk, timeout=0.05)
                        break
                    except queue.Full:
                        continue
                # After disconnect/deadline continue draining the model, never
                # enqueue more data or release admission while it is running.
        except BaseException:
            self.error = "model generation failed"
        finally:
            try:
                if iterator is not None and hasattr(iterator, "close"):
                    iterator.close()
            except BaseException:
                self.error = "model cleanup failed"
            finally:
                with self._outcome_lock:
                    expired = time.monotonic() - self.started >= self.gate.deadline
                    state = ("failed" if self.error else "cancelled" if self.cancelled.is_set()
                             else "expired" if expired else "completed" if samples else "failed")
                    self._completion = {"state": state, "samples": samples,
                                        "sha256": digest.hexdigest()}
                try:
                    if self.on_complete is not None:
                        self.on_complete(self.completion())
                finally:
                    self.gate._finish(self)
                    self.finished.set()

    async def iter_bytes(self):
        try:
            while True:
                if self.cancelled.is_set():
                    raise GenerationError(self.error or "delivery cancelled; backend may still be running")
                remaining = self.gate.deadline - (time.monotonic() - self.started)
                if remaining <= 0:
                    self.error = "generation deadline exceeded; backend may still be running"
                    self.cancel()
                    raise GenerationError(self.error)
                try:
                    chunk = await asyncio.to_thread(self.queue.get, True, min(0.05, remaining))
                except queue.Empty:
                    if self.finished.is_set():
                        if self.error:
                            raise GenerationError(self.error)
                        return
                    continue
                if self.cancelled.is_set():
                    raise GenerationError("delivery cancelled")
                yield chunk
        finally:
            if not self.finished.is_set():
                self.cancel()
