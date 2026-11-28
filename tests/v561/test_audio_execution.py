"""Explicit Windows silent-device tests for the independent lease executor."""
import hashlib
import hmac
import os
from pathlib import Path
import secrets
import struct
import subprocess
import sys
import threading
import time
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'packages/contracts-python/src/generated'))
from v561 import audio_execution_pb2 as p
from v561 import media_pb2 as media


class AudioExecutionTests(unittest.TestCase):
    def setUp(self):
        self.token = secrets.token_bytes(32)
        self.name = r'\\.\pipe\nai-v561-audio-' + secrets.token_hex(32)
        self.epoch = 19
        self.pipe = None
        self.pcm = b'\0\0' * 48000
        self.hash = hashlib.sha256(self.pcm).digest()
        env = dict(os.environ, NAI_V561_AUDIO_TOKEN=self.token.hex())
        self.child = subprocess.Popen([os.environ['NAI_AUDIO_OUTPUT'], '--pipe', self.name],
            env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.watchdog = threading.Timer(5, self.child.kill)
        self.watchdog.start()
        until = time.monotonic() + 3
        while True:
            try:
                self.pipe = open(self.name, 'r+b', buffering=0)
                break
            except OSError:
                if time.monotonic() > until:
                    raise
                time.sleep(.01)
        def feed():
            try:
                self.child.stdin.write(self.pcm)
                self.child.stdin.close()
            except (BrokenPipeError, OSError):
                pass
        self.writer = threading.Thread(target=feed, daemon=True)
        self.writer.start()

    def tearDown(self):
        self.watchdog.cancel()
        if self.child.poll() is None:
            self.child.kill()
        self.child.wait(timeout=2)
        self.writer.join(timeout=1)
        if self.pipe:
            self.pipe.close()
        if not self.child.stdin.closed:
            self.child.stdin.close()
        self.child.stdout.close()
        self.child.stderr.close()

    def send(self, seq, action, token=None, epoch=None):
        command = p.Command(version=1, sequence=seq, epoch=self.epoch if epoch is None else epoch)
        if isinstance(action, p.Prepare): command.prepare.CopyFrom(action)
        elif isinstance(action, p.Renew): command.renew.CopyFrom(action)
        else: command.stop.CopyFrom(action)
        command.mac = hmac.new(self.token if token is None else token, command.SerializeToString(), hashlib.sha256).digest()
        data = command.SerializeToString()
        packet = struct.pack('<I', len(data)) + data
        self.assertEqual(self.pipe.write(packet), len(packet))

    def read(self):
        def exact(n):
            data = b''
            while len(data) < n:
                chunk = self.pipe.read(n-len(data))
                if not chunk:
                    self.child.wait(timeout=2)
                    raise EOFError('child control closed: ' + self.child.stderr.read().decode(errors='replace'))
                data += chunk
            return data
        n, = struct.unpack('<I', exact(4))
        self.assertLessEqual(n, 1024)
        receipt = p.Receipt.FromString(exact(n))
        mac = receipt.mac
        receipt.mac = b''
        self.assertTrue(hmac.compare_digest(mac, hmac.new(self.token, receipt.SerializeToString(), hashlib.sha256).digest()))
        return receipt

    def prepare(self):
        self.send(1, p.Prepare(samples=48000, sha256=self.hash))
        receipt = self.read()
        self.assertEqual((receipt.state, receipt.played_samples), (p.STATE_PREPARED, 0))
        self.assertEqual(receipt.sha256, self.hash)
        self.assertTrue(receipt.HasField('clock'))
        self.assertEqual(receipt.clock.state, media.CLOCK_STATE_UNAVAILABLE)
        self.assertFalse(receipt.clock.HasField('played_sample'))
        self.assertEqual(receipt.clock.epoch,self.epoch)

    def test_prepared_without_renewal_stays_silent_and_exits(self):
        self.prepare()
        self.assertEqual(self.child.wait(timeout=1.5), 1)

    def test_bad_hash_is_rejected_before_audio(self):
        self.send(1, p.Prepare(samples=48000, sha256=b'X'*32))
        self.assertEqual(self.child.wait(timeout=1), 1)

    def test_wrong_mac_is_rejected_before_audio(self):
        self.send(1, p.Prepare(samples=48000, sha256=self.hash), token=b'X'*32)
        self.assertEqual(self.child.wait(timeout=1), 1)

    def test_silent_device_clock_reaches_source_end(self):
        self.prepare()
        last_sequence=0
        last_played=0
        generation=None
        for seq in range(2, 36):
            self.send(seq, p.Renew(ttl_ms=500))
            receipt = self.read()
            self.assertTrue(receipt.HasField('clock'))
            clock=receipt.clock
            self.assertGreater(clock.observation_sequence,last_sequence)
            self.assertGreaterEqual(clock.played_sample,last_played)
            self.assertEqual(clock.played_sample,receipt.played_samples)
            self.assertEqual(clock.sample_rate,24000)
            if generation is None: generation=clock.device_generation
            self.assertEqual(clock.device_generation,generation)
            last_sequence,last_played=clock.observation_sequence,clock.played_sample
            self.assertEqual((receipt.epoch, receipt.sequence), (self.epoch, seq))
            if receipt.state == p.STATE_COMPLETED:
                self.assertEqual(clock.state,media.CLOCK_STATE_STOPPED)
                self.assertEqual(receipt.played_samples, 48000)
                self.send(seq+1, p.Stop())
                self.assertEqual(self.child.wait(timeout=1), 0)
                return
            self.assertEqual(receipt.state, p.STATE_RUNNING)
            self.assertEqual(clock.state,media.CLOCK_STATE_RUNNING)
            time.sleep(.1)
        self.fail('device playback did not reach end')

    def test_lost_renewal_stops_active_device(self):
        self.prepare()
        self.send(2, p.Renew(ttl_ms=500))
        self.assertEqual(self.read().state, p.STATE_RUNNING)
        self.assertEqual(self.child.wait(timeout=1.5), 1)

    def test_replayed_renewal_stops_active_device(self):
        self.prepare()
        self.send(2, p.Renew(ttl_ms=500))
        self.assertEqual(self.read().state, p.STATE_RUNNING)
        self.send(2, p.Renew(ttl_ms=500))
        self.assertEqual(self.child.wait(timeout=1), 1)

    def test_wrong_epoch_stops_active_device(self):
        self.prepare()
        self.send(2, p.Renew(ttl_ms=500))
        self.assertEqual(self.read().state, p.STATE_RUNNING)
        self.send(3, p.Renew(ttl_ms=500), epoch=20)
        self.assertEqual(self.child.wait(timeout=1), 1)

    def test_closed_control_stops_active_device(self):
        self.prepare()
        self.send(2, p.Renew(ttl_ms=500))
        self.assertEqual(self.read().state, p.STATE_RUNNING)
        self.pipe.close()
        self.pipe = None
        self.assertEqual(self.child.wait(timeout=1), 1)

    def test_explicit_stop_replies_and_exits(self):
        self.prepare()
        self.send(2, p.Renew(ttl_ms=500))
        self.assertEqual(self.read().state, p.STATE_RUNNING)
        self.send(3, p.Stop())
        self.assertEqual(self.read().state, p.STATE_STOPPED)
        self.pipe.close()
        self.pipe = None
        self.assertEqual(self.child.wait(timeout=1), 0)


if __name__ == '__main__': unittest.main()
