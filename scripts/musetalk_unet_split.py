"""Index two counted serial-512 windows and the diagnostic UNet rows after each.

The counted window is the 100-frame measurement. The four diagnostic batches
record the same UNet forward twice: the outer CUDA event, and the sum of
profiler device time. Those batches are not part of the 32 ms mark.
"""

COUNTED_BATCHES = 25
DIAGNOSTIC_BATCHES = 4


def device_time_ms(rows):
    """Sum profiler rows. Each row is (self_device_time_total microseconds, count)."""
    total = 0.0
    calls = 0
    for microseconds, count in rows:
        total += float(microseconds)
        calls += int(count)
    return {"device_ms": total / 1000.0, "kernel_calls": calls}


def summarize_diagnostic(rows):
    if not isinstance(rows, list) or len(rows) != DIAGNOSTIC_BATCHES:
        found = len(rows) if isinstance(rows, list) else type(rows).__name__
        raise ValueError("diagnostic row count " + str(found))
    summarized = []
    for index, row in enumerate(rows, start=1):
        if not isinstance(row, dict):
            raise ValueError("diagnostic row " + str(index) + " incomplete")
        outer = row.get("outer_ms")
        device = row.get("device_ms")
        calls = row.get("kernel_calls")
        if outer is None or device is None or calls is None:
            raise ValueError("diagnostic row " + str(index) + " incomplete")
        if outer < 0 or device < 0 or calls < 0:
            raise ValueError("diagnostic row " + str(index) + " negative")
        summarized.append({
            "outer_ms": outer,
            "device_ms": device,
            "kernel_calls": calls,
            "device_over_outer": None if outer == 0 else device / outer,
        })
    return summarized


def index_split_traces(traces):
    if len(traces) != 2:
        raise ValueError("process count " + str(len(traces)))
    windows = []
    diagnostics = []
    for process, trace in enumerate(traces, start=1):
        if trace.get("location_cache") != "hit":
            raise ValueError("process " + str(process) + " cache miss")
        if trace.get("cache_rewritten") or not trace.get("runner_untouched"):
            raise ValueError("process " + str(process) + " cache or runner changed")
        found = trace.get("windows")
        if not isinstance(found, list) or len(found) != 1:
            raise ValueError("process " + str(process) + " window count")
        window = found[0]
        summary = window["gpu_summary"]
        batches = window["batches"]
        if summary.get("parsed") != summary.get("samples") or summary.get("samples", 0) < 1:
            raise ValueError("process " + str(process) + " unparsed samples")
        memory = summary.get("memory_clock_mhz") or {}
        if memory.get("median") is None:
            raise ValueError("process " + str(process) + " memory clock missing")
        if not isinstance(batches, list) or len(batches) != COUNTED_BATCHES:
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
            "memory_clock_mhz": memory,
            "max_memory_clock_mhz": summary.get("max_memory_clock_mhz"),
            "power_w": summary["power_w"],
            "util_pct": summary["util_pct"],
            "temperature_c": summary["temperature_c"],
            "reason_counts": summary["reason_counts"],
            "max_clock_mhz": summary["max_clock_mhz"],
        })
        diagnostics.append({
            "process": process,
            "rows": summarize_diagnostic(trace.get("diagnostic")),
        })
    return {
        "process_count": len(traces),
        "window_count": len(windows),
        "diagnostic_batches": DIAGNOSTIC_BATCHES,
        "windows": windows,
        "diagnostics": diagnostics,
    }
