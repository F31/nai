import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from musetalk_unet_split import device_time_ms, index_split_traces, summarize_diagnostic


def diagnostic_rows(outer=90.0, device=80.0):
    return [
        {"outer_ms": outer + index, "device_ms": device + index, "kernel_calls": 12 + index}
        for index in range(4)
    ]


def window(unet=2.3):
    return {
        "label": "512-split",
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


class UnetSplitTest(unittest.TestCase):
    def test_device_time_converts_microseconds(self):
        split = device_time_ms([(1824.0, 1), (1000.0, 2)])
        self.assertAlmostEqual(split["device_ms"], 2.824)
        self.assertEqual(split["kernel_calls"], 3)

    def test_empty_device_rows_are_zero(self):
        split = device_time_ms([])
        self.assertEqual(split["device_ms"], 0.0)
        self.assertEqual(split["kernel_calls"], 0)

    def test_three_diagnostic_rows_are_refused(self):
        with self.assertRaises(ValueError):
            summarize_diagnostic(diagnostic_rows()[:3])

    def test_ratio_uses_each_row(self):
        rows = summarize_diagnostic([
            {"outer_ms": 100.0, "device_ms": 25.0, "kernel_calls": 4},
            {"outer_ms": 80.0, "device_ms": 80.0, "kernel_calls": 5},
            {"outer_ms": 0.0, "device_ms": 0.0, "kernel_calls": 0},
            {"outer_ms": 50.0, "device_ms": 10.0, "kernel_calls": 1},
        ])
        self.assertEqual(len(rows), 4)
        self.assertAlmostEqual(rows[0]["device_over_outer"], 0.25)
        self.assertAlmostEqual(rows[1]["device_over_outer"], 1.0)
        self.assertIsNone(rows[2]["device_over_outer"])

    def test_two_processes_keep_both_windows_and_four_rows(self):
        result = index_split_traces([trace(2.305), trace(1.09)])
        self.assertEqual(result["process_count"], 2)
        self.assertEqual(result["window_count"], 2)
        self.assertEqual(result["diagnostic_batches"], 4)
        self.assertEqual([item["unet_seconds"] for item in result["windows"]], [2.305, 1.09])
        self.assertEqual(result["windows"][0]["memory_clock_mhz"]["median"], 810.0)
        self.assertEqual(len(result["diagnostics"][1]["rows"]), 4)
        self.assertEqual(result["windows"][0]["unet_batch_ms_min"], 90.0)

    def test_one_process_is_refused(self):
        with self.assertRaises(ValueError):
            index_split_traces([trace()])

    def test_three_processes_are_refused(self):
        with self.assertRaises(ValueError):
            index_split_traces([trace(), trace(), trace()])

    def test_missing_memory_median_is_refused(self):
        with self.assertRaises(ValueError):
            index_split_traces([trace(), trace(memory=None)])

    def test_cache_miss_is_refused(self):
        with self.assertRaises(ValueError):
            index_split_traces([trace(cache="miss"), trace()])

    def test_rewritten_cache_is_refused(self):
        with self.assertRaises(ValueError):
            index_split_traces([trace(rewritten=True), trace()])

    def test_short_counted_batches_are_refused(self):
        with self.assertRaises(ValueError):
            index_split_traces([trace(batches=24), trace()])

    def test_unparsed_samples_are_refused(self):
        with self.assertRaises(ValueError):
            index_split_traces([trace(parsed=7), trace()])

    def test_short_diagnostic_is_refused(self):
        with self.assertRaises(ValueError):
            index_split_traces([trace(rows=diagnostic_rows()[:3]), trace()])
