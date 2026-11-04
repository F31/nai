"""Parse NVIDIA sample lines taken during a research window.

The short line is temperature, SM clock, max SM clock, power, utilization,
five throttle reasons, and the active reason mask. The long line inserts
memory clock and max memory clock after the max SM clock. Any other length
stays unparsed. The summary uses observed samples only. Its median is the
upper middle sorted sample, so the reported clock is one of the samples.
"""

REASON_NAMES = (
    "gpu_idle",
    "sw_power_cap",
    "hw_slowdown",
    "hw_thermal_slowdown",
    "sw_thermal_slowdown",
)

_HEAD = (
    "temperature.gpu",
    "clocks.sm",
    "clocks.max.sm",
)
_TAIL = (
    "power.draw",
    "utilization.gpu",
    *("clocks_throttle_reasons." + name for name in REASON_NAMES),
    "clocks_throttle_reasons.active",
)
GPU_QUERY = ",".join(_HEAD + _TAIL)
GPU_QUERY_WITH_MEMORY = ",".join(_HEAD + ("clocks.mem", "clocks.max.mem") + _TAIL)
_BASE_FIELDS = 6 + len(REASON_NAMES)
_MEMORY_FIELDS = _BASE_FIELDS + 2


def _first_number(token):
    number = ""
    seen_dot = False
    for char in token.strip():
        if char.isdigit() or (char == "." and not seen_dot and number):
            if char == ".":
                seen_dot = True
            number += char
        elif number:
            break
    if not number:
        return None
    return float(number)


def parse_gpu_sample(line):
    parts = [part.strip() for part in line.split(",")]
    memory_clock = None
    max_memory_clock = None
    extended = len(parts) == _MEMORY_FIELDS
    if extended:
        memory_clock = _first_number(parts[3])
        max_memory_clock = _first_number(parts[4])
        parts = parts[:3] + parts[5:]
    if len(parts) != _BASE_FIELDS:
        return {"raw": line, "parsed": False}
    reasons = {}
    for name, token in zip(REASON_NAMES, parts[5:10]):
        if token not in ("Active", "Not Active"):
            return {"raw": line, "parsed": False}
        reasons[name] = token == "Active"
    throttle = parts[10]
    if not throttle.startswith("0x"):
        return {"raw": line, "parsed": False}
    try:
        mask = int(throttle, 16)
    except ValueError:
        return {"raw": line, "parsed": False}
    parsed = {
        "parsed": True,
        "temperature_c": _first_number(parts[0]),
        "clock_mhz": _first_number(parts[1]),
        "max_clock_mhz": _first_number(parts[2]),
        "power_w": _first_number(parts[3]),
        "util_pct": _first_number(parts[4]),
        "reasons": reasons,
        "throttle_hex": throttle,
        "throttle_active": mask != 0,
    }
    if extended:
        parsed["memory_clock_mhz"] = memory_clock
        parsed["max_memory_clock_mhz"] = max_memory_clock
    return parsed


def _ends(values):
    if not values:
        return {"min": None, "max": None, "median": None}
    ordered = sorted(values)
    return {"min": ordered[0], "max": ordered[-1], "median": ordered[len(ordered) // 2]}


def summarize_gpu_samples(samples):
    parsed = [sample for sample in samples if sample.get("parsed")]

    def series(key):
        return [sample[key] for sample in parsed if sample.get(key) is not None]

    return {
        "samples": len(samples),
        "parsed": len(parsed),
        "unparsed": len(samples) - len(parsed),
        "clock_mhz": _ends(series("clock_mhz")),
        "memory_clock_mhz": _ends(series("memory_clock_mhz")),
        "util_pct": _ends(series("util_pct")),
        "power_w": _ends(series("power_w")),
        "temperature_c": _ends(series("temperature_c")),
        "reason_counts": {
            name: sum(1 for sample in parsed if sample["reasons"][name])
            for name in REASON_NAMES
        },
        "throttle_samples": sum(1 for sample in parsed if sample["throttle_active"]),
        "max_clock_mhz": next(
            (sample["max_clock_mhz"] for sample in parsed if sample.get("max_clock_mhz") is not None),
            None,
        ),
        "max_memory_clock_mhz": next(
            (sample["max_memory_clock_mhz"] for sample in parsed if sample.get("max_memory_clock_mhz") is not None),
            None,
        ),
    }
