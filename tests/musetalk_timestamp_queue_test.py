import sys
import unittest
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from musetalk_timestamp_queue import TimestampFrames, playback_tick, source_index

class TimestampFramesTest(unittest.TestCase):
    def test_expiration_and_future_hold(self):
        q = TimestampFrames(1, 3)
        self.assertEqual(q.offer(1, 0, "old"), "accepted")
        self.assertEqual(q.offer(1, 2, "now"), "accepted")
        self.assertEqual(q.offer(1, 4, "future"), "accepted")
        self.assertEqual(q.take(1, 2), ("frame", "now", 1))
        self.assertEqual(q.take(1, 3), ("missing", None, 0))
        self.assertEqual(q.take(1, 4), ("frame", "future", 0))
        self.assertEqual(q.offer(1, 3, "old"), "out_of_order")
    def test_capacity_late_epoch_revoke(self):
        q = TimestampFrames(7, 1)
        self.assertEqual(q.offer(7, 3, "a"), "accepted")
        self.assertEqual(q.offer(7, 4, "b"), "full")
        self.assertEqual(q.take(8, 3)[0], "revoked")
        self.assertEqual(q.take(7, 4), ("missing", None, 1))
        self.assertEqual(q.offer(7, 4, "b"), "late")
        self.assertEqual(q.offer(7, 5, "c"), "accepted")
        self.assertEqual(q.revoke(), 1)
        self.assertEqual(q.take(7, 5)[0], "revoked")
        self.assertEqual(q.offer(7, 6, "x"), "revoked")
    def test_source_time_mapping(self):
        self.assertEqual([source_index(i,30,25,120) for i in (0,1,4,5,24,25,99,100)], [0,1,4,6,28,30,118,0])
        with self.assertRaises(ValueError): source_index(-1,30,25,120)
        with self.assertRaises(ValueError): source_index(0,30,0,120)
    def test_playback_tick_uses_played_samples(self):
        self.assertEqual([playback_tick(i * 960, 24000, 25) for i in range(5)], [0, 1, 2, 3, 4])
        self.assertEqual(playback_tick(959, 24000, 25), 0)
        self.assertEqual(playback_tick(96000, 24000, 25), 100)
        self.assertEqual(source_index(playback_tick(24000, 24000, 25), 30, 25, 120), 30)
        with self.assertRaises(ValueError): playback_tick(-1, 24000, 25)
        with self.assertRaises(ValueError): playback_tick(True, 24000, 25)
        with self.assertRaises(ValueError): playback_tick(0, 24000, 0)
if __name__ == "__main__": unittest.main()
