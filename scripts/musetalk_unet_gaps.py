"""Index two counted serial-512 windows and the idle-gap shape after each.

The counted window is the 100-frame measurement. The four diagnostic batches
reuse the device-op cluster and add the number of idle gaps inside the span,
the longest gap, and the upper-middle median of those gaps. Those batches are
not part of the 32 ms mark.
"""
from musetalk_unet_span import _number, index_span_traces


def index_gap_traces(traces):
    indexed = index_span_traces(traces)
    for block, trace in zip(indexed["diagnostics"], traces):
        raw_rows = trace.get("diagnostic")
        if not isinstance(raw_rows, list) or len(raw_rows) != len(block["rows"]):
            raise ValueError("diagnostic row count")
        for index, (summarized, raw) in enumerate(zip(block["rows"], raw_rows), start=1):
            if not isinstance(raw, dict):
                raise ValueError("diagnostic row " + str(index) + " incomplete")
            try:
                gap_count = _number(raw, "gap_count")
                max_gap = _number(raw, "max_gap_ms")
                median_gap = _number(raw, "median_gap_ms")
            except ValueError as exc:
                raise ValueError("diagnostic row " + str(index) + " " + str(exc)) from exc
            if gap_count != int(gap_count):
                raise ValueError("diagnostic row " + str(index) + " gap count")
            gap_count = int(gap_count)
            idle = summarized["idle_inside_ms"]
            if gap_count == 0 and (max_gap != 0 or median_gap != 0):
                raise ValueError("diagnostic row " + str(index) + " empty gaps")
            if max_gap > idle + 0.001:
                raise ValueError("diagnostic row " + str(index) + " max gap exceeds idle")
            if median_gap > max_gap + 0.001:
                raise ValueError("diagnostic row " + str(index) + " median exceeds max")
            summarized["gap_count"] = gap_count
            summarized["max_gap_ms"] = max_gap
            summarized["median_gap_ms"] = median_gap
            summarized["max_over_idle"] = None if idle == 0 else max_gap / idle
    return indexed
