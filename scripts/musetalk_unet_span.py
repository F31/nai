"""Index two counted serial-512 windows and the device-op cluster after each.

The counted window is the 100-frame measurement. The four diagnostic batches
record one UNet forward on the profiler timeline: the span from the earliest
device-op start to the latest end, the sum of those durations, the idle time
inside that span, and the overlap. The outer CUDA event stays a separate
number. Those batches are not part of the 32 ms mark.
"""

COUNTED_BATCHES = 25
DIAGNOSTIC_BATCHES = 4


def cluster_intervals(intervals):
    """Collapse (start_ns, duration_ns) pairs onto one timeline.

    A zero duration does not extend the span. Overlapping ops add to the
    overlap and not to the idle time. An empty list has no span.
    """
    parsed = []
    for start_ns, duration_ns in intervals:
        duration = int(duration_ns)
        if duration < 0:
            raise ValueError("negative device duration")
        if duration == 0:
            continue
        parsed.append((int(start_ns), duration))
    if not parsed:
        return {
            "op_count": 0,
            "device_ms": 0.0,
            "span_ms": None,
            "idle_inside_ms": None,
            "overlap_ms": None,
            "gap_count": 0,
            "max_gap_ms": None,
            "median_gap_ms": None,
        }
    order = sorted(parsed)
    device_ns = sum(duration for _, duration in order)
    min_start = order[0][0]
    max_end = max(start + duration for start, duration in order)
    span_ns = max_end - min_start
    union_ns = 0
    gaps = []
    current_start, current_duration = order[0]
    current_end = current_start + current_duration
    for start, duration in order[1:]:
        end = start + duration
        if start <= current_end:
            current_end = max(current_end, end)
        else:
            union_ns += current_end - current_start
            gaps.append(start - current_end)
            current_start, current_end = start, end
    union_ns += current_end - current_start
    idle_ns = span_ns - union_ns
    overlap_ns = device_ns - union_ns
    if idle_ns < 0 or overlap_ns < 0 or span_ns <= 0:
        raise ValueError("device cluster is inconsistent")
    if gaps:
        ordered = sorted(gaps)
        max_gap_ms = ordered[-1] / 1_000_000.0
        median_gap_ms = ordered[len(ordered) // 2] / 1_000_000.0
    else:
        max_gap_ms = 0.0
        median_gap_ms = 0.0
    return {
        "op_count": len(order),
        "device_ms": device_ns / 1_000_000.0,
        "span_ms": span_ns / 1_000_000.0,
        "idle_inside_ms": idle_ns / 1_000_000.0,
        "overlap_ms": overlap_ns / 1_000_000.0,
        "gap_count": len(gaps),
        "max_gap_ms": max_gap_ms,
        "median_gap_ms": median_gap_ms,
    }


def _number(row, name):
    value = row.get(name)
    if value is None or isinstance(value, bool):
        raise ValueError("diagnostic row incomplete")
    if value < 0:
        raise ValueError("diagnostic row negative")
    return value


def summarize_span_rows(rows):
    if not isinstance(rows, list) or len(rows) != DIAGNOSTIC_BATCHES:
        found = len(rows) if isinstance(rows, list) else type(rows).__name__
        raise ValueError("diagnostic row count " + str(found))
    summarized = []
    for index, row in enumerate(rows, start=1):
        if not isinstance(row, dict):
            raise ValueError("diagnostic row " + str(index) + " incomplete")
        try:
            outer = _number(row, "outer_ms")
            device = _number(row, "device_ms")
            calls = _number(row, "kernel_calls")
            span = _number(row, "span_ms")
            idle = _number(row, "idle_inside_ms")
            overlap = _number(row, "overlap_ms")
            op_count = _number(row, "op_count")
            kineto = _number(row, "kineto_device_ms")
        except ValueError as exc:
            raise ValueError("diagnostic row " + str(index) + " " + str(exc)) from exc
        if span <= 0 or op_count < 1:
            raise ValueError("diagnostic row " + str(index) + " cluster missing")
        summarized.append({
            "outer_ms": outer,
            "device_ms": device,
            "kernel_calls": calls,
            "span_ms": span,
            "idle_inside_ms": idle,
            "overlap_ms": overlap,
            "op_count": op_count,
            "kineto_device_ms": kineto,
            "outside_ms": outer - span,
            "device_over_outer": None if outer == 0 else device / outer,
        })
    return summarized


def index_span_traces(traces):
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
            "rows": summarize_span_rows(trace.get("diagnostic")),
        })
    return {
        "process_count": len(traces),
        "window_count": len(windows),
        "diagnostic_batches": DIAGNOSTIC_BATCHES,
        "windows": windows,
        "diagnostics": diagnostics,
    }
