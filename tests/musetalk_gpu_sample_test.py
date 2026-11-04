import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))

from musetalk_gpu_sample import parse_gpu_sample, summarize_gpu_samples


IDLE = "52, 180 MHz, 3090 MHz, 9.10 W, 0 %, Not Active, Not Active, Not Active, Not Active, Not Active, 0x0000000000000000"
CAPPED = "70, 1200 MHz, 3090 MHz, 55.5 W, 80 %, Not Active, Active, Not Active, Not Active, Not Active, 0x0000000000000004"


class GpuSampleTest(unittest.TestCase):
    def test_idle_line_parses_clock_and_clear_mask(self):
        sample = parse_gpu_sample(IDLE)
        self.assertTrue(sample["parsed"])
        self.assertEqual(sample["clock_mhz"], 180)
        self.assertEqual(sample["max_clock_mhz"], 3090)
        self.assertEqual(sample["power_w"], 9.10)
        self.assertEqual(sample["util_pct"], 0)
        self.assertFalse(sample["throttle_active"])
        self.assertFalse(any(sample["reasons"].values()))

    def test_power_cap_line_is_active(self):
        sample = parse_gpu_sample(CAPPED)
        self.assertTrue(sample["reasons"]["sw_power_cap"])
        self.assertTrue(sample["throttle_active"])

    def test_short_line_stays_unparsed(self):
        sample = parse_gpu_sample("52, 180 MHz")
        self.assertFalse(sample["parsed"])
        self.assertEqual(sample["raw"], "52, 180 MHz")

    def test_summary_uses_parsed_samples_and_an_observed_median(self):
        summary = summarize_gpu_samples([
            parse_gpu_sample(IDLE),
            parse_gpu_sample(CAPPED),
            parse_gpu_sample("not a sample"),
        ])
        self.assertEqual(summary["samples"], 3)
        self.assertEqual(summary["parsed"], 2)
        self.assertEqual(summary["unparsed"], 1)
        self.assertEqual(summary["clock_mhz"], {"min": 180, "max": 1200, "median": 1200})
        self.assertEqual(summary["util_pct"]["max"], 80)
        self.assertEqual(summary["power_w"]["max"], 55.5)
        self.assertEqual(summary["reason_counts"]["sw_power_cap"], 1)
        self.assertEqual(summary["reason_counts"]["hw_thermal_slowdown"], 0)
        self.assertEqual(summary["throttle_samples"], 1)
        self.assertEqual(summary["max_clock_mhz"], 3090)
        self.assertEqual(summary["memory_clock_mhz"]["median"], None)
        self.assertIsNone(summary["max_memory_clock_mhz"])

    def test_memory_clock_line_keeps_the_short_fields(self):
        line = (
            "57, 1612 MHz, 3090 MHz, 810 MHz, 14001 MHz, 93.00 W, 78 %, "
            "Not Active, Active, Not Active, Not Active, Not Active, 0x0000000000000004"
        )
        sample = parse_gpu_sample(line)
        self.assertTrue(sample["parsed"])
        self.assertEqual(sample["clock_mhz"], 1612)
        self.assertEqual(sample["power_w"], 93.00)
        self.assertEqual(sample["util_pct"], 78)
        self.assertEqual(sample["memory_clock_mhz"], 810)
        self.assertEqual(sample["max_memory_clock_mhz"], 14001)
        self.assertTrue(sample["reasons"]["sw_power_cap"])
        self.assertFalse(sample["reasons"]["hw_thermal_slowdown"])
        summary = summarize_gpu_samples([sample, parse_gpu_sample(IDLE)])
        self.assertEqual(summary["parsed"], 2)
        self.assertEqual(summary["clock_mhz"], {"min": 180, "max": 1612, "median": 1612})
        self.assertEqual(summary["memory_clock_mhz"], {"min": 810, "max": 810, "median": 810})
        self.assertEqual(summary["max_memory_clock_mhz"], 14001)

    def test_twelve_fields_stay_unparsed(self):
        sample = parse_gpu_sample(IDLE + ", extra")
        self.assertFalse(sample["parsed"])

    def test_empty_summary_has_no_invented_numbers(self):
        summary = summarize_gpu_samples([])
        self.assertEqual(summary["parsed"], 0)
        self.assertEqual(summary["clock_mhz"]["median"], None)
        self.assertEqual(summary["throttle_samples"], 0)


if __name__ == "__main__":
    unittest.main()
