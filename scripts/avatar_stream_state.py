"""Signed incremental-input consumer. Not wired to worker6 or an OS raw pipe.

The caller supplies authenticated raw-channel payload identity separately; this
module proves content/state, not OS peer identity, ACL or provider correctness.
"""
import hashlib
import hmac
from v561 import avatar_stream_pb2 as wire


class StreamState:
    def __init__(self, key, portrait, block_frames, check, invalidate, epoch_floor=0, task_floor=0):
        if len(key) != 32 or len(portrait) != 32 or not 1 <= block_frames <= 97:
            raise ValueError('STREAM_CONFIG')
        self.key, self.portrait, self.block_frames = key, portrait, block_frames
        self.check, self.invalidate = check, invalidate
        self.epoch_floor, self.task_floor = epoch_floor, task_floor
        self.sequence = self.event_sequence = self.task = self.epoch = 0
        self.samples = self.last_payload = 0
        self.hash = hashlib.sha256()
        self.limit = 0
        self.pending = None
        self.ended = self.failed = False

    def _abort(self):
        self.ended = True
        self.pending = None
        if not self.failed:
            self.failed = True
            self.invalidate()

    def _reply(self, kind, request_sequence, payload=0):
        self.event_sequence += 1
        event = wire.Event(version=wire.WIRE_VERSION_CURRENT, sequence=self.event_sequence,
            request_sequence=request_sequence, task_id=self.task, epoch=self.epoch,
            kind=kind, raw_payload_id=payload, samples=self.samples, sha256=self.hash.digest())
        event.mac = hmac.digest(self.key, event.SerializeToString(deterministic=True), 'sha256')
        return event

    def handle(self, request, raw_payload_id=0, pcm=b''):
        try:
            self.check()
            # EOS closes input, not the executing task. Cancel must remain live.
            action = request.WhichOneof('action')
            if self.ended and (self.failed or action not in ('cancel', 'release')):
                raise ValueError('STREAM_ENDED')
            unsigned = wire.Request()
            unsigned.CopyFrom(request)
            unsigned.mac = b''
            if request.ByteSize() > 16384 or request.version != wire.WIRE_VERSION_CURRENT or request.sequence != self.sequence+1:
                raise ValueError('STREAM_SEQUENCE')
            if not hmac.compare_digest(request.mac, hmac.digest(self.key, unsigned.SerializeToString(deterministic=True), 'sha256')):
                raise ValueError('STREAM_AUTH')
            if action == 'begin':
                b = request.begin
                if self.task or self.pending or request.task_id <= self.task_floor or request.epoch <= self.epoch_floor:
                    raise ValueError('STREAM_IDENTITY')
                if b.portrait_sha256 != self.portrait or b.block_frames != self.block_frames or b.sample_rate != 24000 or not 1 <= b.max_samples <= 144000:
                    raise ValueError('STREAM_BEGIN')
                if raw_payload_id or pcm:
                    raise ValueError('STREAM_RAW')
                self.task, self.epoch, self.limit = request.task_id, request.epoch, b.max_samples
            else:
                if not self.task or (request.task_id, request.epoch) != (self.task, self.epoch):
                    raise ValueError('STREAM_IDENTITY')
                if action == 'chunk':
                    c = request.chunk
                    if self.pending is not None:
                        raise ValueError('STREAM_IN_FLIGHT')
                    if not 1 <= c.samples <= 38400:
                        raise ValueError('STREAM_RAW')
                    if not c.raw_payload_id or c.raw_payload_id <= self.last_payload or c.start_sample != self.samples:
                        raise ValueError('STREAM_OFFSET')
                    if self.samples+c.samples > self.limit or len(c.sha256) != 32:
                        raise ValueError('STREAM_CONTENT')
                    self.pending = (request.sequence, c.raw_payload_id, c.samples, c.sha256, False)
                    # None means a separately authenticated raw reader will
                    # supply bytes later. Advance control before waiting on IO.
                    if pcm is not None:
                        self.receive(raw_payload_id, pcm)
                    elif raw_payload_id:
                        raise ValueError('STREAM_RAW')
                elif action == 'release':
                    if raw_payload_id or pcm:
                        raise ValueError('STREAM_RAW')
                elif action in ('eos', 'cancel'):
                    if raw_payload_id or pcm:
                        raise ValueError('STREAM_RAW')
                    if action == 'eos':
                        e = request.eos
                        if self.pending or not self.samples or not e.provider_eos_verified or e.samples != self.samples or e.sha256 != self.hash.digest():
                            raise ValueError('STREAM_EOS')
                        self.ended = True
                    else:
                        self._abort()
                else:
                    raise ValueError('STREAM_ACTION')
            self.sequence = request.sequence
            if action != 'cancel':
                self.check()
            if action in ('chunk', 'release'):
                return None  # only local compute consumption may release the slot
            kind = {'begin': wire.KIND_INPUT_ACCEPTED, 'eos': wire.KIND_EOS_VERIFIED, 'cancel': wire.KIND_CANCELLED}[action]
            return self._reply(kind, request.sequence)
        except BaseException:
            self._abort()
            raise

    def consumed(self, raw_payload_id):
        try:
            self.check()
            if self.ended or self.pending is None or self.pending[1] != raw_payload_id or not self.pending[4]:
                raise ValueError('STREAM_RELEASE')
            sequence, payload = self.pending[:2]
            self.pending = None
            return self._reply(wire.KIND_CHUNK_CONSUMED, sequence, payload)
        except BaseException:
            self._abort()
            raise

    def receive(self, raw_payload_id, pcm):
        try:
            self.check()
            if self.ended or self.pending is None or self.pending[4]:
                raise ValueError('STREAM_RAW')
            sequence, payload, samples, digest, _ = self.pending
            if raw_payload_id != payload or not isinstance(pcm, bytes) or len(pcm) != samples*2:
                raise ValueError('STREAM_RAW')
            if hashlib.sha256(pcm).digest() != digest:
                raise ValueError('STREAM_CONTENT')
            self.hash.update(pcm)
            self.samples += samples
            self.last_payload = payload
            self.pending = (sequence, payload, samples, digest, True)
            self.check()
        except BaseException:
            self._abort()
            raise
