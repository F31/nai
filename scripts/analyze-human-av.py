"""Offline evidence inspection, never part of the Windows runtime or model path."""
import array
import json
import math
import subprocess
import sys
from pathlib import Path

base = Path(sys.argv[1]).resolve()
soak_only = "--soak-only" in sys.argv[2:]
clock = json.loads((base / "recording-clock.json").read_text(encoding="utf-8-sig"))
video = base / "recordings" / clock["recording"]
events = [json.loads(line) for line in (base / "timeline.jsonl").read_text(encoding="utf-8-sig").splitlines()]
def phase(name):
    return next(e for e in events if e["phase"] == name)
def at(name):
    return (phase(name)["unix_ms"] - clock.get("recording_log_started_unix_ms", clock["recording_file_created_unix_ms"])) / 1000
def run(args):
    return subprocess.check_output(args)

probe = json.loads(run(["ffprobe", "-v", "error", "-show_streams", "-show_format", "-of", "json", str(video)]))
(base / "ffprobe.json").write_text(json.dumps(probe, indent=2))
audio = [s for s in probe["streams"] if s["codec_type"] == "audio"]
assert len(audio) == 3, "mixed, AI and human tracks must be independently inspectable"
assert all(s["sample_rate"] == "48000" for s in audio)
intervals = {} if soak_only else {
    "human_takeover": (at("human_active") + .25, at("human_muted") - .25),
    "resumed_ai": (at("new_auto_grant"), at("completed")),
}
soak_end = next((e for e in events if e["phase"] == "soak_completed"), None)
last_soak = soak_end or [e for e in events if e["phase"] == "soak_tick"][-1]
known_end = (last_soak["unix_ms"] - clock.get("recording_log_started_unix_ms", clock["recording_file_created_unix_ms"])) / 1000
intervals["human_soak"] = (at("soak_human_started") + .25, known_end - .25)
intervals["after_human_stop"] = (known_end + .5, float(probe["format"]["duration"]) - .25) if soak_end else (0, 0)
results = {}
audio_end = max(stop for _, stop in intervals.values())
for track, label in enumerate(["mixed", "ai", "human"]):
    p = subprocess.Popen(["ffmpeg", "-v", "error", "-i", str(video), "-t", str(audio_end), "-map", f"0:a:{track}", "-ac", "1", "-ar", "48000", "-f", "f32le", "-"], stdout=subprocess.PIPE)
    index = 0
    stats = {name: {"blocks": 0, "active_blocks": 0, "square_sum": 0., "peak": 0., "strongest_s": 0., "strongest_rms": 0.} for name in intervals}
    while True:
        data = p.stdout.read(480 * 4)
        if not data:
            break
        assert len(data) % 4 == 0
        samples = array.array("f", data)
        if sys.byteorder != "little":
            samples.byteswap()
        square = sum(v * v for v in samples) / len(samples)
        rms = math.sqrt(square)
        peak = max(map(abs, samples))
        t = index / 100
        for name, (start, stop) in intervals.items():
            if start <= t < stop:
                s = stats[name]
                s["blocks"] += 1
                s["active_blocks"] += int(rms > .00316)
                s["square_sum"] += square
                s["peak"] = max(s["peak"], peak)
                if rms > s["strongest_rms"]:
                    s["strongest_rms"] = rms
                    s["strongest_s"] = t
        index += 1
    assert p.wait() == 0
    for s in stats.values():
        s["rms"] = math.sqrt(s.pop("square_sum") / max(s["blocks"], 1))
    results[label] = stats

times = json.loads(run(["ffprobe", "-v", "error", "-select_streams", "v:0", "-show_frames", "-show_entries", "frame=best_effort_timestamp_time", "-of", "json", str(video)]))["frames"]
pts = [float(f["best_effort_timestamp_time"]) for f in times]
gaps = [b - a for a, b in zip(pts, pts[1:])]
assert all(g > 0 for g in gaps), "nonmonotonic video timestamps"
active_start, active_end = intervals["human_soak"]
active_gaps = [b - a for a, b in zip(pts, pts[1:]) if a >= active_start and b <= active_end]
outside_gaps = [{"from_s": a, "to_s": b, "gap_s": b - a} for a, b in zip(pts, pts[1:]) if b - a > .068 and not (a >= active_start and b <= active_end)]

# Decode only the speech segment at native frame cadence; do not infer content from metadata.
p = subprocess.Popen(["ffmpeg", "-v", "error", "-i", str(video), "-t", "1" if soak_only else "80", "-an", "-vsync", "0", "-pix_fmt", "gray", "-f", "rawvideo", "-"], stdout=subprocess.PIPE)
size = 512 * 288
caption, mouth = [], []
baseline = None
i = 0
while True:
    data = p.stdout.read(size)
    if not data:
        break
    assert len(data) == size, "partial decoded frame"
    patch = bytes(data[y * 512 + x] for y in range(125, 145) for x in range(241, 272))
    if baseline is None:
        baseline = patch
    whites = sum(data[y * 512 + x] > 215 for y in range(250, 284) for x in range(12, 130))
    diff = sum(abs(a - b) for a, b in zip(patch, baseline)) / len(patch)
    if whites > 35:
        caption.append(pts[i])
    if diff > .5:
        mouth.append(pts[i])
    i += 1
assert p.wait() == 0
resumed_start, resumed_end = intervals.get("resumed_ai", (0, 0))
checks = {} if soak_only else {
    "resumed_ai_has_recorded_audio": results["ai"]["resumed_ai"]["active_blocks"] > 5,
    "human_has_recorded_audio": results["human"]["human_soak"]["peak"] > .001,
    "ai_silent_during_human": results["ai"]["human_takeover"]["active_blocks"] == 0 and results["ai"]["human_soak"]["active_blocks"] == 0,
    "human_silent_during_resumed_ai": results["human"]["resumed_ai"]["active_blocks"] == 0,
    "resumed_caption_observed": any(resumed_start <= t <= resumed_end for t in caption),
    "resumed_mouth_changes_observed": any(resumed_start <= t <= resumed_end for t in mouth),
}
checks.update({
    "human_has_recorded_audio": results["human"]["human_soak"]["peak"] > .001,
    "human_stops_in_recording": bool(soak_end) and results["human"]["after_human_stop"]["blocks"] > 50 and results["human"]["after_human_stop"]["active_blocks"] == 0,
    "active_soak_video_pts_continuous": bool(active_gaps) and max(active_gaps) <= .068,
    "soak_at_least_600_seconds": bool(soak_end) and soak_end["unix_ms"] - phase("soak_human_started")["unix_ms"] >= 600000,
    "ai_silent_during_soak": results["ai"]["human_soak"]["active_blocks"] == 0,
})
tasks = {t["id"] for e in events if e["phase"] == "auto_tick" for t in e["tasks"] if t["id"]}
checks["bounded_three_tasks"] = len(tasks) <= 3
summary = {"recording": str(video), "duration_s": float(probe["format"]["duration"]), "video_frames": len(pts), "max_video_gap_s": max(gaps), "active_soak_max_video_gap_s": max(active_gaps) if active_gaps else None, "whole_file_strict_video_continuity":max(gaps)<=.068, "outside_soak_video_gaps":outside_gaps, "intervals_s": intervals, "tracks": results, "model_tasks": len(tasks), "checks": checks, "passed": all(checks.values()), "restoration_passed":None if soak_only else all(checks[k] for k in ["resumed_ai_has_recorded_audio","ai_silent_during_human","human_silent_during_resumed_ai","resumed_caption_observed","resumed_mouth_changes_observed"]), "soak_completed":bool(soak_end), "clock_alignment": "OBS recording-start log versus native wall clock; not hardware timing", "subjective_listening": "requires human confirmation", "scope": "ten-minute human audio plus public standby; short real AI takeover/resume in separate evidence; not 24-hour continuous narration"}
(base / "recording-analysis.json").write_text(json.dumps(summary, indent=2))
speaking = next(t for t in caption if resumed_start <= t <= resumed_end) + .25 if checks.get("resumed_caption_observed") else resumed_start
images = [("human-standby", at("soak_human_started") + 2), ("stopped" if soak_end else "after-interruption", known_end + 1)]
if not soak_only:
    images.insert(0, ("resumed-speaking", speaking))
for label, t in images:
    subprocess.run(["ffmpeg", "-y", "-v", "error", "-ss", str(t), "-i", str(video), "-frames:v", "1", str(base / (label + ".png"))], check=True)
if not soak_only:
    subprocess.run(["ffmpeg", "-y", "-v", "error", "-ss", str(max(0, resumed_start - .5)), "-i", str(video), "-t", str(resumed_end - resumed_start + 1), "-map", "0:v:0", "-map", "0:a:1", "-c:v", "libx264", "-preset", "fast", "-crf", "18", "-c:a", "aac", "-movflags", "+faststart", str(base / "resumed-ai-preview.mp4")], check=True)
print(json.dumps({"passed": summary["passed"], "checks": checks, "duration_s": summary["duration_s"], "model_tasks": len(tasks)}))
sys.exit(0 if summary["passed"] else 1)
