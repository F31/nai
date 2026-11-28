import importlib.util
from pathlib import Path
import unittest

path = Path(__file__).resolve().parents[2] / 'scripts' / 'flashhead-bounded-diagnostic.py'
spec = importlib.util.spec_from_file_location('flashhead_bounded_diagnostic', path)
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class AttentionSelectionTests(unittest.TestCase):
    def test_frame_number_requires_temporal_alignment_and_nonempty_output(self):
        module.validate_frame_number(33, 8, 9)
        module.validate_frame_number(49, 8, 9)
        for frame_num, stride, motion in [(48, 8, 9), (9, 8, 9), (33, 0, 9)]:
            with self.subTest(frame_num=frame_num, stride=stride, motion=motion), self.assertRaises(ValueError):
                module.validate_frame_number(frame_num, stride, motion)
    def test_required_flash_rejects_unavailable_or_higher_priority(self):
        for sage, flash2, flash3 in [(False, False, False), (True, True, False), (False, True, True)]:
            with self.subTest(sage=sage, flash2=flash2, flash3=flash3), self.assertRaises(RuntimeError):
                module.validate_required_attention(False, True, sage, flash2, flash3)
        module.validate_required_attention(False, True, False, True, False)

    def test_required_sage_rejects_fallback(self):
        with self.assertRaises(RuntimeError):
            module.validate_required_attention(True, False, False, True, False)
        module.validate_required_attention(True, False, True, False, False)


if __name__ == '__main__':
    unittest.main()
