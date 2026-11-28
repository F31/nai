import sys
import time
import unittest
from pathlib import Path

import numpy as np
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
from flashhead_shm_diagnostic import DiagnosticTransport


class SharedMemoryDiagnosticTests(unittest.TestCase):
    def test_real_child_confirms_frames_across_slot_reuse(self):
        transport = DiagnosticTransport(4, 3, 2, 16000, 25, 3)
        try:
            for value in range(7):
                transport.send_rgb(np.full((3, 4, 3), value, dtype=np.uint8))
            result = transport.finish()
            transport = None
            self.assertEqual(result['frames_sent'], 7)
            self.assertEqual(result['frames_acked'], 7)
            self.assertEqual(result['slot_bytes'], 48)
        finally:
            if transport is not None:
                transport.abort()

    def test_dead_consumer_is_detected_without_waiting_indefinitely(self):
        transport = DiagnosticTransport(4, 3, 1, 16000, 25, 1)
        try:
            transport.process.terminate()
            transport.process.join(timeout=2)
            self.assertFalse(transport.process.is_alive())
            frame = np.zeros((3, 4, 3), dtype=np.uint8)
            transport.send_rgb(frame)
            start = time.monotonic()
            with self.assertRaises((RuntimeError, TimeoutError)):
                transport.send_rgb(frame)
            self.assertLess(time.monotonic() - start, 2)
        finally:
            transport.abort()

    def test_live_consumer_without_ack_times_out(self):
        transport = DiagnosticTransport(4, 3, 1, 16000, 25, 0.5)
        try:
            start = time.monotonic()
            with self.assertRaises(TimeoutError):
                transport._get()
            self.assertLess(time.monotonic() - start, 2)
        finally:
            transport.abort()

    def test_rgb_is_written_as_bgra(self):
        transport = DiagnosticTransport(1, 1, 1, 16000, 25, 2)
        try:
            transport.send_rgb(np.array([[[1, 2, 3]]], dtype=np.uint8))
            self.assertEqual(bytes(transport.memory.buf[:4]), bytes([3, 2, 1, 255]))
            transport.finish()
            transport = None
        finally:
            if transport is not None:
                transport.abort()

    def test_rejects_invalid_frame_without_dispatch(self):
        transport = DiagnosticTransport(4, 3, 1, 16000, 25, 2)
        try:
            with self.assertRaises(ValueError):
                transport.send_rgb(np.zeros((4, 3, 3), dtype=np.uint8))
            self.assertEqual(transport.sent, 0)
            transport.finish()
            transport = None
        finally:
            if transport is not None:
                transport.abort()


if __name__ == '__main__':
    unittest.main()
