import sys
import unittest
from pathlib import Path
from types import SimpleNamespace as NS
import torch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]/'scripts'))
from avatar_step_override import config_sample_steps, override_sample_steps, override_timesteps


class ConfigStepsTests(unittest.TestCase):
    def test_missing_or_valid_values(self):
        self.assertIsNone(config_sample_steps({}))
        self.assertEqual(config_sample_steps({'sample_steps': 2}), 2)
        self.assertEqual(config_sample_steps({'sample_steps': 3}), 3)
        self.assertEqual(config_sample_steps({'sample_steps': 4}), 4)

    def test_invalid_values_rejected_before_model_load(self):
        for bad in (0, 1, 5, '2', 2.0, True, [2]):
            with self.assertRaises(ValueError):
                config_sample_steps({'sample_steps': bad})


def make_backend(steps=4, use_transform=True):
    pipeline = NS(device=torch.device('cpu'), use_timestep_transform=use_transform,
                  num_timesteps=1000, timesteps=None)
    return NS(torch=torch, pipeline=pipeline, params={'sample_steps': steps},
              inference=NS(infer_params={'sample_steps': steps, 'sample_shift': 5}))


class StepOverrideTests(unittest.TestCase):
    def test_two_step_schedule_matches_upstream_values_and_updates_params(self):
        backend = make_backend()
        self.assertTrue(override_sample_steps(backend, 2, transform=lambda t, shift, num_timesteps: t + 1))
        values = [float(t.item()) for t in backend.pipeline.timesteps]
        self.assertEqual(values, [1001.0, 501.0, 1.0])
        self.assertEqual(backend.params['sample_steps'], 2)
        self.assertEqual(backend.inference.infer_params['sample_steps'], 2)
        self.assertEqual(len(backend.pipeline.timesteps), 3)

    def test_four_step_schedule_and_identity_without_transform(self):
        backend = make_backend(steps=2, use_transform=False)
        self.assertTrue(override_sample_steps(backend, 4, transform=lambda t, shift, num_timesteps: t))
        values = [float(t.item()) for t in backend.pipeline.timesteps]
        self.assertEqual(values, [1000.0, 750.0, 500.0, 250.0, 0.0])

    def test_three_step_schedule_is_four_step_grid_subset(self):
        backend = make_backend()
        self.assertTrue(override_sample_steps(backend, 3, transform=lambda t, shift, num_timesteps: t))
        values = [float(t.item()) for t in backend.pipeline.timesteps]
        self.assertEqual(values, [1000.0, 500.0, 250.0, 0.0])
        self.assertEqual(backend.params['sample_steps'], 3)
        self.assertEqual(backend.inference.infer_params['sample_steps'], 3)

    def test_same_value_is_noop_and_other_values_rejected(self):
        backend = make_backend()
        self.assertFalse(override_sample_steps(backend, 4))
        self.assertIsNone(backend.pipeline.timesteps)
        for bad in (0, 1, 5, 8, -2):
            with self.assertRaises(ValueError):
                override_sample_steps(backend, bad)
        self.assertEqual(backend.params['sample_steps'], 4)


class ExplicitTimestepsTests(unittest.TestCase):
    def test_explicit_list_order_transform_and_params(self):
        backend = make_backend()
        self.assertTrue(override_timesteps(backend, [1000, 500, 250],
                                           transform=lambda t, shift, num_timesteps: t * 2))
        values = [float(t.item()) for t in backend.pipeline.timesteps]
        self.assertEqual(values, [2000.0, 1000.0, 500.0, 0.0])
        self.assertEqual(backend.params['sample_steps'], 3)
        self.assertEqual(backend.inference.infer_params['sample_steps'], 3)

    def test_invalid_lists_rejected(self):
        backend = make_backend()
        for bad in ([], [1000]*9, [500, 1000], [1000, 1000], [0], [-1], [1001],
                    [1000, 500.5], '1000,500', (1000, 500), [True]):
            with self.assertRaises(ValueError):
                override_timesteps(backend, bad)
        self.assertIsNone(backend.pipeline.timesteps)


if __name__ == '__main__':
    unittest.main()
