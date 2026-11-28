import sys
import unittest
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
from flashhead_portrait import crop_box, resize_crop_preserving_aspect

class PortraitTests(unittest.TestCase):
    def test_clipped_rectangular_crop_is_padded_not_stretched(self):
        import numpy as np
        from PIL import Image
        image = np.zeros((2, 4, 3), dtype=np.uint8)
        image[0, :, 0] = 32
        image[1, :, 0] = 224
        actual = np.asarray(resize_crop_preserving_aspect(Image.fromarray(image), 4))
        self.assertEqual(actual[:, 0, 0].tolist(), [32, 32, 224, 224])
        self.assertEqual(actual.shape, (4, 4, 3))
    def test_single_face_is_composed_inside_original(self):
        box, score = crop_box([[0.3, 0.2, 0.5, 0.5]], [0.9], 1000, 800, 2.8, 0.5)
        self.assertEqual(box, (120, 0, 680, 532))
        self.assertEqual(score, 0.9)

    def test_missing_multiple_and_invalid_faces_fail(self):
        for boxes, scores in [([], []), ([[0.1, 0.1, 0.3, 0.3], [0.5, 0.1, 0.7, 0.3]], [0.9, 0.8]), ([[0.1, 0.1, 0.3, 0.3]], [0.4]), ([[-0.1, 0.1, 0.3, 0.3]], [0.9])]:
            with self.subTest(boxes=boxes), self.assertRaises(ValueError):
                crop_box(boxes, scores, 1000, 800, 2.8, 0.5)

    def test_invalid_parameters_rejected(self):
        for ratio, confidence in [(float('nan'), 0.5), (1, 0.5), (2.8, 1.1)]:
            with self.assertRaises(ValueError): crop_box([[0.2, 0.2, 0.4, 0.4]], [0.9], 1000, 800, ratio, confidence)

if __name__ == '__main__': unittest.main()
