"""Synthetic timebase tests; these do not validate real audio features."""
import unittest
import numpy as np
from avatar_semantic_dataset import audio_window


class AudioWindowTests(unittest.TestCase):
    def setUp(self):
        self.features = np.repeat(np.arange(1000, dtype=np.float32)[:, None], 1024, axis=1)

    def test_source_offset_is_not_patch_index(self):
        result = audio_window(self.features, 250, 25)
        self.assertEqual(result.shape, (40, 1024))
        np.testing.assert_array_equal(result[:, 0], np.arange(480, 520))

    def test_initial_boundary_clamps_without_wrapping(self):
        result = audio_window(self.features, 0, 25)
        np.testing.assert_array_equal(result[:21, 0], np.zeros(21))
        self.assertEqual(result[-1, 0], 19)

    def test_audio_missing_frame_is_rejected(self):
        with self.assertRaises(ValueError):
            audio_window(self.features, 500, 25)

    def test_invalid_features_or_timebase_rejected(self):
        for features, fps in ((self.features[:, :10], 25), (self.features, 30), (self.features * np.nan, 25)):
            with self.assertRaises(ValueError):
                audio_window(features, 0, fps)


if __name__ == '__main__':
    unittest.main()
