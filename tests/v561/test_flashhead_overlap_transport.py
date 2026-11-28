import sys
import threading
import unittest
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
from flashhead_overlap_transport import OverlapTransport

class Sink:
    def __init__(self, fail=False): self.values, self.fail, self.aborted = [], fail, False
    def send_rgb(self, frame):
        if self.fail: raise ValueError('sender failed')
        self.values.append(frame)
    def finish(self): return {'count': len(self.values)}
    def stop(self): pass
    def abort(self): self.aborted = True

class OverlapTests(unittest.TestCase):
    def test_order_and_finish(self):
        sink = Sink(); sender = OverlapTransport(sink, 2)
        sender.submit([1, 2]); sender.submit([3, 4])
        self.assertEqual(sender.finish()['count'], 4)
        self.assertEqual(sink.values, [1, 2, 3, 4])

    def test_failure_propagates(self):
        sink = Sink(True); sender = OverlapTransport(sink, 2)
        sender.submit([1]); sender.thread.join(timeout=2)
        with self.assertRaises(RuntimeError): sender.finish()
        sender.abort(); self.assertTrue(sink.aborted)

    def test_abort_stops_inflight_before_releasing_transport(self):
        class BlockingSink(Sink):
            def __init__(self):
                super().__init__(); self.entered = threading.Event(); self.stopped = threading.Event()
            def send_rgb(self, frame):
                self.entered.set(); self.stopped.wait(2)
            def stop(self): self.stopped.set()
            def abort(self):
                if not self.stopped.is_set(): raise AssertionError('released before stopping')
                super().abort()
        sink = BlockingSink(); sender = OverlapTransport(sink, 2)
        sender.submit([1]); self.assertTrue(sink.entered.wait(2))
        sender.abort()
        self.assertFalse(sender.thread.is_alive()); self.assertTrue(sink.aborted)

if __name__ == '__main__': unittest.main()
