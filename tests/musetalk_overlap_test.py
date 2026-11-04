import sys
import unittest
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from musetalk_overlap import run_pipeline

class OverlapScheduleTest(unittest.TestCase):
    def test_next_unet_is_issued_before_the_current_decode_finishes(self):
        calls = []
        def start_unet(index):
            calls.append(("unet", index))
            return index
        def start_decode(index, pending):
            calls.append(("decode-start", index, pending))
            return index
        def finish_decode(index, token):
            calls.append(("decode-finish", index, token))
            return "face-%s" % index
        def blend(index, faces):
            calls.append(("blend", index, faces))
        run_pipeline(3, start_unet, start_decode, finish_decode, blend)
        self.assertEqual(calls, [
            ("unet", 0),
            ("decode-start", 0, 0),
            ("unet", 1),
            ("decode-finish", 0, 0),
            ("blend", 0, "face-0"),
            ("decode-start", 1, 1),
            ("unet", 2),
            ("decode-finish", 1, 1),
            ("blend", 1, "face-1"),
            ("decode-start", 2, 2),
            ("decode-finish", 2, 2),
            ("blend", 2, "face-2"),
        ])

    def test_one_batch_does_not_start_a_second_unet(self):
        calls = []
        run_pipeline(
            1,
            lambda index: calls.append(("unet", index)) or index,
            lambda index, pending: calls.append(("decode-start", index)) or index,
            lambda index, token: calls.append(("decode-finish", index)) or "face",
            lambda index, faces: calls.append(("blend", index)),
        )
        self.assertEqual(calls, [("unet", 0), ("decode-start", 0), ("decode-finish", 0), ("blend", 0)])

    def test_bounds(self):
        noop = lambda *args: None
        for count in (0, -1, 513, True, 1.5):
            with self.assertRaises(ValueError):
                run_pipeline(count, noop, noop, noop, noop)

if __name__ == "__main__":
    unittest.main()
