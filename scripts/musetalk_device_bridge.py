"""Dev-only stdio client for the local WASAPI clock bridge.

The bridge plays PCM. This module never sends pixels and never starts a stream.
"""
import re
import subprocess
import threading
import time

CREATE_NO_WINDOW = 0x08000000
_READY = re.compile(r"BRIDGE_READY samples=(\d+) rate=(\d+)\Z")
_CLOCK = re.compile(r"CLOCK played=(\d+) submitted=(\d+) allowed=([01])\Z")
_STOPPED = re.compile(r"BRIDGE_STOPPED reason=([a-z0-9_]+) played=(\d+) submitted=(\d+)\Z")
_PAUSED = re.compile(r"PAUSED played=(\d+) submitted=(\d+)\Z")
_RESUMED = re.compile(r"RESUMED played=(\d+) submitted=(\d+)\Z")


class BridgeError(RuntimeError):
    pass


def parse_bridge_line(line):
    text = line.strip()
    ready = _READY.fullmatch(text)
    if ready:
        return ("ready", int(ready.group(1)), int(ready.group(2)))
    clock = _CLOCK.fullmatch(text)
    if clock:
        return ("clock", int(clock.group(1)), int(clock.group(2)), clock.group(3) == "1")
    stopped = _STOPPED.fullmatch(text)
    if stopped:
        return ("stopped", stopped.group(1), int(stopped.group(2)), int(stopped.group(3)))
    paused = _PAUSED.fullmatch(text)
    if paused:
        return ("paused", int(paused.group(1)), int(paused.group(2)))
    resumed = _RESUMED.fullmatch(text)
    if resumed:
        return ("resumed", int(resumed.group(1)), int(resumed.group(2)))
    return ("invalid", text)


def require_pcm_wav(path, channels=1, rate=24000, bits=16, frames=96000):
    with open(path, "rb") as stream:
        data = stream.read(65536)
    if len(data) < 44 or data[0:4] != b"RIFF" or data[8:12] != b"WAVE":
        raise BridgeError("bad wav")
    offset = 12
    found = None
    data_size = None
    while offset + 8 <= len(data):
        kind = data[offset:offset + 4]
        size = int.from_bytes(data[offset + 4:offset + 8], "little")
        start = offset + 8
        if start + size > len(data) and kind != b"data":
            break
        if kind == b"fmt " and size >= 16:
            found = (
                int.from_bytes(data[start:start + 2], "little"),
                int.from_bytes(data[start + 2:start + 4], "little"),
                int.from_bytes(data[start + 4:start + 8], "little"),
                int.from_bytes(data[start + 14:start + 16], "little"),
            )
        elif kind == b"data":
            data_size = size
            break
        offset = start + size + (size & 1)
    if found != (1, channels, rate, bits) or data_size != frames * (bits // 8) * channels:
        raise BridgeError(f"expected pcm s16 mono {rate} Hz {frames} frames")


class DeviceBridge:
    """Clock view. `latest` is non-consuming. `wait_update` waits for a new line."""

    def __init__(self, exe, wav, ready_timeout=8):
        self.exe = str(exe)
        self.wav = str(wav)
        self.ready_timeout = ready_timeout
        self.proc = None
        self.played = 0
        self.submitted = 0
        self.allowed = False
        self.stopped = False
        self.paused = False
        self.resumed = False
        self.reason = ""
        self.version = 0
        self._seen = 0
        self.stderr_tail = ""
        self._progress_after_stop = False
        self._cv = threading.Condition()
        self._stderr_thread = None
        self._reader = None

    def open(self):
        self.proc = subprocess.Popen(
            [self.exe, self.wav],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            bufsize=1,
            creationflags=CREATE_NO_WINDOW,
        )
        self._stderr_thread = threading.Thread(target=self._drain_stderr, daemon=True)
        self._stderr_thread.start()
        ready = self._readline(self.ready_timeout)
        parsed = parse_bridge_line(ready)
        if parsed[0] == "stopped":
            self._apply_stopped(parsed)
            try:
                self.proc.wait(timeout=3)
            except subprocess.TimeoutExpired:
                self.proc.kill()
            raise BridgeError(f"bridge stopped before ready: {parsed[1]}")
        if parsed[0] != "ready":
            self.close()
            raise BridgeError(f"unexpected bridge line: {ready.strip()}")
        if parsed[1] != 96000 or parsed[2] != 24000:
            self.close()
            raise BridgeError(f"unexpected wav clock: samples={parsed[1]} rate={parsed[2]}")
        self._reader = threading.Thread(target=self._read_stdout, daemon=True)
        self._reader.start()

    def go(self):
        self._write("GO\n")

    def pause(self, hold_s):
        """Stop the device stream, hold, then start it again. The clock is not reset."""
        if type(hold_s) not in (int, float) or isinstance(hold_s, bool) or not 0 < hold_s <= 2:
            raise BridgeError("pause hold out of range")
        self._write("PAUSE\n")
        if not self._wait(lambda: self.paused, 2):
            raise BridgeError(f"pause was not acknowledged: {self.reason}")
        with self._cv:
            played, submitted = self.played, self.submitted
        peak_played, peak_submitted = played, submitted
        end = time.monotonic() + float(hold_s)
        while time.monotonic() < end:
            time.sleep(min(0.05, max(0.0, end - time.monotonic())))
            with self._cv:
                peak_played = max(peak_played, self.played)
                peak_submitted = max(peak_submitted, self.submitted)
                if self.stopped:
                    raise BridgeError(f"stopped during pause: {self.reason}")
        with self._cv:
            self.resumed = False
        self._write("RESUME\n")
        if not self._wait(lambda: self.resumed, 2):
            raise BridgeError(f"resume was not acknowledged: {self.reason}")
        with self._cv:
            after_played, after_submitted = self.played, self.submitted
        return {
            "played_at_pause": played,
            "submitted_at_pause": submitted,
            "played_peak_during_hold": peak_played,
            "submitted_peak_during_hold": peak_submitted,
            "played_at_resume": after_played,
            "submitted_at_resume": after_submitted,
            "played_moved": peak_played != played,
            "submitted_moved": peak_submitted != submitted,
            "hold_seconds": float(hold_s),
        }

    def _wait(self, ready, timeout):
        deadline = time.monotonic() + timeout
        with self._cv:
            while not ready() and not self.stopped:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    break
                self._cv.wait(remaining)
            return ready() and not self.stopped

    def latest(self):
        with self._cv:
            return self.played, self.submitted, self.allowed, self.stopped

    def wait_update(self, timeout):
        with self._cv:
            if self.version == self._seen:
                self._cv.wait(timeout)
            self._seen = self.version
            return self.played, self.submitted, self.allowed, self.stopped

    def close(self):
        if self.proc is None:
            return self._snapshot()
        if not self.stopped and self.proc.poll() is None:
            try:
                self._write("STOP\n")
            except OSError:
                pass
        # CLOCK lines also notify. Keep waiting until STOPPED or the deadline.
        deadline = time.monotonic() + 3
        with self._cv:
            while not self.stopped:
                remaining = deadline - time.monotonic()
                if remaining <= 0:
                    break
                self._cv.wait(remaining)
            snapshot = self._snapshot()
        try:
            self.proc.wait(timeout=3)
        except subprocess.TimeoutExpired:
            self.proc.kill()
            snapshot["reason"] = snapshot["reason"] or "kill"
        return snapshot

    def _snapshot(self):
        return {
            "reason": self.reason,
            "played": self.played,
            "submitted": self.submitted,
            "progress_after_stop": self._progress_after_stop,
            "stderr_tail": self.stderr_tail,
        }

    def _write(self, text):
        self.proc.stdin.write(text)
        self.proc.stdin.flush()

    def _readline(self, timeout):
        box = []

        def read():
            box.append(self.proc.stdout.readline())

        thread = threading.Thread(target=read, daemon=True)
        thread.start()
        thread.join(timeout)
        if thread.is_alive() or not box or box[0] == "":
            self.close()
            raise BridgeError("bridge closed before ready")
        return box[0]

    def _read_stdout(self):
        for line in self.proc.stdout:
            parsed = parse_bridge_line(line)
            with self._cv:
                if self.stopped:
                    if parsed[0] == "clock":
                        self._progress_after_stop = True
                    continue
                if parsed[0] == "clock":
                    self.played, self.submitted, self.allowed = parsed[1], parsed[2], parsed[3]
                    self.version += 1
                    self._cv.notify_all()
                elif parsed[0] == "stopped":
                    self._apply_stopped(parsed)
                    self._cv.notify_all()
                    break
                elif parsed[0] == "paused":
                    self.played, self.submitted = parsed[1], parsed[2]
                    self.paused = True
                    self.version += 1
                    self._cv.notify_all()
                elif parsed[0] == "resumed":
                    self.played, self.submitted = parsed[1], parsed[2]
                    self.paused = False
                    self.resumed = True
                    self.version += 1
                    self._cv.notify_all()

    def _apply_stopped(self, parsed):
        self.reason = parsed[1]
        self.played = parsed[2]
        self.submitted = parsed[3]
        self.allowed = False
        self.stopped = True
        self._progress_after_stop = parsed[1] == "progress_after_stop"
        self.version += 1

    def _drain_stderr(self):
        chunks = []
        total = 0
        for line in self.proc.stderr:
            chunks.append(line)
            total += len(line)
            if total > 8000:
                break
        self.stderr_tail = "".join(chunks)[-4000:]
