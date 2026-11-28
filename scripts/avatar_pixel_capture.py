"""Bounded diagnostic snapshots, not an authenticated cross-process broker."""
import hashlib
import math
from multiprocessing import shared_memory
import os


class Capture:
    def __init__(self, path, width, height, samples, epoch):
        self.count = math.ceil(samples/960)
        self.frame_bytes = width*height*4
        if (not 1 <= samples <= 144000 or not 1 <= epoch or
                any(n < 256 or n > 1024 or n % 32 for n in (width, height)) or
                self.count*self.frame_bytes > 268435456 or path.exists()):
            raise ValueError('CAPTURE_LIMIT')
        self.path, self.width, self.height, self.epoch = path, width, height, epoch
        self.partial = path.with_suffix(path.suffix+'.partial')
        self.file = self.partial.open('xb')
        self.next, self.identity = 0, None
        self.failed = False
        self.sha = hashlib.sha256()

    def __enter__(self): return self

    def accept(self, name, frame, job):
        if self.failed:
            raise ValueError('CAPTURE_INVALID')
        try:
            self._accept(name, frame, job)
        except BaseException:
            self.failed = True
            raise

    def _accept(self, name, frame, job):
        from v561.media_pb2 import PIXEL_FORMAT_BGRA8
        job.check()
        if (self.next >= self.count or frame.epoch != self.epoch or frame.sequence != self.next
                or frame.source_sample != self.next*960 or frame.source_sample_rate != 24000
                or frame.pts_ns != self.next*40000000 or frame.width != self.width or frame.height != self.height
                or frame.stride_bytes != self.width*4 or frame.payload_bytes != self.frame_bytes
                or frame.pixel_format != PIXEL_FORMAT_BGRA8 or frame.slot_index != self.next%3
                or frame.slot_generation != self.next//3+1 or len(frame.content_sha256) != 32
                or not name or not frame.pool_id):
            raise ValueError('CAPTURE_FRAME')
        identity = (name, frame.pool_id)
        if self.identity is not None and self.identity != identity:
            raise ValueError('CAPTURE_POOL')
        memory = shared_memory.SharedMemory(name=name)
        try:
            if len(memory.buf) != self.frame_bytes*3:
                raise ValueError('CAPTURE_POOL_SIZE')
            offset = frame.slot_index*self.frame_bytes
            data = bytes(memory.buf[offset:offset+self.frame_bytes])
        finally:
            memory.close()
        if hashlib.sha256(data).digest() != frame.content_sha256:
            raise ValueError('CAPTURE_DIGEST')
        job.check()
        if self.file.write(data) != len(data):
            raise OSError('CAPTURE_SHORT_WRITE')
        self.sha.update(data)
        self.identity = identity
        self.next += 1
        job.check()

    def finish(self):
        if self.failed or self.next != self.count or self.file.closed:
            raise ValueError('CAPTURE_INCOMPLETE')
        self.file.flush()
        os.fsync(self.file.fileno())
        self.file.close()
        self.partial.rename(self.path)
        return {'path': str(self.path), 'frames': self.next, 'bytes': self.frame_bytes*self.next,
                'sha256': self.sha.hexdigest(), 'pixel_format': 'bgra8'}

    def __exit__(self, *_):
        self.file.close()
