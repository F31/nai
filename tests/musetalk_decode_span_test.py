import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
sys.path.insert(0, str(ROOT / "tests"))

from musetalk_decode_span import index_decode_traces
from musetalk_unet_gaps_test import gap_trace, gap_rows
from musetalk_unet_span_test import trace


class DecodeSpanTest(unittest.TestCase):
    def test_two_processes_keep_decode_gap_fields(self):
        first = gap_trace()
        first["windows"][0]["label"] = "512-decode"
        second = gap_trace()
        second["windows"][0]["label"] = "512-decode"
        result = index_decode_traces([first, second])
        self.assertEqual(result["process_count"], 2)
        self.assertEqual(result["windows"][0]["label"], "512-decode")
        row = result["diagnostics"][0]["rows"][0]
        self.assertEqual(row["gap_count"], 2)
        self.assertAlmostEqual(row["max_gap_ms"], 1.5)

    def test_missing_gap_count_is_refused(self):
        rows = gap_rows()
        del rows[0]["gap_count"]
        with self.assertRaises(ValueError):
            index_decode_traces([trace(rows=rows), gap_trace()])

    def test_one_process_is_refused(self):
        with self.assertRaises(ValueError):
            index_decode_traces([gap_trace()])
