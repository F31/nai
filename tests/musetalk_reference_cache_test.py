import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location("cache", Path(__file__).resolve().parents[1] / "scripts/musetalk_reference_cache.py")
cache = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cache)


class ReferenceCacheTests(unittest.TestCase):
    def test_hit_and_identity_change(self):
        with tempfile.TemporaryDirectory() as root:
            identity = {"video": "a", "model": "b", "runtime": "c"}
            boxes = [[1, 2, 10, 20] for _ in range(120)]
            self.assertTrue(cache.save(root, identity, boxes, 960, 540))
            self.assertEqual(cache.load(root, identity, 960, 540), boxes)
            self.assertIsNone(cache.load(root, {**identity, "model": "changed"}, 960, 540))
            self.assertFalse(cache.save(root, identity, boxes, 960, 540))

    def test_corruption_and_bounds_never_hit(self):
        with tempfile.TemporaryDirectory() as root:
            identity = {"video": "a"}
            path = Path(root) / (cache.key(identity) + ".json")
            path.write_text('{"identity":', encoding="utf-8")
            self.assertIsNone(cache.load(root, identity, 960, 540))
            boxes = [[1, 2, 10, 20] for _ in range(120)]
            path.write_text(json.dumps({"identity": identity, "boxes": boxes, "boxes_sha256": cache.boxes_digest(boxes)}), encoding="utf-8")
            boxes[7] = [1, 2, 11, 20]
            value = json.loads(path.read_text(encoding="utf-8")); value["boxes"] = boxes
            path.write_text(json.dumps(value), encoding="utf-8")
            self.assertIsNone(cache.load(root, identity, 960, 540))
            boxes[7] = [1, 2, 961, 20]
            path.write_text(json.dumps({"identity": identity, "boxes": boxes}), encoding="utf-8")
            self.assertIsNone(cache.load(root, identity, 960, 540))
            with self.assertRaises(ValueError):
                cache.save(root, identity, boxes, 960, 540)


if __name__ == "__main__":
    unittest.main()
