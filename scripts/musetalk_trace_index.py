"""Index three cold serial-512 traces.

The 2 second cut only separates two bands already measured on this machine:
UNet events at or below 1.538 seconds, and UNet events from 2.336 seconds upward.
It is not a playback bar and it does not change the 32 ms research line.
"""

SLOW_UNET_SECONDS = 2.0
WINDOWS_PER_PROCESS = 2
BATCHES_PER_WINDOW = 25


def index_traces(traces):
    if len(traces) != 3:
        raise ValueError("process count " + str(len(traces)))
    windows = []
    for process, trace in enumerate(traces, start=1):
        if trace.get("location_cache") != "hit":
            raise ValueError("process " + str(process) + " cache miss")
        if trace.get("cache_rewritten") or not trace.get("runner_untouched"):
            raise ValueError("process " + str(process) + " cache or runner changed")
        found = trace.get("windows")
        if not isinstance(found, list) or len(found) != WINDOWS_PER_PROCESS:
            raise ValueError("process " + str(process) + " window count")
        for window in found:
            summary = window["gpu_summary"]
            batches = window["batches"]
            if summary["parsed"] != summary["samples"] or summary["samples"] < 1:
                raise ValueError("process " + str(process) + " unparsed samples")
            if len(batches) != BATCHES_PER_WINDOW:
                raise ValueError("process " + str(process) + " batch count")
            unet_ms = [batch["unet_ms"] for batch in batches]
            windows.append({
                "process": process,
                "label": window["label"],
                "wall_seconds": window["wall_seconds"],
                "ms_per_frame": window["ms_per_frame"],
                "unet_seconds": window["unet_seconds"],
                "decode_seconds": window["decode_seconds"],
                "blend_seconds": window["blend_seconds"],
                "steady_bar_met": window["steady_bar_met"],
                "unet_batch_ms_min": min(unet_ms),
                "unet_batch_ms_max": max(unet_ms),
                "samples": summary["samples"],
                "parsed": summary["parsed"],
                "clock_mhz": summary["clock_mhz"],
                "power_w": summary["power_w"],
                "util_pct": summary["util_pct"],
                "temperature_c": summary["temperature_c"],
                "reason_counts": summary["reason_counts"],
                "max_clock_mhz": summary["max_clock_mhz"],
            })
    return {
        "process_count": len(traces),
        "window_count": len(windows),
        "slow_unet_seconds": SLOW_UNET_SECONDS,
        "windows_at_or_above_slow_cut": sum(
            1 for window in windows if window["unet_seconds"] >= SLOW_UNET_SECONDS
        ),
        "unet_seconds": [window["unet_seconds"] for window in windows],
        "windows": windows,
    }
