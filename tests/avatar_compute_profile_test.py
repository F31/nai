import sys
import unittest
import itertools
from pathlib import Path
from types import SimpleNamespace as NS

sys.path.insert(0, str(Path(__file__).resolve().parents[1]/'scripts'))
from avatar_compute_profile import BlockProfile


class ProfileTest(unittest.TestCase):
    def test_video_stages_preserve_calls_results_and_restore_inherited_methods(self):
        class Event:
            def __init__(self, enable_timing): pass
            def record(self): pass
            def elapsed_time(self, other): return 2.0
        class Model:
            def forward(self, value): return value
            def __call__(self, value): return self.forward(value)
        class Vae:
            def decode(self, value): return value
            def encode(self, value): return value
        pipeline = NS(model=Model(), vae=Vae())
        profile = BlockProfile(NS(Event=Event), itertools.count().__next__)
        profile.feature_done()
        value = object()
        with profile.video_stages(pipeline, 2):
            self.assertIs(pipeline.model(value), value)
            self.assertIs(pipeline.model(value), value)
            self.assertIs(pipeline.vae.decode(value), value)
            self.assertIs(pipeline.vae.encode(value), value)
            with self.assertRaises(ValueError):
                pipeline.model(value)
        self.assertNotIn('forward', vars(pipeline.model))
        self.assertNotIn('decode', vars(pipeline.vae))
        self.assertNotIn('encode', vars(pipeline.vae))
        profile.submitted()
        stages = profile.result(1, 1)['video_stages']
        self.assertEqual(stages['denoise']['calls'], 2)
        self.assertEqual(stages['denoise']['cuda_stream_ms'], 4)
        self.assertEqual(stages['vae_decode']['calls'], 1)

    def test_video_stage_exception_restores_original_instance_methods(self):
        class Event:
            def __init__(self, enable_timing): pass
            def record(self): pass
        def fail(): raise RuntimeError('model failure')
        pipeline = NS(model=NS(forward=fail), vae=NS(encode=fail, decode=fail))
        profile = BlockProfile(NS(Event=Event))
        with self.assertRaisesRegex(RuntimeError, 'model failure'):
            with profile.video_stages(pipeline, 1):
                pipeline.model.forward()
        self.assertIs(pipeline.model.forward, fail)
        self.assertIs(pipeline.vae.encode, fail)
        self.assertIs(pipeline.vae.decode, fail)

    def test_stage_boundaries_do_not_insert_synchronization(self):
        calls = []
        class Event:
            def __init__(self, enable_timing):
                self.index = len(calls)
                calls.append(('event', enable_timing))
            def record(self):
                calls.append(('record', self.index))
            def elapsed_time(self, other):
                return 1.25 if self.index == 0 else 7.5
        ticks = iter([1000000, 3000000, 8000000])
        profile = BlockProfile(NS(Event=Event), lambda: next(ticks))
        with self.assertRaises(ValueError):
            profile.result(1, 1)
        profile.feature_done()
        profile.submitted()
        result = profile.result(1, 1)
        self.assertEqual(result['phase'], 'business')
        self.assertEqual(result['feature_host_call_ms'], 2)
        self.assertEqual(result['video_host_call_ms'], 5)
        self.assertEqual(result['feature_cuda_stream_ms'], 1.25)
        self.assertEqual(result['video_cuda_stream_ms'], 7.5)
        self.assertEqual(len([x for x in calls if x[0]=='record']), 3)
        self.assertEqual(profile.result(0, 1)['phase'], 'warmup')


if __name__ == '__main__':
    unittest.main()
