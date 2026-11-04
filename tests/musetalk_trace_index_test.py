import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from musetalk_trace_index import index_traces


def window(unet, batches=25, parsed=4, samples=4):
    return {
        "label": "512-trace",
        "wall_seconds": round(unet + 2.1, 3),
        "ms_per_frame": 40.0,
        "unet_seconds": unet,
        "decode_seconds": 1.7,
        "blend_seconds": 0.3,
        "steady_bar_met": False,
        "batches": [{"unet_ms": 40.0 + index, "decode_ms": 68.0, "blend_ms": 10.0} for index in range(batches)],
        "gpu_summary": {
            "samples": samples,
            "parsed": parsed,
            "clock_mhz": {"min": 1100.0, "max": 1600.0, "median": 1400.0},
            "power_w": {"min": 50.0, "max": 150.0, "median": 90.0},
            "util_pct": {"min": 10.0, "max": 80.0, "median": 70.0},
            "temperature_c": {"min": 55.0, "max": 62.0, "median": 58.0},
            "reason_counts": {
                "gpu_idle": 0,
                "sw_power_cap": samples,
                "hw_slowdown": 0,
                "hw_thermal_slowdown": 0,
                "sw_thermal_slowdown": 0,
            },
            "max_clock_mhz": 3090.0,
        },
    }


def trace(first, second, **window_kwargs):
    return {
        "location_cache": "hit",
        "cache_rewritten": False,
        "runner_untouched": True,
        "windows": [window(first, **window_kwargs), window(second, **window_kwargs)],
    }


class TraceIndexTest(unittest.TestCase):
    def test_three_processes_keep_fast_band_below_the_cut(self):
        result = index_traces([trace(1.09, 1.538), trace(0.993, 1.197), trace(1.2, 1.4)])
        self.assertEqual(result["process_count"], 3)
        self.assertEqual(result["window_count"], 6)
        self.assertEqual(result["windows_at_or_above_slow_cut"], 0)
        self.assertEqual(result["unet_seconds"], [1.09, 1.538, 0.993, 1.197, 1.2, 1.4])

    def test_previously_seen_slow_band_counts_and_1_538_does_not(self):
        result = index_traces([trace(1.538, 2.336), trace(2.439, 1.09), trace(1.0, 1.1)])
        self.assertEqual(result["windows_at_or_above_slow_cut"], 2)
        self.assertEqual(result["windows"][1]["unet_batch_ms_min"], 40.0)
        self.assertEqual(result["windows"][1]["unet_batch_ms_max"], 64.0)

    def test_two_processes_are_refused(self):
        with self.assertRaises(ValueError):
            index_traces([trace(1.0, 1.1), trace(1.2, 1.3)])

    def test_unparsed_window_is_refused(self):
        with self.assertRaises(ValueError):
            index_traces([
                trace(1.0, 1.1, parsed=0, samples=4),
                trace(1.2, 1.3),
                trace(1.4, 1.5),
            ])

    def test_short_batch_list_is_refused(self):
        with self.assertRaises(ValueError):
            index_traces([
                trace(1.0, 1.1, batches=24),
                trace(1.2, 1.3),
                trace(1.4, 1.5),
            ])
