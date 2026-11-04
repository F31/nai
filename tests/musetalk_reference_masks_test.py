import sys
import unittest
from pathlib import Path
import numpy as np
from PIL import Image

root = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(root / "scripts"))
sys.path.insert(0, str(root / "target-windows/realistic-avatar-research/MuseTalk"))
import musetalk_reference_masks as masks
from musetalk.utils.blending import get_image

class MaskTests(unittest.TestCase):
    def test_exact_blend_for_each_frame_and_edge_crop(self):
        rng = np.random.default_rng(8)
        def parser(image, mode):
            self.assertEqual(mode, "jaw")
            return Image.fromarray(np.asarray(image)[:, :, 0])
        for box in ([10, 8, 70, 70], [0, 0, 20, 35], [65, 55, 95, 79]):
            image = rng.integers(0, 256, (80, 96, 3), dtype=np.uint8)
            x, y, x1, y1 = box
            face = rng.integers(0, 256, (y1-y, x1-x, 3), dtype=np.uint8)
            expected = get_image(image, face, box, mode="jaw", fp=parser)
            actual = masks.blend(image, face, box, masks.prepare_mask(image, box, parser))
            np.testing.assert_array_equal(actual, expected)
    def test_pairing_and_count_are_bounded(self):
        with self.assertRaises(ValueError): masks.prepare_all([], [], None)
        with self.assertRaises(ValueError): masks.prepare_all([0], [], None)
        with self.assertRaises(ValueError): masks.prepare_all([0] * 121, [0] * 121, None)

if __name__ == "__main__": unittest.main()
