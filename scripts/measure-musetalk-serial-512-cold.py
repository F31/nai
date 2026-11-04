"""Run three fresh serial-512 traces and write one index.

Each child is a new process with --unet-trace. The count stays three whether
or not a window lands in the previously seen slow UNet band. This does not
open a device, request Qwen, or change the reference runner.
"""
import json
import subprocess
import sys
import time
from pathlib import Path

from musetalk_trace_index import index_traces

COUNT = 3
CHILD_TIMEOUT_S = 900
ROOT = Path(__file__).resolve().parents[1] / "target-windows" / "realistic-avatar-research"
SCRIPT = Path(__file__).resolve().parent / "measure-musetalk-serial-512.py"


def child_directory(stdout):
    found = None
    for line in stdout.splitlines():
        if line.startswith("serial_512_trace_dir "):
            found = Path(line.split(" ", 1)[1].strip())
    return found


def main():
    if not SCRIPT.is_file():
        raise RuntimeError("measure script is missing")
    traces = []
    paths = []
    directory = ROOT / "video-inspection" / f"musetalk-serial-512-cold-{time.time_ns()}"
    directory.mkdir(parents=True, exist_ok=True)
    for index in range(1, COUNT + 1):
        print("cold_process_start", index, flush=True)
        try:
            completed = subprocess.run(
                [sys.executable, str(SCRIPT), "--unet-trace"],
                text=True,
                capture_output=True,
                timeout=CHILD_TIMEOUT_S,
            )
        except subprocess.TimeoutExpired as exc:
            partial = {
                "error": "TimeoutExpired",
                "completed_processes": index - 1,
                "paths": [str(path) for path in paths],
                "playback_60s": "not run",
            }
            (directory / "serial-512-cold.json").write_text(
                json.dumps(partial, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
            raise RuntimeError("process " + str(index) + " timed out") from exc
        child = child_directory(completed.stdout)
        if completed.returncode != 0 or child is None:
            partial = {
                "error": "child_failed",
                "process": index,
                "exit": completed.returncode,
                "stderr_tail": "\n".join(completed.stderr.splitlines()[-30:]),
                "paths": [str(path) for path in paths],
                "playback_60s": "not run",
            }
            (directory / "serial-512-cold.json").write_text(
                json.dumps(partial, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
            raise RuntimeError("process " + str(index) + " failed")
        payload = json.loads((child / "serial-512-trace.json").read_text(encoding="utf-8"))
        traces.append(payload)
        paths.append(child)
        print("cold_process_done", index, str(child), flush=True)
    indexed = index_traces(traces)
    result = {
        "process_count": COUNT,
        "protocol": "three fresh processes; each runs two counted 512 windows with in-window GPU samples",
        "paths": [str(path) for path in paths],
        "index": indexed,
        "pytorch_peak_mib": [trace["pytorch_peak_mib"] for trace in traces],
        "process_rss_mib": [trace["process_rss_mib"] for trace in traces],
        "playback_60s": "not run",
        "scope": "three cold serial 512 traces; not public output, OBS, a device playback, or an adopted schedule",
    }
    target = directory / "serial-512-cold.json"
    with target.open("x", encoding="utf-8") as stream:
        json.dump(result, stream, ensure_ascii=False, indent=2)
        stream.write("\n")
    print("serial_512_cold", json.dumps({
        "unet_seconds": indexed["unet_seconds"],
        "windows_at_or_above_slow_cut": indexed["windows_at_or_above_slow_cut"],
        "out": str(directory),
    }, ensure_ascii=False), flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
