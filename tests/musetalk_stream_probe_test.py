import sys
import threading
import time
import unittest
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import musetalk_stream_probe as probe

class ScriptedClock:
    def __init__(self, played, submitted, allowed, stopped=False):
        self.played, self.submitted, self.allowed, self.stopped = played, submitted, allowed, stopped
        self.version, self._seen = 1, 0
        self.cv = threading.Condition()

    def go(self):
        return None

    def latest(self):
        with self.cv:
            return self.played, self.submitted, self.allowed, self.stopped

    def wait_update(self, timeout):
        with self.cv:
            if self.version == self._seen:
                self.cv.wait(timeout)
            self._seen = self.version
            return self.played, self.submitted, self.allowed, self.stopped

    def publish(self, played, submitted, allowed, stopped=False):
        with self.cv:
            self.played, self.submitted, self.allowed, self.stopped = played, submitted, allowed, stopped
            self.version += 1
            self.cv.notify_all()

    def close(self):
        if not self.stopped:
            self.publish(self.played, self.submitted, False, True)
        return {"reason": "stop", "played": self.played, "submitted": self.submitted,
                "progress_after_stop": False, "stderr_tail": ""}

class StreamProbeTest(unittest.TestCase):
    def test_capacity_and_deadline(self):
        result = probe.run(lambda i: [i, i+1], 0.2, fps=25, capacity=2)
        self.assertEqual(result["consumed"] + result["underflow"], 5)
        self.assertLessEqual(result["queue_peak"], 2)
        self.assertLess(result["elapsed_seconds"], 1)
    def test_underflow_and_generator_error(self):
        def slow(i):
            time.sleep(0.12)
            return [i]
        self.assertGreater(probe.run(slow, 0.2)["underflow"], 0)
        def fail(i): raise RuntimeError("failure")
        with self.assertRaises(RuntimeError): probe.run(fail, 0.2)
    def test_timestamped_underflow_and_shutdown(self):
        def slow(i):
            time.sleep(0.12)
            return [i]
        result = probe.run_timestamped(slow, 0.25, fps=20, capacity=2)
        self.assertEqual(result["presented"] + result["missing"], 5)
        self.assertGreater(result["missing"], 0)
        self.assertLessEqual(result["queue_peak"], 2)
        self.assertLess(result["elapsed_seconds"], 1)
        self.assertEqual(result["prefill_frames"], 0)
    def test_prefill_stays_outside_the_timed_window(self):
        def paced(i):
            time.sleep(0.02)
            return [i]
        result = probe.run_timestamped(paced, 0.4, fps=10, capacity=4, prefill=2)
        self.assertEqual(result["prefill_frames"], 2)
        self.assertGreaterEqual(result["prefill_seconds"], 0.02)
        self.assertEqual(result["presented"] + result["missing"], 4)
        self.assertEqual(result["missing"], 0)
    def test_prefill_bounds(self):
        with self.assertRaises(ValueError):
            probe.run_timestamped(lambda i: [i], 0.2, capacity=8, prefill=9)

    def test_bounds(self):
        with self.assertRaises(ValueError): probe.run(lambda i: [i], 601)
        with self.assertRaises(ValueError): probe.run(lambda i: [i], 1, capacity=0)

    def test_prefill_follows_played_samples_and_disallow_stops(self):
        clock = ScriptedClock(960, 50 * 960, True)
        got = []
        def on_present(tick, frame):
            got.append((tick, frame))
            if tick == 1:
                clock.publish(96000, 96000, False, True)
        result = probe.run_external_clock(lambda i: [f"f{i}"], lambda: clock, fps=25, capacity=8,
                                           prefill=2, sample_rate=24000, clip_samples=8 * 960,
                                           on_present=on_present, deadline_s=2)
        self.assertEqual(got, [(0, "f0"), (1, "f1")])
        self.assertEqual(result["presented"], 2)
        self.assertEqual(result["missing"], 0)
        self.assertEqual(result["presented_after_disallow"], 0)
        self.assertEqual(result["selection_violations"], 0)

    def test_disallow_before_audio_presents_nothing(self):
        clock = ScriptedClock(96000, 96000, False, True)
        result = probe.run_external_clock(lambda i: [f"f{i}"], lambda: clock, prefill=2,
                                           sample_rate=24000, clip_samples=96000, deadline_s=2)
        self.assertEqual(result["presented"], 0)
        self.assertEqual(result["missing"], 0)
        self.assertFalse(result["allowed_seen"])
        self.assertGreaterEqual(result["remaining_revoked"], 2)

    def test_late_generation_counts_missing(self):
        clock = ScriptedClock(3 * 960, 0, True)
        def on_present(tick, frame):
            self.assertIsNone(frame)
            if tick == 2:
                clock.publish(3 * 960, 0, False, True)
        result = probe.run_external_clock(lambda i: [f"f{i}"], lambda: clock, fps=25, capacity=8,
                                           prefill=0, sample_rate=24000, clip_samples=3 * 960,
                                           on_present=on_present, deadline_s=2)
        self.assertEqual(result["presented"], 0)
        self.assertEqual(result["missing"], 3)
        self.assertEqual(result["produced"], 0)

    def test_each_audio_second_counts_presented_and_missing(self):
        clock = ScriptedClock(20, 0, True)
        sampled = []
        def on_present(tick, frame):
            if tick == 19:
                clock.publish(20, 0, False, True)
        def before_go():
            sampled.append("before-go")
        result = probe.run_external_clock(lambda i: [f"f{i}"], lambda: clock, fps=10, capacity=8,
                                           prefill=4, sample_rate=10, clip_samples=20,
                                           on_present=on_present, before_go=before_go, deadline_s=2)
        self.assertEqual(sampled, ["before-go"])
        self.assertEqual(result["presented_by_second"], [4, 0])
        self.assertEqual(result["missing_by_second"], [6, 10])
        self.assertEqual(result["ratio_by_second"], [0.4, 0.0])

    def test_external_clock_pauses_once_when_played_reaches_the_mark(self):
        clock = ScriptedClock(24000, 48000, True)
        calls = []

        def pause(hold_s):
            calls.append(hold_s)
            clock.publish(96000, 96000, False, True)
            return {"played_at_pause": 24000, "submitted_at_pause": 48000,
                    "played_moved": False, "submitted_moved": False, "hold_seconds": hold_s}

        clock.pause = pause
        result = probe.run_external_clock(lambda i: [f"f{i}"], lambda: clock, fps=25, capacity=8,
                                           prefill=2, sample_rate=24000, clip_samples=96000,
                                           pause_at_samples=24000, pause_hold_s=1, deadline_s=2)
        self.assertEqual(calls, [1])
        self.assertEqual(result["pause"]["requested_samples"], 24000)
        self.assertFalse(result["pause"]["played_moved"])

    def test_external_clock_bounds(self):
        with self.assertRaises(ValueError):
            probe.run_external_clock(lambda i: [i], lambda: None, prefill=9)
        with self.assertRaises(ValueError):
            probe.run_external_clock(lambda i: [i], lambda: None, pause_at_samples=1)
if __name__ == "__main__": unittest.main()
