import struct
import sys
import tempfile
import threading
import time
import unittest
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import musetalk_device_bridge as bridge


def wav_bytes(frames, rate=24000, channels=1, bits=16, audio_format=1):
    payload = b"\x00" * (frames * channels * (bits // 8))
    fmt = struct.pack("<HHIIHH", audio_format, channels, rate, rate * channels * bits // 8,
                      channels * bits // 8, bits)
    chunks = b"fmt " + struct.pack("<I", len(fmt)) + fmt + b"data" + struct.pack("<I", len(payload)) + payload
    return b"RIFF" + struct.pack("<I", len(chunks)) + b"WAVE" + chunks


class DeviceBridgeProtocolTest(unittest.TestCase):
    def test_parse_ready_clock_and_stop(self):
        self.assertEqual(bridge.parse_bridge_line("BRIDGE_READY samples=96000 rate=24000\n"), ("ready", 96000, 24000))
        self.assertEqual(bridge.parse_bridge_line("CLOCK played=960 submitted=1920 allowed=1"), ("clock", 960, 1920, True))
        self.assertEqual(bridge.parse_bridge_line("CLOCK played=0 submitted=0 allowed=0"), ("clock", 0, 0, False))
        self.assertEqual(bridge.parse_bridge_line("BRIDGE_STOPPED reason=clip_end played=96000 submitted=96000"),
                         ("stopped", "clip_end", 96000, 96000))
        self.assertEqual(bridge.parse_bridge_line("PAUSED played=24000 submitted=30000"), ("paused", 24000, 30000))
        self.assertEqual(bridge.parse_bridge_line("RESUMED played=24000 submitted=30000"), ("resumed", 24000, 30000))
        self.assertEqual(bridge.parse_bridge_line("CLOCK played=1 submitted=1 allowed=2")[0], "invalid")

    def test_wav_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "clip.wav"
            path.write_bytes(wav_bytes(96000))
            bridge.require_pcm_wav(path)
            path.write_bytes(wav_bytes(96000, rate=48000))
            with self.assertRaises(bridge.BridgeError):
                bridge.require_pcm_wav(path)

    def test_close_keeps_the_stopped_line_after_clock_updates(self):
        client = bridge.DeviceBridge("unused", "unused")
        written = []

        class Proc:
            def poll(self):
                return None

            def wait(self, timeout):
                return 0

            def kill(self):
                raise AssertionError("kill")

        client.proc = Proc()
        client._write = lambda text: written.append(text)

        def clocks_then_stop():
            time.sleep(0.05)
            for played in (100, 200, 300):
                with client._cv:
                    client.played = played
                    client.submitted = 96000
                    client.allowed = True
                    client.version += 1
                    client._cv.notify_all()
                time.sleep(0.05)
            with client._cv:
                client._apply_stopped(("stopped", "stop", 400, 96000))
                client._cv.notify_all()

        thread = threading.Thread(target=clocks_then_stop)
        thread.start()
        snapshot = client.close()
        thread.join(1)
        self.assertFalse(thread.is_alive())
        self.assertEqual(written, ["STOP\n"])
        self.assertEqual(snapshot["reason"], "stop")
        self.assertEqual(snapshot["played"], 400)
        self.assertFalse(snapshot["progress_after_stop"])

    def test_pause_records_a_frozen_hold_and_then_resume(self):
        client = bridge.DeviceBridge("unused", "unused")
        written = []
        client._write = lambda text: written.append(text)

        def answer():
            time.sleep(0.05)
            with client._cv:
                client.played, client.submitted = 24000, 30000
                client.paused = True
                client._cv.notify_all()
            deadline = time.monotonic() + 2
            while "RESUME\n" not in written and time.monotonic() < deadline:
                time.sleep(0.02)
            with client._cv:
                client.played, client.submitted = 24000, 30000
                client.resumed = True
                client._cv.notify_all()

        thread = threading.Thread(target=answer)
        thread.start()
        record = client.pause(0.2)
        thread.join(1)
        self.assertFalse(thread.is_alive())
        self.assertEqual(written, ["PAUSE\n", "RESUME\n"])
        self.assertEqual(record["played_at_pause"], 24000)
        self.assertEqual(record["submitted_at_pause"], 30000)
        self.assertFalse(record["played_moved"])
        self.assertFalse(record["submitted_moved"])
        self.assertEqual(record["played_at_resume"], 24000)
if __name__ == "__main__": unittest.main()
