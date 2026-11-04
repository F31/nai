"""Research-only bounded timestamp selector; ticks are simulated, not device audio."""
from collections import deque
import threading

class TimestampFrames:
    def __init__(self, epoch, capacity=8):
        if type(epoch) is not int or epoch < 1 or not 1 <= capacity <= 16:
            raise ValueError("invalid epoch or capacity")
        self._epoch, self._capacity = epoch, capacity
        self._frames, self._lock = deque(), threading.Lock()
        self._last_offer, self._last_tick = -1, -1
        self._active = True

    def offer(self, epoch, tick, frame):
        with self._lock:
            if not self._active or epoch != self._epoch: return "revoked"
            if type(tick) is not int or tick < 0 or tick <= self._last_offer: return "out_of_order"
            if tick <= self._last_tick:
                self._last_offer = tick
                return "late"
            if len(self._frames) >= self._capacity: return "full"
            self._frames.append((tick, frame))
            self._last_offer = tick
            return "accepted"

    def take(self, epoch, tick):
        with self._lock:
            if not self._active or epoch != self._epoch: return ("revoked", None, 0)
            if type(tick) is not int or tick <= self._last_tick: return ("out_of_order", None, 0)
            self._last_tick = tick
            dropped = 0
            while self._frames and self._frames[0][0] < tick:
                self._frames.popleft(); dropped += 1
            if self._frames and self._frames[0][0] == tick:
                return ("frame", self._frames.popleft()[1], dropped)
            return ("missing", None, dropped)

    def revoke(self):
        with self._lock:
            self._active = False
            count = len(self._frames)
            self._frames.clear()
            return count

    def size(self):
        with self._lock: return len(self._frames)

def source_index(output_tick, source_fps, output_fps, frame_count):
    if (type(output_tick) is not int or output_tick < 0 or
        any(type(x) is not int or x <= 0 for x in (source_fps, output_fps, frame_count))):
        raise ValueError("invalid frame clock")
    return ((output_tick * source_fps) // output_fps) % frame_count


def playback_tick(played_source_samples, sample_rate, output_fps):
    """Frame index covered by samples the device has actually played.

    The submitted cursor is not an input. Tick 0 covers the first period.
    A 4.0 s 24 kHz clip is tick 100, so the presentable indexes are 0..99.
    """
    if (type(played_source_samples) is not int or played_source_samples < 0
            or type(sample_rate) is not int or sample_rate <= 0
            or type(output_fps) is not int or output_fps <= 0):
        raise ValueError("invalid playback clock")
    return (played_source_samples * output_fps) // sample_rate
