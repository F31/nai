import ast
from pathlib import Path
import sys
from types import SimpleNamespace as NS
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2]/'scripts'))
from avatar_gpu_stages import without_timing_sync, timing_sync_candidate, Stages

torch = NS(cuda=NS(synchronize=lambda: None))


class Pipeline:
    def generate(self, x):
        torch.cuda.synchronize()
        y = x * 3 + 2
        print(f'[generate] {y}')
        torch.cuda.synchronize()
        return y


class ProfileTests(unittest.TestCase):
    def test_only_timing_statements_removed_and_method_restored_on_failure(self):
        pipeline = Pipeline()
        original = pipeline.generate
        with self.assertRaisesRegex(RuntimeError, 'TEST_FAILURE'):
            with timing_sync_candidate(pipeline) as removed:
                self.assertEqual(removed, {'sync': 2, 'print': 1})
                self.assertEqual(pipeline.generate(7), 23)
                raise RuntimeError('TEST_FAILURE')
        self.assertEqual(pipeline.generate, original)

    def test_reject_unknown_or_side_effecting_statements(self):
        for source in ['def other(): pass', 'def generate(): return 1',
                       'def generate(): torch.cuda.synchronize(1)',
                       'def generate():\n torch.cuda.synchronize()\n print(f"{mutate()}")']:
            with self.assertRaises(ValueError):
                without_timing_sync(source)

    def test_preserve_algorithm_and_sync_in_other_call_expressions(self):
        tree, counts = without_timing_sync('def generate(x):\n torch.cuda.synchronize()\n return f(x)')
        self.assertEqual(counts['sync'], 1)
        self.assertIn('return f(x)', ast.unparse(tree))

    def test_stage_hooks_restore_after_failure(self):
        owner = NS(preprocess_audio=lambda x: x, model=NS(forward=lambda x: x),
                   vae=NS(decode=lambda x: x, encode=lambda x: x))
        originals = (owner.preprocess_audio, owner.model.forward, owner.vae.decode, owner.vae.encode)
        class Event:
            def __init__(self, **_): pass
            def record(self): pass
            def synchronize(self): pass
            def elapsed_time(self, _): return 2.
        with self.assertRaises(RuntimeError):
            with Stages(NS(cuda=NS(Event=Event)), owner) as stages:
                self.assertEqual(owner.model.forward(4), 4)
                raise RuntimeError()
        self.assertEqual(originals, (owner.preprocess_audio, owner.model.forward, owner.vae.decode, owner.vae.encode))
        self.assertEqual(stages.report()['denoise']['cuda_stream_ms'], 2.)


if __name__ == '__main__': unittest.main()
