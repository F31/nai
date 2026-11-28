"""Windows integration tests; set NAI_RUST_RECEIVER and NAI_MAPPING_READER."""
import os
import struct
import sys
import time
import subprocess
import unittest
from unittest import mock
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
import numpy as np
from flashhead_rust_transport import DiagnosticTransport

class FinishDeadlineTests(unittest.TestCase):
    def test_hung_receiver_remains_a_failure(self):
        transport = DiagnosticTransport.__new__(DiagnosticTransport)
        transport.process = mock.Mock()
        transport.acked = 1
        transport.finish_timeout = 7.0
        transport.process.wait.side_effect = subprocess.TimeoutExpired('receiver', 7.0)
        with self.assertRaises(subprocess.TimeoutExpired):
            transport.finish()
        transport.process.stdin.close.assert_called_once()
        transport.process.wait.assert_called_once_with(timeout=7.0)

@unittest.skipUnless(sys.platform == 'win32' and os.getenv('NAI_RUST_RECEIVER') and os.getenv('NAI_MAPPING_READER'), 'Windows binaries not configured')
class RustTransportTests(unittest.TestCase):
    def open(self):
        return DiagnosticTransport(16, 16, 3, 16000, 25, 3, os.environ['NAI_RUST_RECEIVER'], os.environ['NAI_MAPPING_READER'])

    def test_real_shared_slots_and_generation_reuse(self):
        transport = self.open()
        try:
            for index in range(9): transport.send_rgb(np.full((16, 16, 3), index, dtype=np.uint8))
            result = transport.finish()
            self.assertEqual((result['frames_sent'], result['frames_acked']), (9, 9))
        except BaseException:
            transport.abort()
            raise

    def test_receiver_exit_is_not_acknowledged(self):
        transport = self.open()
        try:
            transport.process.kill()
            transport.process.wait(timeout=3)
            with self.assertRaises((BrokenPipeError, OSError, RuntimeError)):
                transport.send_rgb(np.zeros((16, 16, 3), dtype=np.uint8))
            self.assertEqual(transport.acked, 0)
        finally:
            transport.abort()

    def test_completion_count_must_match_acked_frames(self):
        transport = self.open()
        try:
            transport.send_rgb(np.zeros((16, 16, 3), dtype=np.uint8))
            transport.process.stdin.write(struct.pack('<IQ', 0, 2))
            transport.process.stdin.close()
            self.assertNotEqual(transport.process.wait(timeout=4), 0)
        finally:
            transport.abort()

    @unittest.skipUnless(os.getenv('NAI_AV_RECEIVER') and os.getenv('NAI_SILENT_LONG_PCM'), 'long AV diagnostic not configured')
    def test_buffered_audio_drains_before_receiver_exit(self):
        transport = DiagnosticTransport(16, 16, 3, 16000, 25, 5, os.environ['NAI_AV_RECEIVER'], os.environ['NAI_MAPPING_READER'], os.environ['NAI_SILENT_LONG_PCM'], 96)
        try:
            for index in range(168):
                transport.send_rgb(np.full((16, 16, 3), index % 256, dtype=np.uint8))
            result = transport.finish()
            self.assertEqual((result['frames_sent'], result['frames_acked']), (168, 168))
        except BaseException:
            transport.abort()
            raise

    @unittest.skipUnless(os.getenv('NAI_AV_RECEIVER') and os.getenv('NAI_SILENT_PCM'), 'AV diagnostic not configured')
    def test_stale_epoch_emits_failure_summary_without_playback(self):
        original_popen = subprocess.Popen
        def launch(*args, **kwargs):
            return original_popen(*args, **kwargs, stderr=subprocess.PIPE)
        with mock.patch('flashhead_rust_transport.subprocess.Popen', side_effect=launch):
            transport = DiagnosticTransport(16, 16, 3, 16000, 25, 5, os.environ['NAI_AV_RECEIVER'], os.environ['NAI_MAPPING_READER'], os.environ['NAI_SILENT_PCM'], 8)
        try:
            transport.send_rgb(np.zeros((16, 16, 3), dtype=np.uint8))
            transport.epoch += 1
            with self.assertRaises((BrokenPipeError, OSError, RuntimeError)):
                transport.send_rgb(np.zeros((16, 16, 3), dtype=np.uint8))
            self.assertNotEqual(transport.process.wait(timeout=4), 0)
            stderr = transport.process.stderr.read().decode('utf-8', errors='replace')
            transport.process.stderr.close()
            self.assertIn('AV_EXECUTION outcome=failed', stderr)
            self.assertIn('first_window_submit_ms=None', stderr)
            self.assertEqual(transport.acked, 1)
        finally:
            transport.abort()

    @unittest.skipUnless(os.getenv('NAI_AV_RECEIVER') and os.getenv('NAI_SILENT_PCM'), 'AV diagnostic not configured')
    def test_audio_end_with_producer_still_open_fails_without_timeout(self):
        original_popen = subprocess.Popen
        def launch(*args, **kwargs):
            return original_popen(*args, **kwargs, stderr=subprocess.PIPE)
        with mock.patch('flashhead_rust_transport.subprocess.Popen', side_effect=launch):
            transport = DiagnosticTransport(16, 16, 3, 16000, 25, 5, os.environ['NAI_AV_RECEIVER'], os.environ['NAI_MAPPING_READER'], os.environ['NAI_SILENT_PCM'], 8)
        try:
            for index in range(10): transport.send_rgb(np.full((16, 16, 3), index, dtype=np.uint8))
            start = time.monotonic()
            code = transport.process.wait(timeout=4)
            self.assertNotEqual(code, 0)
            self.assertLess(time.monotonic() - start, 4)
            stderr = transport.process.stderr.read().decode('utf-8', errors='replace')
            transport.process.stderr.close()
            self.assertIn('AV_TIMING_INCOMPLETE', stderr)
            self.assertIn('AV_EXECUTION outcome=failed', stderr)
            self.assertIn('AV_TIMING first_window_submit_ms=', stderr)
            self.assertIn('audio completed before all frames were received', stderr)
            self.assertIn('clock_rate=24000', stderr)
        finally:
            transport.abort()

    @unittest.skipUnless(os.getenv('NAI_AV_RECEIVER') and os.getenv('NAI_SILENT_LONG_PCM'), 'long AV diagnostic not configured')
    def test_playing_receiver_fails_closed_when_frame_producer_stops(self):
        original_popen = subprocess.Popen

        def launch(*args, **kwargs):
            return original_popen(*args, **kwargs, stderr=subprocess.PIPE)

        for action in ('pause', 'disconnect', 'stop'):
            with self.subTest(action=action):
                with mock.patch('flashhead_rust_transport.subprocess.Popen', side_effect=launch):
                    transport = DiagnosticTransport(16, 16, 3, 16000, 25, 5, os.environ['NAI_AV_RECEIVER'], os.environ['NAI_MAPPING_READER'], os.environ['NAI_SILENT_LONG_PCM'], 8)
                try:
                    for index in range(10):
                        transport.send_rgb(np.full((16, 16, 3), index + 1, dtype=np.uint8))
                    if action != 'pause':
                        time.sleep(0.65)
                    if action == 'disconnect':
                        transport.process.stdin.close()
                    elif action == 'stop':
                        transport.request_stop()
                    start = time.monotonic()
                    status = transport.process.wait(timeout=4)
                    self.assertEqual(status == 0, action == 'stop')
                    elapsed = time.monotonic() - start
                    stderr = transport.process.stderr.read().decode('utf-8', errors='replace')
                    self.assertRegex(stderr, r'first_window_submit_ms=Some\(\d+\)')
                    if action == 'disconnect':
                        self.assertIn('frame input failed', stderr)
                    elif action == 'stop':
                        self.assertIn('AV_EXECUTION outcome=stopped', stderr)
                    else:
                        self.assertRegex(stderr, r'selected frame lease expired|audio (?:device failed|stopped before submitted sentence completed)')
                    print(f'\nframe input action={action}, receiver exit after {elapsed:.3f}s', flush=True)
                finally:
                    transport.abort()
                    transport.process.stderr.close()

if __name__ == '__main__': unittest.main()
