import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
sys.path.insert(0, str(ROOT / "tests"))

from musetalk_unet_gaps import index_gap_traces
from musetalk_unet_span import cluster_intervals
from musetalk_unet_span_test import cluster_row, trace


def gap_row(gap_count=2, max_gap=1.5, median_gap=0.5, idle=2.0, span=22.0):
    row = cluster_row(span=span, idle=idle, overlap=0.0)
    row["gap_count"] = gap_count
    row["max_gap_ms"] = max_gap
    row["median_gap_ms"] = median_gap
    return row


def gap_rows():
    return [gap_row() for _ in range(4)]


def gap_trace(**kwargs):
    return trace(rows=gap_rows(), **kwargs)


class UnetGapsTest(unittest.TestCase):
    def test_two_gaps_keep_count_max_and_median(self):
        cluster = cluster_intervals([
            (0, 1_000_000),
            (2_000_000, 1_000_000),
            (6_000_000, 1_000_000),
        ])
        self.assertEqual(cluster["gap_count"], 2)
        self.assertAlmostEqual(cluster["idle_inside_ms"], 4.0)
        self.assertAlmostEqual(cluster["max_gap_ms"], 3.0)
        self.assertAlmostEqual(cluster["median_gap_ms"], 3.0)

    def test_three_gaps_use_the_upper_middle(self):
        cluster = cluster_intervals([
            (0, 1_000_000),
            (2_000_000, 1_000_000),
            (4_000_000, 1_000_000),
            (8_000_000, 1_000_000),
        ])
        self.assertEqual(cluster["gap_count"], 3)
        self.assertAlmostEqual(cluster["max_gap_ms"], 3.0)
        self.assertAlmostEqual(cluster["median_gap_ms"], 1.0)

    def test_adjacent_ops_have_zero_gaps(self):
        cluster = cluster_intervals([(0, 1_000_000), (1_000_000, 1_000_000)])
        self.assertEqual(cluster["gap_count"], 0)
        self.assertAlmostEqual(cluster["max_gap_ms"], 0.0)
        self.assertAlmostEqual(cluster["median_gap_ms"], 0.0)

    def test_overlap_does_not_create_a_gap(self):
        cluster = cluster_intervals([(0, 2_000_000), (500_000, 2_000_000)])
        self.assertEqual(cluster["gap_count"], 0)
        self.assertAlmostEqual(cluster["idle_inside_ms"], 0.0)

    def test_empty_intervals_have_no_gap_span(self):
        cluster = cluster_intervals([])
        self.assertEqual(cluster["gap_count"], 0)
        self.assertIsNone(cluster["max_gap_ms"])
        self.assertIsNone(cluster["median_gap_ms"])

    def test_two_processes_keep_the_gap_fields(self):
        result = index_gap_traces([gap_trace(), gap_trace()])
        row = result["diagnostics"][0]["rows"][0]
        self.assertEqual(row["gap_count"], 2)
        self.assertAlmostEqual(row["max_gap_ms"], 1.5)
        self.assertAlmostEqual(row["median_gap_ms"], 0.5)
        self.assertAlmostEqual(row["max_over_idle"], 1.5 / 2.0)
        self.assertEqual(len(result["diagnostics"][1]["rows"]), 4)

    def test_zero_idle_leaves_max_over_idle_empty(self):
        rows = [gap_row(gap_count=0, max_gap=0.0, median_gap=0.0, idle=0.0) for _ in range(4)]
        result = index_gap_traces([trace(rows=rows), gap_trace()])
        self.assertIsNone(result["diagnostics"][0]["rows"][0]["max_over_idle"])

    def test_missing_gap_count_is_refused(self):
        rows = gap_rows()
        del rows[0]["gap_count"]
        with self.assertRaises(ValueError):
            index_gap_traces([trace(rows=rows), gap_trace()])

    def test_empty_gaps_with_a_max_are_refused(self):
        rows = [gap_row(gap_count=0, max_gap=1.0, median_gap=0.0, idle=2.0) for _ in range(4)]
        with self.assertRaises(ValueError):
            index_gap_traces([trace(rows=rows), gap_trace()])

    def test_max_gap_past_idle_is_refused(self):
        rows = [gap_row(gap_count=1, max_gap=3.0, median_gap=3.0, idle=2.0) for _ in range(4)]
        with self.assertRaises(ValueError):
            index_gap_traces([trace(rows=rows), gap_trace()])

    def test_median_past_max_is_refused(self):
        rows = [gap_row(gap_count=2, max_gap=1.0, median_gap=1.5, idle=2.0) for _ in range(4)]
        with self.assertRaises(ValueError):
            index_gap_traces([trace(rows=rows), gap_trace()])

    def test_one_process_is_refused(self):
        with self.assertRaises(ValueError):
            index_gap_traces([gap_trace()])

    def test_cache_miss_is_refused(self):
        with self.assertRaises(ValueError):
            index_gap_traces([gap_trace(cache="miss"), gap_trace()])
