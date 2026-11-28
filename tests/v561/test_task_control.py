"""Real native child + generated Protobuf + loopback fake Qwen; no model calls."""
import io
import os
from pathlib import Path
import queue
import struct
import subprocess
import sys
import threading
import time
import unittest
import wave
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'packages/contracts-python/src/generated'))
from v561 import task_control_pb2 as p
from v561 import media_pb2 as media


class NativeControlTests(unittest.TestCase):
    def setUp(self):
        self.release = threading.Event()
        self.arrived = threading.Event()
        self.calls = 0
        self.pcm_samples = 240
        owner = self
        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *_):
                pass
            def do_POST(self):
                self.rfile.read(int(self.headers['Content-Length']))
                owner.calls += 1
                owner.arrived.set()
                owner.release.wait(4)
                data = io.BytesIO()
                with wave.open(data, 'wb') as wav:
                    wav.setnchannels(1)
                    wav.setsampwidth(2)
                    wav.setframerate(24000)
                    wav.writeframes(b'\0\0' * owner.pcm_samples)
                try:
                    self.send_response(200)
                    self.send_header('Content-Length', str(len(data.getvalue())))
                    self.end_headers()
                    self.wfile.write(data.getvalue())
                except (BrokenPipeError, ConnectionResetError, ConnectionAbortedError):
                    pass
        self.server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        arguments = [
            os.environ['NAI_TASK_CONTROL'], '--endpoint',
            f'http://127.0.0.1:{self.server.server_port}/v1', '--deadline-ms', '4000'
        ]
        if self._testMethodName.startswith(('test_audio_', 'test_clock_')):
            arguments += ['--audio-output', os.environ['NAI_AUDIO_OUTPUT']]
        if self._testMethodName.startswith('test_clock_'):
            arguments += ['--clock-events','true']
        self.child = subprocess.Popen(arguments, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.events = queue.Queue(maxsize=32)
        def read():
            try:
                while prefix := self.child.stdout.read(4):
                    length, = struct.unpack('<I', prefix)
                    if length > 16384:
                        raise ValueError('oversize event')
                    self.events.put(p.Event.FromString(self.child.stdout.read(length)))
            except Exception as error:
                self.events.put(error)
        self.reader = threading.Thread(target=read, daemon=True)
        self.reader.start()

    def tearDown(self):
        self.release.set()
        if not self.child.stdin.closed:
            self.child.stdin.close()
        try:
            self.child.wait(timeout=2)
        except subprocess.TimeoutExpired:
            self.child.kill()
            self.child.wait(timeout=2)
        self.reader.join(timeout=1)
        self.child.stdout.close()
        self.child.stderr.close()
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=1)

    def send(self, seq, command, text='', task_id=0, version=1):
        data = p.Request(version=version, sequence=seq, command=command, text=text, task_id=task_id).SerializeToString()
        self.child.stdin.write(struct.pack('<I', len(data)) + data)
        self.child.stdin.flush()

    def event(self, kind, code):
        event = self.events.get(timeout=2)
        self.assertIsInstance(event, p.Event)
        if event.kind == p.EVENT_KIND_FAILED and kind != p.EVENT_KIND_FAILED:
            self.child.kill()
            self.child.wait(timeout=2)
            self.fail(f'unexpected {event.code}: ' + self.child.stderr.read().decode(errors='replace'))
        self.assertEqual((event.kind, event.code), (kind, code))
        return event

    def test_synthesis_reports_samples_without_playback(self):
        self.release.set()
        self.send(1, p.COMMAND_NARRATE, '你好')
        self.event(p.EVENT_KIND_ACCEPTED, 'SYNTHESIS_STARTED')
        event = self.event(p.EVENT_KIND_COMPLETED, 'SYNTHESIZED_NOT_PLAYED')
        self.assertEqual((event.samples, event.sample_rate), (240, 24000))
        self.assertEqual(self.calls, 1)

    def test_takeover_busy_late_completion_and_explicit_resume(self):
        self.send(1, p.COMMAND_NARRATE, '你好')
        self.event(p.EVENT_KIND_ACCEPTED, 'SYNTHESIS_STARTED')
        self.assertTrue(self.arrived.wait(2))
        self.send(2, p.COMMAND_ANSWER, '问题')
        self.event(p.EVENT_KIND_REJECTED, 'BUSY')
        self.send(3, p.COMMAND_TAKEOVER)
        self.event(p.EVENT_KIND_ACCEPTED, 'HUMAN_OWNER_NO_MIC')
        self.event(p.EVENT_KIND_CANCELLED, 'CANCELLED')
        self.send(4, p.COMMAND_STATUS)
        self.assertEqual(self.event(p.EVENT_KIND_STATUS, 'NO_MEDIA_OUTPUT').task_id, 3)
        self.send(5, p.COMMAND_NARRATE, '旧讲解')
        self.event(p.EVENT_KIND_REJECTED, 'OWNER_DENIED')
        self.send(6, p.COMMAND_FINISH, task_id=1)
        self.event(p.EVENT_KIND_REJECTED, 'STALE_TASK')
        self.send(7, p.COMMAND_FINISH, task_id=3)
        self.event(p.EVENT_KIND_COMPLETED, 'OWNER_RELEASED')
        self.release.set()
        self.send(8, p.COMMAND_NARRATE, '恢复讲解')
        self.event(p.EVENT_KIND_ACCEPTED, 'SYNTHESIS_STARTED')
        self.event(p.EVENT_KIND_COMPLETED, 'SYNTHESIZED_NOT_PLAYED')
        self.assertEqual(self.calls, 2)

    def test_stop_does_not_wait_for_supplier(self):
        self.send(1, p.COMMAND_NARRATE, '你好')
        self.event(p.EVENT_KIND_ACCEPTED, 'SYNTHESIS_STARTED')
        self.assertTrue(self.arrived.wait(2))
        start = time.monotonic()
        self.send(2, p.COMMAND_STOP)
        self.event(p.EVENT_KIND_STOPPED, 'LOGICALLY_STOPPED')
        self.assertEqual(self.child.wait(timeout=1), 0)
        self.assertLess(time.monotonic() - start, 1)
        self.assertEqual(self.calls, 1)

    def test_replay_version_and_unknown_command_never_call_model(self):
        self.send(1, p.COMMAND_STATUS, version=2)
        self.event(p.EVENT_KIND_REJECTED, 'INVALID_SEQUENCE_OR_VERSION')
        self.send(1, p.COMMAND_STATUS)
        self.event(p.EVENT_KIND_STATUS, 'NO_MEDIA_OUTPUT')
        self.send(1, p.COMMAND_NARRATE, '重放')
        self.event(p.EVENT_KIND_REJECTED, 'INVALID_SEQUENCE_OR_VERSION')
        self.send(2, 999)
        self.event(p.EVENT_KIND_REJECTED, 'UNKNOWN_COMMAND')
        self.assertEqual(self.calls, 0)

    def test_oversized_frame_fails_closed(self):
        self.child.stdin.write(struct.pack('<I', 16385))
        self.child.stdin.flush()
        self.assertEqual(self.child.wait(timeout=1), 1)
        self.assertEqual(self.calls, 0)

    def test_partial_frame_fails_closed(self):
        self.child.stdin.write(struct.pack('<I', 128) + b'\x08')
        self.child.stdin.close()
        self.assertEqual(self.child.wait(timeout=1), 1)
        self.assertEqual(self.calls, 0)

    def test_parent_eof_during_request_exits(self):
        self.send(1, p.COMMAND_NARRATE, '你好')
        self.event(p.EVENT_KIND_ACCEPTED, 'SYNTHESIS_STARTED')
        self.assertTrue(self.arrived.wait(2))
        self.child.stdin.close()
        self.assertEqual(self.child.wait(timeout=1), 0)

    def test_audio_silent_playback_reports_device_count(self):
        self.release.set()
        self.send(1, p.COMMAND_NARRATE, '你好')
        self.event(p.EVENT_KIND_ACCEPTED, 'SYNTHESIS_STARTED')
        event = self.event(p.EVENT_KIND_COMPLETED, 'AUDIO_PLAYBACK_COMPLETED')
        self.assertEqual(event.played_samples, 240)

    def test_audio_takeover_cancels_independent_playback(self):
        self.pcm_samples = 96000
        self.release.set()
        self.send(1, p.COMMAND_NARRATE, '你好')
        self.event(p.EVENT_KIND_ACCEPTED, 'SYNTHESIS_STARTED')
        self.assertTrue(self.arrived.wait(2))
        time.sleep(.7)
        self.send(2, p.COMMAND_TAKEOVER)
        self.event(p.EVENT_KIND_ACCEPTED, 'HUMAN_OWNER_NO_MIC')
        self.event(p.EVENT_KIND_CANCELLED, 'CANCELLED')
        self.send(3, p.COMMAND_STATUS)
        self.assertEqual(self.event(p.EVENT_KIND_STATUS, 'AUDIO_OUTPUT_ENABLED').task_id, 2)

    def test_clock_events_bound_to_active_task_and_stop_after_takeover(self):
        self.pcm_samples=48000
        self.release.set()
        self.send(1,p.COMMAND_NARRATE,'你好')
        self.event(p.EVENT_KIND_ACCEPTED,'SYNTHESIS_STARTED')
        while True:
            event=self.event(p.EVENT_KIND_CLOCK,'DEVICE_CLOCK')
            self.assertEqual(event.task_id,1)
            if event.clock.state==media.CLOCK_STATE_RUNNING:
                break
        self.send(2,p.COMMAND_TAKEOVER)
        # Already emitted old clocks may precede the serialized takeover receipt.
        while True:
            event=self.events.get(timeout=2)
            if event.kind==p.EVENT_KIND_ACCEPTED:
                self.assertEqual(event.task_id,2)
                break
            self.assertEqual(event.kind,p.EVENT_KIND_CLOCK)
        self.event(p.EVENT_KIND_CANCELLED,'CANCELLED')
        self.send(3,p.COMMAND_STATUS)
        self.assertEqual(self.event(p.EVENT_KIND_STATUS,'AUDIO_OUTPUT_ENABLED').task_id,2)


if __name__ == '__main__':
    unittest.main()
