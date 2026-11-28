import hashlib
import hmac
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path[:0] = [str(ROOT/'scripts'), str(ROOT/'packages/contracts-python/src/generated')]
from v561 import avatar_stream_pb2 as wire
from avatar_stream_state import StreamState

KEY = bytes([7])*32
PORTRAIT = bytes([8])*32
PCM = b'\x01\x00\x02\x00'


def sign(request):
    request.mac = b''
    request.mac = hmac.digest(KEY, request.SerializeToString(deterministic=True), 'sha256')
    return request


def request(sequence, **action):
    return sign(wire.Request(version=wire.WIRE_VERSION_CURRENT, sequence=sequence, task_id=5, epoch=6, **action))


def begin():
    return request(1, begin=wire.Begin(portrait_sha256=PORTRAIT, block_frames=40, sample_rate=24000, max_samples=144000))


def chunk(sequence=2, **changes):
    fields = dict(raw_payload_id=1, start_sample=0, samples=2, sha256=hashlib.sha256(PCM).digest())
    fields.update(changes)
    return request(sequence, chunk=wire.Chunk(**fields))


class StreamTests(unittest.TestCase):
    def state(self, check=lambda: None):
        invalidated = []
        return StreamState(KEY, PORTRAIT, 40, check, lambda: invalidated.append(True)), invalidated

    def test_valid_consumption_then_eos_and_signed_ack(self):
        state, invalidated = self.state()
        accepted = state.handle(begin())
        self.assertEqual(accepted.kind, wire.KIND_INPUT_ACCEPTED)
        self.assertIsNone(state.handle(chunk(), 1, PCM))
        event = state.consumed(1)
        self.assertEqual((event.request_sequence, event.raw_payload_id, event.samples), (2, 1, 2))
        mac = event.mac; event.mac = b''
        self.assertEqual(mac, hmac.digest(KEY, event.SerializeToString(deterministic=True), 'sha256'))
        final = state.handle(request(3, eos=wire.Eos(samples=2, sha256=hashlib.sha256(PCM).digest(), provider_eos_verified=True)))
        self.assertEqual(final.kind, wire.KIND_EOS_VERIFIED)
        self.assertEqual(invalidated, [])
        self.assertEqual(state.handle(request(4, cancel=wire.Cancel())).kind, wire.KIND_CANCELLED)

    def test_cancel_does_not_wait_for_consumer(self):
        cancelled = [False]
        def check():
            if cancelled[0]: raise RuntimeError('CANCELLED')
        state = StreamState(KEY, PORTRAIT, 40, check, lambda: cancelled.__setitem__(0, True))
        state.handle(begin()); state.handle(chunk(), 1, PCM)
        self.assertEqual(state.handle(request(3, cancel=wire.Cancel())).kind, wire.KIND_CANCELLED)
        self.assertTrue(cancelled[0]); self.assertIsNone(state.pending)
        with self.assertRaises(RuntimeError): state.consumed(1)

    def test_signature_sequence_epoch_and_portrait_fail_closed(self):
        for mode in ['signature', 'sequence', 'epoch', 'portrait', 'geometry', 'format', 'floor', 'old_version']:
            state, invalidated = self.state()
            r = begin()
            if mode == 'signature': r.mac = bytes(32)
            if mode == 'sequence': r.sequence = 2; sign(r)
            if mode == 'epoch': r.epoch = 0; sign(r)
            if mode == 'portrait': r.begin.portrait_sha256 = bytes(32); sign(r)
            if mode == 'geometry': r.begin.block_frames = 41; sign(r)
            if mode == 'format': r.begin.sample_rate = 16000; sign(r)
            if mode == 'floor': state.task_floor = 5
            if mode == 'old_version': r.version = 1; sign(r)
            with self.assertRaises(ValueError, msg=mode): state.handle(r)
            self.assertEqual(invalidated, [True])
            with self.assertRaises(ValueError): state.handle(begin())

    def test_chunk_offsets_bytes_hash_and_single_slot(self):
        for mode in ['offset', 'length', 'hash', 'payload', 'slot', 'task', 'limit', 'release']:
            state, invalidated = self.state(); state.handle(begin())
            r = chunk(); data = PCM; raw_id = 1
            if mode == 'offset': r.chunk.start_sample = 1
            if mode == 'length': data = PCM[:-1]
            if mode == 'hash': r.chunk.sha256 = bytes(32)
            if mode == 'payload': raw_id = 2
            if mode == 'task': r.task_id = 7
            if mode == 'limit': state.limit = 1
            sign(r)
            with self.assertRaises(ValueError, msg=mode):
                state.handle(r, raw_id, data)
                if mode == 'slot': state.handle(chunk(3, raw_payload_id=2, start_sample=2), 2, PCM)
                if mode == 'release': state.consumed(2)
            self.assertEqual(invalidated, [True])

    def test_eos_requires_empty_slot_receipt_total_and_digest(self):
        for mode in ['slot', 'receipt', 'total', 'hash']:
            state, invalidated = self.state(); state.handle(begin()); state.handle(chunk(), 1, PCM)
            if mode != 'slot': state.consumed(1)
            e = wire.Eos(samples=2, sha256=hashlib.sha256(PCM).digest(), provider_eos_verified=True)
            if mode == 'receipt': e.provider_eos_verified = False
            if mode == 'total': e.samples = 3
            if mode == 'hash': e.sha256 = bytes(32)
            with self.assertRaises(ValueError): state.handle(request(3, eos=e))
            self.assertEqual(invalidated, [True])

    def test_parent_cancellation_prevents_raw_consumption(self):
        cancelled = [False]
        def check():
            if cancelled[0]: raise RuntimeError('PARENT_CANCELLED')
        state, invalidated = self.state(check)
        state.handle(begin())
        cancelled[0] = True
        with self.assertRaisesRegex(RuntimeError, 'PARENT_CANCELLED'):
            state.handle(chunk(), 1, PCM)
        self.assertEqual(state.samples, 0)
        self.assertEqual(invalidated, [True])

    def test_staged_chunk_cancel_and_receive_before_local_ack(self):
        state, invalidated = self.state()
        state.handle(begin())
        state.handle(chunk(), pcm=None)
        self.assertEqual(state.samples, 0)
        event = state.handle(request(3, cancel=wire.Cancel()))
        self.assertEqual(event.kind, wire.KIND_CANCELLED)
        with self.assertRaises(ValueError): state.receive(1, PCM)
        self.assertEqual(invalidated, [True])
        state, _ = self.state(); state.handle(begin()); state.handle(chunk(), pcm=None)
        with self.assertRaises(ValueError): state.consumed(1)

    def test_frame_release_does_not_free_raw_slot_and_is_allowed_after_eos(self):
        from v561.media_pb2 import FrameRelease
        state,_=self.state();state.handle(begin());state.handle(chunk(),1,PCM)
        self.assertIsNone(state.handle(request(3,release=FrameRelease(pool_id='fixture'))))
        self.assertIsNotNone(state.pending)
        state.consumed(1)
        state.handle(request(4,eos=wire.Eos(samples=2,sha256=hashlib.sha256(PCM).digest(),provider_eos_verified=True)))
        self.assertIsNone(state.handle(request(5,release=FrameRelease(pool_id='fixture'))))

    def test_signed_chunks_feed_one_continuous_resampler(self):
        import numpy as np
        import librosa
        from avatar_audio_stream import AudioInput
        pcm = np.random.default_rng(81).integers(-32000, 32000, 117120, dtype=np.int16)
        raw = pcm.astype('<i2').tobytes()
        state, invalidated = self.state()
        state.handle(begin())
        audio = AudioInput(40, 144000, lambda: None)
        blocks = []
        sequence = 2
        for index, start in enumerate(range(0, len(raw), 76800)):
            part = raw[start:start+76800]
            r = request(sequence, chunk=wire.Chunk(raw_payload_id=index+1,
                start_sample=start//2, samples=len(part)//2, sha256=hashlib.sha256(part).digest()))
            self.assertIsNone(state.handle(r, index+1, part))
            audio.push(part, blocks.append)
            state.consumed(index+1)
            sequence += 1
        self.assertGreater(len(blocks), 0)
        state.handle(request(sequence, eos=wire.Eos(samples=len(raw)//2,
            sha256=hashlib.sha256(raw).digest(), provider_eos_verified=True)))
        audio.finish(state.samples, blocks.append)
        actual = np.concatenate([b.samples[:b.valid] for b in blocks])
        expected = librosa.resample(pcm.astype(np.float32)/32768.0, orig_sr=24000, target_sr=16000)
        np.testing.assert_allclose(actual, expected, rtol=0, atol=2e-7)
        self.assertEqual(invalidated, [])


if __name__ == '__main__': unittest.main()
