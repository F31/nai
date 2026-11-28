import sys
import unittest
from pathlib import Path
from types import SimpleNamespace as NS
import torch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]/'scripts'))
from avatar_audio_condition_cache import projection_slot, pipeline_candidate


class Projection(torch.nn.Module):
    def __init__(self):
        super().__init__()
        self.calls = 0
    def forward(self, a, b):
        self.calls += 1
        return torch.cat((a, b), dim=-1)*3


class CacheTests(unittest.TestCase):
    def setUp(self):
        self.module = Projection().eval()
        self.args = (torch.tensor([[1.,2.]]), torch.tensor([[3.,4.]]))

    def test_equal_content_at_new_addresses_reuses_exact_output(self):
        expected = self.module(*self.args)
        self.module.calls = 0
        with torch.no_grad(), projection_slot(torch, self.module, 4, 4096) as stats:
            outputs = [self.module(*(x.clone() for x in self.args)) for _ in range(4)]
            self.assertTrue(all(torch.equal(x, expected) for x in outputs))
            self.assertEqual(self.module.calls, 1)
            self.assertEqual(stats['hits'], 3)
            self.assertLessEqual(stats['resident_bytes'], 4096)
        self.assertNotIn('forward', vars(self.module))

    def test_changed_input_or_modified_result_is_rejected(self):
        for output_mutation in (False, True):
            with torch.no_grad(), projection_slot(torch, self.module, 4, 4096):
                result = self.module(*self.args)
                if output_mutation:
                    result.add_(1)
                    args = self.args
                else:
                    args = (self.args[0]+1, self.args[1])
                with self.assertRaisesRegex(ValueError, 'CONDITION_CACHE_CHANGED'):
                    self.module(*args)
                with self.assertRaisesRegex(ValueError, 'CONDITION_CACHE_FAILED'):
                    self.module(*self.args)
            self.assertNotIn('forward', vars(self.module))

    def test_budgets_training_and_exception_restore(self):
        with torch.no_grad():
            with self.assertRaisesRegex(ValueError, 'CONDITION_CACHE_BYTES'):
                with projection_slot(torch, self.module, 4, 1):
                    self.module(*self.args)
            with projection_slot(torch, self.module, 1, 4096):
                self.module(*self.args)
                with self.assertRaisesRegex(ValueError, 'CONDITION_CACHE_CALL_LIMIT'):
                    self.module(*self.args)
            self.module.train()
            with self.assertRaisesRegex(ValueError, 'CONDITION_CACHE_CONFIG'):
                with projection_slot(torch, self.module, 1, 4096): pass
        self.assertNotIn('forward', vars(self.module))

    def test_each_video_block_gets_a_fresh_slot_and_restores_pipeline(self):
        pipeline = NS(model=NS(audio_proj=self.module))
        def run(pipeline, embedding):
            for _ in range(2):
                value = pipeline.model.audio_proj(*embedding)
            return value
        backend = NS(torch=torch, pipeline=pipeline, inference=NS(run_pipeline=run), params={'sample_steps':2})
        with torch.no_grad(), pipeline_candidate(backend, 4096, 2) as stats:
            backend.inference.run_pipeline(pipeline, self.args)
            backend.inference.run_pipeline(pipeline, self.args)
            self.assertEqual(self.module.calls, 2)
            self.assertEqual(stats['hits'], 2)
            with self.assertRaisesRegex(ValueError, 'CONDITION_CACHE_BLOCK_LIMIT'):
                backend.inference.run_pipeline(pipeline, self.args)
        self.assertIs(backend.inference.run_pipeline, run)
        self.assertNotIn('forward', vars(self.module))


if __name__ == '__main__': unittest.main()
