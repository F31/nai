import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from musetalk_unet_span import cluster_intervals, index_span_traces


def cluster_row(outer=40.0, span=21.0, idle=1.0, overlap=0.5):
    return {
        "outer_ms": outer,
        "device_ms": 20.0,
        "kernel_calls": 1255,
        "span_ms": span,
        "idle_inside_ms": idle,
        "overlap_ms": overlap,
        "op_count": 1255,
        "kineto_device_ms": 20.5,
    }


def diagnostic_rows():
    return [cluster_row(outer=40.0 + index) for index in range(4)]


def window(unet=2.3):
    return {
        "label": "512-span",
        "wall_seconds": 4.7,
        "ms_per_frame": 47.0,
        "unet_seconds": unet,
        "decode_seconds": 1.72,
        "blend_seconds": 0.6,
        "steady_bar_met": False,
        "batches": [{"unet_ms": 90.0 + index} for index in range(25)],
        "gpu_summary": {
            "samples": 8,
            "parsed": 8,
            "clock_mhz": {"min": 1600.0, "max": 1900.0, "median": 1750.0},
            "memory_clock_mhz": {"min": 810.0, "max": 810.0, "median": 810.0},
            "max_memory_clock_mhz": 14001.0,
            "power_w": {"min": 90.0, "max": 150.0, "median": 100.0},
            "util_pct": {"min": 20.0, "max": 60.0, "median": 40.0},
            "temperature_c": {"min": 55.0, "max": 64.0, "median": 60.0},
            "reason_counts": {"sw_power_cap": 8},
            "max_clock_mhz": 3090.0,
        },
    }


def trace(unet=2.3, rows=None, cache="hit", rewritten=False, runner=True, batches=25, parsed=8, memory=810.0):
    item = window(unet)
    item["batches"] = [{"unet_ms": 90.0} for _ in range(batches)]
    item["gpu_summary"]["parsed"] = parsed
    item["gpu_summary"]["memory_clock_mhz"] = {
        "min": memory, "max": memory, "median": memory,
    }
    return {
        "location_cache": cache,
        "cache_rewritten": rewritten,
        "runner_untouched": runner,
        "windows": [item],
        "diagnostic": diagnostic_rows() if rows is None else rows,
    }


class UnetSpanTest(unittest.TestCase):
    def test_adjacent_ops_have_no_idle(self):
        cluster = cluster_intervals([(0, 1_000_000), (1_000_000, 1_000_000)])
        self.assertEqual(cluster["op_count"], 2)
        self.assertAlmostEqual(cluster["span_ms"], 2.0)
        self.assertAlmostEqual(cluster["device_ms"], 2.0)
        self.assertAlmostEqual(cluster["idle_inside_ms"], 0.0)
        self.assertAlmostEqual(cluster["overlap_ms"], 0.0)

    def test_gap_is_idle_inside_the_span(self):
        cluster = cluster_intervals([(0, 1_000_000), (3_000_000, 1_000_000)])
        self.assertAlmostEqual(cluster["span_ms"], 4.0)
        self.assertAlmostEqual(cluster["device_ms"], 2.0)
        self.assertAlmostEqual(cluster["idle_inside_ms"], 2.0)
        self.assertAlmostEqual(cluster["overlap_ms"], 0.0)

    def test_overlap_is_not_idle(self):
        cluster = cluster_intervals([(0, 2_000_000), (500_000, 2_000_000)])
        self.assertAlmostEqual(cluster["span_ms"], 2.5)
        self.assertAlmostEqual(cluster["device_ms"], 4.0)
        self.assertAlmostEqual(cluster["idle_inside_ms"], 0.0)
        self.assertAlmostEqual(cluster["overlap_ms"], 1.5)

    def test_nested_op_counts_as_overlap(self):
        cluster = cluster_intervals([(0, 5_000_000), (1_000_000, 1_000_000)])
        self.assertAlmostEqual(cluster["span_ms"], 5.0)
        self.assertAlmostEqual(cluster["idle_inside_ms"], 0.0)
        self.assertAlmostEqual(cluster["overlap_ms"], 1.0)

    def test_zero_duration_does_not_extend_the_span(self):
        cluster = cluster_intervals([(0, 1_000_000), (9_000_000, 0)])
        self.assertEqual(cluster["op_count"], 1)
        self.assertAlmostEqual(cluster["span_ms"], 1.0)

    def test_negative_duration_is_refused(self):
        with self.assertRaises(ValueError):
            cluster_intervals([(0, -1)])

    def test_empty_intervals_have_no_span(self):
        cluster = cluster_intervals([])
        self.assertEqual(cluster["op_count"], 0)
        self.assertIsNone(cluster["span_ms"])
        self.assertIsNone(cluster["idle_inside_ms"])

    def test_two_processes_keep_the_difference(self):
        result = index_span_traces([trace(2.361), trace(1.048)])
        self.assertEqual(result["process_count"], 2)
        self.assertEqual([item["unet_seconds"] for item in result["windows"]], [2.361, 1.048])
        row = result["diagnostics"][0]["rows"][0]
        self.assertAlmostEqual(row["outside_ms"], 40.0 - 21.0)
        self.assertEqual(len(result["diagnostics"][1]["rows"]), 4)

    def test_span_longer_than_the_event_stays_negative(self):
        rows = [cluster_row(outer=10.0, span=12.0) for _ in range(4)]
        result = index_span_traces([trace(rows=rows), trace()])
        self.assertAlmostEqual(result["diagnostics"][0]["rows"][0]["outside_ms"], -2.0)

    def test_one_process_is_refused(self):
        with self.assertRaises(ValueError):
            index_span_traces([trace()])

    def test_three_processes_are_refused(self):
        with self.assertRaises(ValueError):
            index_span_traces([trace(), trace(), trace()])

    def test_missing_span_is_refused(self):
        rows = diagnostic_rows()
        del rows[0]["span_ms"]
        with self.assertRaises(ValueError):
            index_span_traces([trace(rows=rows), trace()])

    def test_missing_memory_median_is_refused(self):
        with self.assertRaises(ValueError):
            index_span_traces([trace(), trace(memory=None)])

    def test_cache_miss_is_refused(self):
        with self.assertRaises(ValueError):
            index_span_traces([trace(cache="miss"), trace()])

    def test_rewritten_cache_is_refused(self):
        with self.assertRaises(ValueError):
            index_span_traces([trace(rewritten=True), trace()])

    def test_short_counted_batches_are_refused(self):
        with self.assertRaises(ValueError):
            index_span_traces([trace(batches=24), trace()])

    def test_unparsed_samples_are_refused(self):
        with self.assertRaises(ValueError):
            index_span_traces([trace(parsed=7), trace()])

    def test_short_diagnostic_is_refused(self):
        with self.assertRaises(ValueError):
            index_span_traces([trace(rows=diagnostic_rows()[:3]), trace()])
