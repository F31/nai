"""Run two fresh serial-512 spans and write one index.

Each child is a new process with --unet-span. The count stays two. The
counted window names the band. The four profiled batches record the device-op
cluster and are not the 32 ms mark. This does not open a device, request
Qwen, or change the reference runner.
"""
import json
import subprocess
import sys
import time
from pathlib import Path

from musetalk_unet_span import index_span_traces

COUNT = 2
CHILD_TIMEOUT_S = 900
ROOT = Path(__file__).resolve().parents[1] / "target-windows" / "realistic-avatar-research"
SCRIPT = Path(__file__).resolve().parent / "measure-musetalk-serial-512.py"


def child_directory(stdout):
    found = None
    for line in stdout.splitlines():
        if line.startswith("serial_512_span_dir "):
            found = Path(line.split(" ", 1)[1].strip())
    return found


def write_partial(directory, payload):
    (directory / "serial-512-unet-span.json").write_text(
        json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def main():
    if not SCRIPT.is_file():
        raise RuntimeError("measure script is missing")
    traces = []
    paths = []
    directory = ROOT / "video-inspection" / f"musetalk-serial-512-unet-span-{time.time_ns()}"
    directory.mkdir(parents=True, exist_ok=True)
    for index in range(1, COUNT + 1):
        print("span_process_start", index, flush=True)
        try:
            completed = subprocess.run(
                [sys.executable, str(SCRIPT), "--unet-span"],
                text=True,
                capture_output=True,
                timeout=CHILD_TIMEOUT_S,
            )
        except subprocess.TimeoutExpired as exc:
            write_partial(directory, {
                "error": "TimeoutExpired",
                "completed_processes": index - 1,
                "paths": [str(path) for path in paths],
                "playback_60s": "not run",
            })
            raise RuntimeError("process " + str(index) + " timed out") from exc
        child = child_directory(completed.stdout)
        if completed.returncode != 0 or child is None:
            write_partial(directory, {
                "error": "child_failed",
                "process": index,
                "exit": completed.returncode,
                "stderr_tail": "\n".join(completed.stderr.splitlines()[-40:]),
                "stdout_tail": "\n".join(completed.stdout.splitlines()[-20:]),
                "paths": [str(path) for path in paths],
                "playback_60s": "not run",
            })
            raise RuntimeError("process " + str(index) + " failed")
        payload = json.loads((child / "serial-512-span.json").read_text(encoding="utf-8"))
        traces.append(payload)
        paths.append(child)
        print("span_process_done", index, str(child), flush=True)
    try:
        indexed = index_span_traces(traces)
    except ValueError as exc:
        write_partial(directory, {
            "error": "index_refused",
            "detail": str(exc),
            "paths": [str(path) for path in paths],
            "playback_60s": "not run",
        })
        raise
    result = {
        "process_count": COUNT,
        "protocol": "two fresh processes; each runs one counted 512 window, then four profiled UNet batches with the device-op span",
        "paths": [str(path) for path in paths],
        "index": indexed,
        "pytorch_peak_mib": [trace["pytorch_peak_mib"] for trace in traces],
        "process_rss_mib": [trace["process_rss_mib"] for trace in traces],
        "playback_60s": "not run",
        "scope": "two serial 512 spans; not public output, OBS, a device playback, or an adopted schedule",
    }
    target = directory / "serial-512-unet-span.json"
    with target.open("x", encoding="utf-8") as stream:
        json.dump(result, stream, ensure_ascii=False, indent=2)
        stream.write("\n")
    print("serial_512_unet_span", json.dumps({
        "unet_seconds": [window["unet_seconds"] for window in indexed["windows"]],
        "out": str(directory),
    }, ensure_ascii=False), flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
