import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from musetalk_composite_scale import scale_box


class CompositeScaleTest(unittest.TestCase):
    def test_public_contract_scales_the_whole_research_frame(self):
        self.assertEqual(scale_box([0, 0, 960, 540], 960, 540, 512, 288), [0, 0, 512, 288])

    def test_face_rectangle_keeps_the_same_eighths(self):
        self.assertEqual(scale_box([150, 60, 450, 420], 960, 540, 512, 288), [80, 32, 240, 224])

    def test_identity_size_returns_the_same_box(self):
        box = [10, 20, 110, 220]
        self.assertEqual(scale_box(box, 960, 540, 960, 540), box)

    def test_one_pixel_source_box_stays_inside_the_destination(self):
        scaled = scale_box([959, 539, 960, 540], 960, 540, 512, 288)
        left, top, right, bottom = scaled
        self.assertTrue(0 <= left < right <= 512)
        self.assertTrue(0 <= top < bottom <= 288)

    def test_rejects_a_box_outside_the_source(self):
        for box in ([0, 0, 961, 10], [-1, 0, 10, 10], [10, 10, 10, 20], [1.5, 0, 10, 10]):
            with self.assertRaises(ValueError):
                scale_box(box, 960, 540, 512, 288)

    def test_rejects_a_non_positive_destination(self):
        with self.assertRaises(ValueError):
            scale_box([0, 0, 10, 10], 960, 540, 0, 288)


if __name__ == "__main__":
    unittest.main()
