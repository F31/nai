import sys
import unittest
import io
import json
import os
import tempfile
import types
import hmac
import struct
from unittest.mock import patch
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
from avatar_prepare_failure import classify


class PreparationFailureTest(unittest.TestCase):
    def test_cuda_has_specific_class_and_does_not_expose_details(self):
        self.assertEqual(classify(RuntimeError('CUDA error: unknown error /private/path')), 'CUDA_PREPARATION_FAILED')
        self.assertEqual(classify(RuntimeError('CUDA out of memory')), 'CUDA_MEMORY_EXHAUSTED')

    def test_timeout_and_non_cuda_failures_are_distinct(self):
        self.assertEqual(classify(TimeoutError('deadline')), 'PREPARATION_TIMEOUT')
        self.assertEqual(classify(FileNotFoundError('/private/model')), 'MODEL_PREPARATION_FAILED')

    def test_host_memory_and_wrapped_windows_commit_failure(self):
        self.assertEqual(classify(MemoryError()), 'HOST_MEMORY_EXHAUSTED')
        cause = OSError('private localized Windows error')
        cause.winerror = 1455
        wrapped = RuntimeError('model load failed')
        wrapped.__cause__ = cause
        self.assertEqual(classify(wrapped), 'HOST_MEMORY_EXHAUSTED')

    def test_worker_emits_only_signed_failure_without_ready_or_raw_channel(self):
        import flashhead_stream_worker as worker
        from v561 import avatar_stream_pb2 as wire
        for failure, code in [(RuntimeError('CUDA error: unknown error'), 'CUDA_PREPARATION_FAILED'),
                              (MemoryError(), 'HOST_MEMORY_EXHAUSTED')]:
            output = io.BytesIO()
            backend = types.ModuleType('flashhead_resident_backend')
            def fail(_config):
                raise failure
            backend.Backend = fail
            warmup = types.ModuleType('avatar_compiled_warmup')
            warmup.warmup = lambda *_: self.fail('must not warm up after load failure')
            control = types.ModuleType('avatar_control_output')
            control.private_control_output = lambda: output
            key = bytes(range(32))
            temporary_root = Path(__file__).resolve().parents[1] / 'target-windows/tool-cache/tmp/prepare-tests'
            temporary_root.mkdir(parents=True, exist_ok=True)
            with tempfile.TemporaryDirectory(dir=temporary_root) as directory:
                config = Path(directory) / 'config.json'
                config.write_text(json.dumps({'timeout_ms':45000, 'maintenance_timeout_ms':240000,
                    'portrait_sha256':'ab'*32}), encoding='utf-8')
                with patch.dict(sys.modules, {'flashhead_resident_backend':backend,
                        'avatar_compiled_warmup':warmup, 'avatar_control_output':control}), \
                     patch.dict(os.environ, {'NAI_V561_AVATAR_TOKEN':key.hex()}), \
                     patch.object(sys, 'argv', ['worker', '--config', str(config), '--raw-port', '1']), \
                     patch.object(sys, 'stderr', io.StringIO()):
                    self.assertEqual(worker.main(), 1)
            data = output.getvalue()
            length, = struct.unpack('<I', data[:4])
            self.assertEqual(length, len(data)-4)
            event = wire.Event.FromString(data[4:])
            self.assertEqual(event.kind, wire.KIND_FAILED)
            self.assertEqual(event.code, code)
            self.assertEqual((event.task_id,event.epoch,event.request_sequence), (0,0,0))
            mac = event.mac
            event.mac = b''
            self.assertTrue(hmac.compare_digest(mac, hmac.digest(key,
                event.SerializeToString(deterministic=True), 'sha256')))


if __name__ == '__main__':
    unittest.main()
