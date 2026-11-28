import sys
import unittest
from pathlib import Path
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
from flashhead_mouth_quality_compare import face_relative_box


class MouthQualityCompareTests(unittest.TestCase):
    def test_crop_size_tracks_face_width(self):
        points = [SimpleNamespace(x=0.5, y=0.5) for _ in range(455)]
        points[234] = SimpleNamespace(x=0.25, y=0.5)
        points[454] = SimpleNamespace(x=0.75, y=0.5)
        self.assertEqual(face_relative_box(points, 1000, 1000), (350, 420, 650, 580))

    def test_invalid_landmarks_fail_closed(self):
        with self.assertRaises(ValueError):
            face_relative_box([], 512, 512)
        points = [SimpleNamespace(x=0.5, y=0.5) for _ in range(455)]
        with self.assertRaises(ValueError):
            face_relative_box(points, 512, 512)


if __name__ == '__main__':
    unittest.main()
