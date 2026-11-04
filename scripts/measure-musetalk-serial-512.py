"""Time the official MuseTalk decode on a 512x288 composite.

Latents still come from the cached 960x540 face crop. The frame, face box,
and jaw mask are scaled together before the paste. A cache miss stops
before face localization. This does not open a device, request Qwen, or
change the reference runner.
"""
import gc
import json
import os
from pathlib import Path
import subprocess
import sys
import threading
import time

ROOT = Path(__file__).resolve().parents[1] / "target-windows" / "realistic-avatar-research"
SOURCE = ROOT / "MuseTalk"
STAGE = ROOT / "offline-reference"
RUNNER = Path(__file__).resolve().parent / "run-musetalk-reference-sample.py"
VIDEO = ROOT / "video-inspection" / "authorized-clip-18s-5s.mp4"
AUDIO = ROOT / "video-inspection" / "qwen-nai-welcome.wav"
OUT = ROOT / "video-inspection" / f"musetalk-serial-512-{time.time_ns()}"
os.environ["HF_HUB_OFFLINE"] = "1"
os.environ["TRANSFORMERS_OFFLINE"] = "1"
os.environ["PYTHONDONTWRITEBYTECODE"] = "1"
os.environ["NUMBA_CACHE_DIR"] = str(ROOT / "temp")
sys.path[:0] = [str(SOURCE), str(SOURCE / "musetalk" / "utils"), str(Path(__file__).resolve().parent)]
os.chdir(STAGE)

import cv2
import numpy as np
import psutil
import torch
import mmcv
import mmengine
import mmpose
import musetalk_reference_cache as cache
import musetalk_reference_masks as masks
from musetalk_composite_scale import scale_box
from musetalk_gpu_sample import (
    GPU_QUERY,
    GPU_QUERY_WITH_MEMORY,
    parse_gpu_sample,
    summarize_gpu_samples,
)
from musetalk_unet_span import cluster_intervals
from musetalk_unet_split import DIAGNOSTIC_BATCHES, device_time_ms
from torch.profiler import DeviceType
from musetalk_timestamp_queue import source_index
from accelerate import init_empty_weights
from diffusers import UNet2DConditionModel
from musetalk.models.unet import PositionalEncoding
from musetalk.models.vae import VAE
from musetalk.utils.audio_processor import AudioProcessor
from musetalk.utils.blending import get_crop_box
from musetalk.utils.face_parsing import FaceParsing
from musetalk.utils.face_parsing.resnet import Resnet18
from PIL import Image
from transformers import WhisperModel

BATCH = 4
STEADY_FRAMES = 100
SRC_W, SRC_H = 960, 540
DST_W, DST_H = 512, 288


def gpu_line():
    try:
        return subprocess.check_output(
            ["nvidia-smi", "--query-gpu=temperature.gpu,clocks.sm,memory.used,utilization.gpu", "--format=csv,noheader"],
            text=True, timeout=5).strip()
    except (OSError, subprocess.SubprocessError) as exc:
        return "unavailable: " + type(exc).__name__


def gpu_query(query=None):
    chosen = GPU_QUERY if query is None else query
    try:
        line = subprocess.check_output(
            ["nvidia-smi", "--query-gpu=" + chosen, "--format=csv,noheader"],
            text=True, timeout=5).strip()
    except (OSError, subprocess.SubprocessError) as exc:
        return {"raw": type(exc).__name__, "parsed": False}
    sample = parse_gpu_sample(line)
    sample["raw"] = line
    return sample


def gpu_apps():
    try:
        text = subprocess.check_output(
            ["nvidia-smi", "--query-compute-apps=pid,used_gpu_memory", "--format=csv,noheader"],
            text=True, timeout=5).strip()
    except (OSError, subprocess.SubprocessError) as exc:
        return "unavailable: " + type(exc).__name__
    return text


class GpuSampler:
    def __init__(self, query=None):
        self.query = GPU_QUERY if query is None else query
        self.samples = []
        self._stop = threading.Event()
        self._thread = None

    def start(self):
        self.samples = []
        self._stop.clear()
        self._thread = threading.Thread(target=self._run, daemon=True)
        self._thread.start()

    def stop(self):
        self._stop.set()
        if self._thread is not None:
            self._thread.join(timeout=3)
        return list(self.samples)

    def _run(self):
        started = time.monotonic()
        while not self._stop.is_set():
            sample = gpu_query(self.query)
            sample["t_ms"] = round((time.monotonic() - started) * 1000, 1)
            self.samples.append(sample)
            if self._stop.wait(0.2):
                return


def identity():
    config_path = SOURCE / "musetalk/utils/dwpose/rtmpose-l_8xb32-270e_coco-ubody-wholebody-384x288.py"
    return {
        "schema": 1,
        "video_sha256": cache.file_sha256(VIDEO),
        "dwpose_sha256": cache.file_sha256(STAGE / "models/dwpose/dw-ll_ucoco_384.pth"),
        "s3fd_sha256": cache.file_sha256(STAGE / "models/s3fd/s3fd.pth"),
        "dwpose_config_sha256": cache.file_sha256(config_path),
        "dwpose_base_sha256": cache.file_sha256(config_path.parent / "default_runtime.py"),
        "detector_source_sha256": cache.file_sha256(SOURCE / "musetalk/utils/face_detection/detection/sfd/sfd_detector.py"),
        "runner_sha256": cache.file_sha256(RUNNER),
        "opencv": cv2.__version__,
        "torch": torch.__version__,
        "mmpose": mmpose.__version__,
        "mmcv": mmcv.__version__,
        "mmengine": mmengine.__version__,
        "frames": 120,
        "size": [960, 540],
    }


def write(payload, directory=None, name="serial-512.json"):
    target = OUT if directory is None else directory
    target.mkdir(parents=True, exist_ok=True)
    with (target / name).open("x", encoding="utf-8") as stream:
        json.dump(payload, stream, ensure_ascii=False, indent=2)
        stream.write("\n")


def scale_composite(image, box, material):
    scaled = cv2.resize(image, (DST_W, DST_H), interpolation=cv2.INTER_AREA)
    if scaled.shape != (DST_H, DST_W, 3):
        raise RuntimeError("unexpected scaled frame")
    new_box = scale_box(box, SRC_W, SRC_H, DST_W, DST_H)
    new_crop, _ = get_crop_box(new_box, 1.5)
    width, height = Image.fromarray(scaled[:, :, ::-1]).crop(tuple(new_crop)).size
    mask, _old_crop = material
    resized = cv2.resize(mask, (width, height), interpolation=cv2.INTER_AREA)
    if resized.dtype != np.uint8:
        resized = np.clip(np.rint(resized), 0, 255).astype(np.uint8)
    if resized.shape != (height, width):
        raise RuntimeError("scaled mask shape")
    return scaled, new_box, (resized, new_crop)


def widths(boxes):
    values = [box[2] - box[0] for box in boxes]
    return {"min": min(values), "max": max(values)}


def main():
    trace = "--unet-trace" in sys.argv
    repeat = "--repeat-512" in sys.argv
    split = "--unet-split" in sys.argv
    span = "--unet-span" in sys.argv
    gaps = "--unet-gaps" in sys.argv
    decode = "--decode-span" in sys.argv
    if sum(1 for flag in (trace, repeat, split, span, gaps, decode) if flag) > 1:
        raise RuntimeError(
            "pass only one of --repeat-512, --unet-trace, --unet-split, --unet-span, --unet-gaps, or --decode-span")
    apps_before_load = gpu_apps() if (trace or split or span or gaps or decode) else None
    if not torch.cuda.is_available():
        raise RuntimeError("CUDA unavailable")
    if not RUNNER.is_file() or not VIDEO.is_file() or not AUDIO.is_file():
        raise RuntimeError("runner, video, or audio is missing")
    boxes = cache.load(ROOT / "video-inspection" / "location-cache", identity(), SRC_W, SRC_H)
    if boxes is None:
        write({"error": "location cache miss", "refused_relocalization": True,
               "cache_rewritten": False, "runner_untouched": True})
        print("location_cache miss; refused relocalization", flush=True)
        return 2
    print("location_cache hit", flush=True)
    capture = cv2.VideoCapture(str(VIDEO))
    frames = []
    for i in range(120):
        ok, frame = capture.read()
        if not ok or frame.shape != (SRC_H, SRC_W, 3):
            raise RuntimeError("unexpected research frame %s" % i)
        frames.append(frame)
    capture.release()
    device = torch.device("cuda")
    torch.manual_seed(2026)
    processor = AudioProcessor(feature_extractor_path="models/whisper")
    whisper = WhisperModel.from_pretrained("models/whisper", local_files_only=True).to(device=device, dtype=torch.float16).eval()
    whisper.requires_grad_(False)
    with torch.inference_mode():
        features, length = processor.get_audio_feature(str(AUDIO))
        chunks = processor.get_whisper_chunk(features, device, torch.float16, whisper, length, fps=30)
    if len(chunks) != 120:
        raise RuntimeError("audio/video frame mismatch")
    print("full_features", len(chunks), flush=True)
    del whisper
    gc.collect()
    torch.cuda.empty_cache()
    vae = VAE(model_path="models/sd-vae", use_float16=True)
    with open("models/musetalkV15/musetalk.json", encoding="utf-8") as handle:
        model_config = json.load(handle)
    with init_empty_weights():
        model = UNet2DConditionModel(**model_config)
    state = torch.load("models/musetalkV15/unet.pth", map_location="cpu", weights_only=True, mmap=True)
    model.load_state_dict(state, strict=True, assign=True)
    del state
    model = model.to(dtype=torch.float16).to(device=device).eval()
    encoding = PositionalEncoding(d_model=384).to(device=device, dtype=torch.float16)
    original_init_weight = Resnet18.init_weight
    try:
        Resnet18.init_weight = lambda self, model_path: None
        parser = FaceParsing(left_cheek_width=90, right_cheek_width=90)
    finally:
        Resnet18.init_weight = original_init_weight
    print("models_loaded", flush=True)
    materials, _ = masks.prepare_all(frames, boxes, parser)
    print("masks_ready", flush=True)
    prepared = []
    with torch.inference_mode():
        for frame, box in zip(frames, boxes):
            x1, y1, x2, y2 = box
            crop = cv2.resize(frame[y1:y2, x1:x2], (256, 256), interpolation=cv2.INTER_LANCZOS4)
            prepared.append(vae.get_latents_for_unet(crop).to(dtype=torch.float16))
    torch.cuda.synchronize()
    print("latents_ready", flush=True)
    small_frames, small_boxes, small_materials = [], [], []
    for frame, box, material in zip(frames, boxes, materials):
        scaled, new_box, new_material = scale_composite(frame, box, material)
        small_frames.append(scaled)
        small_boxes.append(new_box)
        small_materials.append(new_material)
    print("scaled_ready", flush=True)
    record = {"unet": [], "decode": [], "blend": 0.0, "blend_batches": [], "frames": 0, "last_batches": []}

    def indexes_for(batch_index):
        start = batch_index * BATCH
        return [source_index(start + offset, 30, 25, len(frames)) for offset in range(BATCH)]

    def serial_batch(batch_index, target_frames, target_boxes, target_materials, keep):
        chosen = indexes_for(batch_index)
        with torch.inference_mode():
            inputs = torch.cat([prepared[i] for i in chosen])
            audio = torch.cat([chunks[i:i + 1] for i in chosen]).to(device=device, dtype=torch.float16)
            unet_start, unet_end = torch.cuda.Event(True), torch.cuda.Event(True)
            decode_start, decode_end = torch.cuda.Event(True), torch.cuda.Event(True)
            unet_start.record()
            predicted = model(inputs, torch.tensor([0], device=device), encoder_hidden_states=encoding(audio)).sample
            unet_end.record()
            decode_start.record()
            faces = vae.decode_latents(predicted)
            decode_end.record()
        if keep:
            record["unet"].append((unet_start, unet_end))
            record["decode"].append((decode_start, decode_end))
        tick = time.monotonic()
        for index, face in zip(chosen, faces):
            x, y, x1, y1 = target_boxes[index]
            face = cv2.resize(face, (x1 - x, y1 - y), interpolation=cv2.INTER_LANCZOS4)
            masks.blend(target_frames[index], face, target_boxes[index], target_materials[index])
        if keep:
            elapsed = time.monotonic() - tick
            record["blend"] += elapsed
            record["blend_batches"].append(elapsed)
            record["frames"] += len(chosen)

    def event_seconds():
        torch.cuda.synchronize()
        unet_ms = [start.elapsed_time(end) for start, end in record["unet"]]
        decode_ms = [start.elapsed_time(end) for start, end in record["decode"]]
        blend_ms = [item * 1000 for item in record["blend_batches"]]
        if len(unet_ms) == len(decode_ms) == len(blend_ms):
            record["last_batches"] = [
                {"unet_ms": round(unet, 3), "decode_ms": round(decode, 3), "blend_ms": round(blend, 3)}
                for unet, decode, blend in zip(unet_ms, decode_ms, blend_ms)
            ]
        else:
            record["last_batches"] = {
                "error": "batch length mismatch",
                "unet": len(unet_ms),
                "decode": len(decode_ms),
                "blend": len(blend_ms),
            }
        unet = sum(unet_ms) / 1000
        decode = sum(decode_ms) / 1000
        blend_seconds = record["blend"]
        counted = record["frames"]
        record["unet"].clear()
        record["decode"].clear()
        record["blend"] = 0.0
        record["blend_batches"] = []
        record["frames"] = 0
        return unet, decode, blend_seconds, counted

    def run_serial(batches, target_frames, target_boxes, target_materials, keep):
        started = time.monotonic()
        for batch_index in range(batches):
            serial_batch(batch_index, target_frames, target_boxes, target_materials, keep)
        wall = time.monotonic() - started
        if not keep:
            torch.cuda.synchronize()
            record["unet"].clear()
            record["decode"].clear()
            record["blend"] = 0.0
            record["blend_batches"] = []
            record["frames"] = 0
            return wall, None, None, None, 0
        unet, decode, blend_seconds, counted = event_seconds()
        return wall, unet, decode, blend_seconds, counted

    def stage(wall, unet, decode, blend_seconds, counted, label):
        ms = wall * 1000 / counted if counted else None
        return {
            "label": label,
            "frames": counted,
            "wall_seconds": round(wall, 3),
            "ms_per_frame": None if ms is None else round(ms, 2),
            "unet_seconds": round(unet, 3),
            "decode_seconds": round(decode, 3),
            "blend_seconds": round(blend_seconds, 3),
            "steady_bar_ms": 32,
            "steady_bar_met": ms is not None and ms <= 32,
        }

    def counted(label, target_frames, target_boxes, target_materials):
        run_serial(2, target_frames, target_boxes, target_materials, False)
        torch.cuda.synchronize()
        before = gpu_line()
        window = stage(*run_serial(
            STEADY_FRAMES // BATCH, target_frames, target_boxes, target_materials, True), label)
        after = gpu_line()
        return window, before, after

    if trace:
        torch.cuda.synchronize()
        gc.collect()
        torch.cuda.empty_cache()
        torch.cuda.reset_peak_memory_stats()
        sampler = GpuSampler()
        windows = []
        for label in ("512-trace-a", "512-trace-b"):
            run_serial(2, small_frames, small_boxes, small_materials, False)
            torch.cuda.synchronize()
            before = gpu_query()
            sampler.start()
            window = stage(*run_serial(
                STEADY_FRAMES // BATCH, small_frames, small_boxes, small_materials, True), label)
            samples = sampler.stop()
            after = gpu_query()
            batches = record["last_batches"]
            window["batches"] = batches
            window["gpu_before"] = before
            window["gpu_after"] = after
            window["gpu_during"] = samples
            window["gpu_summary"] = summarize_gpu_samples(samples)
            windows.append(window)
        trace_dir = ROOT / "video-inspection" / f"musetalk-serial-512-trace-{time.time_ns()}"
        payload = {
            "location_cache": "hit",
            "cache_rewritten": False,
            "runner_untouched": True,
            "latent_source": "960x540 face crop resized to 256; the 512 canvas does not rebuild latents",
            "order": [item["label"] for item in windows],
            "gpu_query": GPU_QUERY,
            "sample_interval_s": 0.2,
            "compute_apps_before_load": apps_before_load,
            "compute_apps_before_windows": gpu_apps(),
            "windows": windows,
            "steady_bar_ms": 32,
            "pytorch_peak_mib": round(torch.cuda.max_memory_allocated() / 1048576, 1),
            "process_rss_mib": round(psutil.Process().memory_info().rss / 1048576, 1),
            "playback_60s": "not run",
            "scope": "serial 512 batch trace with in-window GPU samples; not public output, OBS, a device playback, or an adopted schedule",
        }
        write(payload, trace_dir, "serial-512-trace.json")
        print("serial_512_trace", json.dumps([
            {
                "label": item["label"],
                "ms": item["ms_per_frame"],
                "unet": item["unet_seconds"],
                "decode": item["decode_seconds"],
                "blend": item["blend_seconds"],
                "clock": item["gpu_summary"]["clock_mhz"],
                "util_max": item["gpu_summary"]["util_pct"]["max"],
                "power_max": item["gpu_summary"]["power_w"]["max"],
                "throttle_samples": item["gpu_summary"]["throttle_samples"],
                "parsed": item["gpu_summary"]["parsed"],
            }
            for item in windows
        ], ensure_ascii=False), flush=True)
        print("serial_512_trace_dir", trace_dir, flush=True)
        return 0

    if split:
        torch.cuda.synchronize()
        gc.collect()
        torch.cuda.empty_cache()
        torch.cuda.reset_peak_memory_stats()
        run_serial(2, small_frames, small_boxes, small_materials, False)
        torch.cuda.synchronize()
        sampler = GpuSampler(GPU_QUERY_WITH_MEMORY)
        before = gpu_query(GPU_QUERY_WITH_MEMORY)
        sampler.start()
        window = stage(*run_serial(
            STEADY_FRAMES // BATCH, small_frames, small_boxes, small_materials, True), "512-split")
        samples = sampler.stop()
        after = gpu_query(GPU_QUERY_WITH_MEMORY)
        window["batches"] = record["last_batches"]
        window["gpu_before"] = before
        window["gpu_after"] = after
        window["gpu_during"] = samples
        window["gpu_summary"] = summarize_gpu_samples(samples)

        def profile_batch(batch_index):
            chosen = indexes_for(batch_index)
            with torch.inference_mode():
                inputs = torch.cat([prepared[i] for i in chosen])
                audio = torch.cat([chunks[i:i + 1] for i in chosen]).to(device=device, dtype=torch.float16)
                unet_start, unet_end = torch.cuda.Event(True), torch.cuda.Event(True)
                with torch.profiler.profile(activities=[torch.profiler.ProfilerActivity.CUDA]) as prof:
                    unet_start.record()
                    predicted = model(
                        inputs,
                        torch.tensor([0], device=device),
                        encoder_hidden_states=encoding(audio),
                    ).sample
                    unet_end.record()
            torch.cuda.synchronize()
            outer_ms = unet_start.elapsed_time(unet_end)
            rows = [
                (event.self_device_time_total, event.count)
                for event in prof.key_averages()
                if event.self_device_time_total
            ]
            timed = device_time_ms(rows)
            del predicted
            return {
                "outer_ms": round(outer_ms, 3),
                "device_ms": round(timed["device_ms"], 3),
                "kernel_calls": timed["kernel_calls"],
            }

        diagnostic = [profile_batch(batch_index) for batch_index in range(DIAGNOSTIC_BATCHES)]
        split_dir = ROOT / "video-inspection" / f"musetalk-serial-512-split-{time.time_ns()}"
        payload = {
            "location_cache": "hit",
            "cache_rewritten": False,
            "runner_untouched": True,
            "latent_source": "960x540 face crop resized to 256; the 512 canvas does not rebuild latents",
            "order": [window["label"]],
            "gpu_query": GPU_QUERY_WITH_MEMORY,
            "sample_interval_s": 0.2,
            "compute_apps_before_load": apps_before_load,
            "compute_apps_before_windows": gpu_apps(),
            "windows": [window],
            "diagnostic": diagnostic,
            "diagnostic_note": "four profiled batches after the counted window; not part of the 32 ms mark",
            "steady_bar_ms": 32,
            "pytorch_peak_mib": round(torch.cuda.max_memory_allocated() / 1048576, 1),
            "process_rss_mib": round(psutil.Process().memory_info().rss / 1048576, 1),
            "playback_60s": "not run",
            "scope": "one counted serial 512 window plus four profiled UNet batches; not public output, OBS, a device playback, or an adopted schedule",
        }
        write(payload, split_dir, "serial-512-split.json")
        print("serial_512_split", json.dumps({
            "ms": window["ms_per_frame"],
            "unet": window["unet_seconds"],
            "decode": window["decode_seconds"],
            "blend": window["blend_seconds"],
            "memory_clock": window["gpu_summary"]["memory_clock_mhz"],
            "diagnostic": diagnostic,
        }, ensure_ascii=False), flush=True)
        print("serial_512_split_dir", split_dir, flush=True)
        return 0

    if span:
        torch.cuda.synchronize()
        gc.collect()
        torch.cuda.empty_cache()
        torch.cuda.reset_peak_memory_stats()
        run_serial(2, small_frames, small_boxes, small_materials, False)
        torch.cuda.synchronize()
        sampler = GpuSampler(GPU_QUERY_WITH_MEMORY)
        before = gpu_query(GPU_QUERY_WITH_MEMORY)
        sampler.start()
        window = stage(*run_serial(
            STEADY_FRAMES // BATCH, small_frames, small_boxes, small_materials, True), "512-span")
        samples = sampler.stop()
        after = gpu_query(GPU_QUERY_WITH_MEMORY)
        window["batches"] = record["last_batches"]
        window["gpu_before"] = before
        window["gpu_after"] = after
        window["gpu_during"] = samples
        window["gpu_summary"] = summarize_gpu_samples(samples)

        def cuda_intervals(prof):
            kineto = prof.profiler.kineto_results
            if kineto is None:
                raise RuntimeError("profiler timeline missing")
            intervals = []
            for event in kineto.events():
                if event.device_type() != DeviceType.CUDA:
                    continue
                duration = event.duration_ns()
                if duration < 0:
                    raise RuntimeError("negative device duration")
                if duration == 0:
                    continue
                intervals.append((event.start_ns(), duration))
            return intervals

        def profile_batch(batch_index):
            chosen = indexes_for(batch_index)
            with torch.inference_mode():
                inputs = torch.cat([prepared[i] for i in chosen])
                audio = torch.cat([chunks[i:i + 1] for i in chosen]).to(device=device, dtype=torch.float16)
                unet_start, unet_end = torch.cuda.Event(True), torch.cuda.Event(True)
                with torch.profiler.profile(activities=[torch.profiler.ProfilerActivity.CUDA]) as prof:
                    unet_start.record()
                    predicted = model(
                        inputs,
                        torch.tensor([0], device=device),
                        encoder_hidden_states=encoding(audio),
                    ).sample
                    unet_end.record()
            torch.cuda.synchronize()
            outer_ms = unet_start.elapsed_time(unet_end)
            rows = [
                (event.self_device_time_total, event.count)
                for event in prof.key_averages()
                if event.self_device_time_total
            ]
            timed = device_time_ms(rows)
            cluster = cluster_intervals(cuda_intervals(prof))
            if cluster["span_ms"] is None:
                raise RuntimeError("device cluster missing")
            del predicted
            return {
                "outer_ms": round(outer_ms, 3),
                "device_ms": round(timed["device_ms"], 3),
                "kernel_calls": timed["kernel_calls"],
                "span_ms": round(cluster["span_ms"], 3),
                "idle_inside_ms": round(cluster["idle_inside_ms"], 3),
                "overlap_ms": round(cluster["overlap_ms"], 3),
                "op_count": cluster["op_count"],
                "kineto_device_ms": round(cluster["device_ms"], 3),
            }

        diagnostic = [profile_batch(batch_index) for batch_index in range(DIAGNOSTIC_BATCHES)]
        span_dir = ROOT / "video-inspection" / f"musetalk-serial-512-span-{time.time_ns()}"
        payload = {
            "location_cache": "hit",
            "cache_rewritten": False,
            "runner_untouched": True,
            "latent_source": "960x540 face crop resized to 256; the 512 canvas does not rebuild latents",
            "order": [window["label"]],
            "gpu_query": GPU_QUERY_WITH_MEMORY,
            "sample_interval_s": 0.2,
            "compute_apps_before_load": apps_before_load,
            "compute_apps_before_windows": gpu_apps(),
            "windows": [window],
            "diagnostic": diagnostic,
            "diagnostic_note": (
                "four profiled batches after the counted window; span, idle, and overlap "
                "are on the profiler device timeline; outer_ms is the CUDA event; "
                "not part of the 32 ms mark"
            ),
            "steady_bar_ms": 32,
            "pytorch_peak_mib": round(torch.cuda.max_memory_allocated() / 1048576, 1),
            "process_rss_mib": round(psutil.Process().memory_info().rss / 1048576, 1),
            "playback_60s": "not run",
            "scope": "one counted serial 512 window plus four profiled device-op spans; not public output, OBS, a device playback, or an adopted schedule",
        }
        write(payload, span_dir, "serial-512-span.json")
        print("serial_512_span", json.dumps({
            "ms": window["ms_per_frame"],
            "unet": window["unet_seconds"],
            "span": [row["span_ms"] for row in diagnostic],
            "idle": [row["idle_inside_ms"] for row in diagnostic],
            "outside": [round(row["outer_ms"] - row["span_ms"], 3) for row in diagnostic],
        }, ensure_ascii=False), flush=True)
        print("serial_512_span_dir", span_dir, flush=True)
        return 0

    if gaps:
        torch.cuda.synchronize()
        gc.collect()
        torch.cuda.empty_cache()
        torch.cuda.reset_peak_memory_stats()
        run_serial(2, small_frames, small_boxes, small_materials, False)
        torch.cuda.synchronize()
        sampler = GpuSampler(GPU_QUERY_WITH_MEMORY)
        before = gpu_query(GPU_QUERY_WITH_MEMORY)
        sampler.start()
        window = stage(*run_serial(
            STEADY_FRAMES // BATCH, small_frames, small_boxes, small_materials, True), "512-gaps")
        samples = sampler.stop()
        after = gpu_query(GPU_QUERY_WITH_MEMORY)
        window["batches"] = record["last_batches"]
        window["gpu_before"] = before
        window["gpu_after"] = after
        window["gpu_during"] = samples
        window["gpu_summary"] = summarize_gpu_samples(samples)

        def cuda_intervals(prof):
            kineto = prof.profiler.kineto_results
            if kineto is None:
                raise RuntimeError("profiler timeline missing")
            intervals = []
            for event in kineto.events():
                if event.device_type() != DeviceType.CUDA:
                    continue
                duration = event.duration_ns()
                if duration < 0:
                    raise RuntimeError("negative device duration")
                if duration == 0:
                    continue
                intervals.append((event.start_ns(), duration))
            return intervals

        def profile_batch(batch_index):
            chosen = indexes_for(batch_index)
            with torch.inference_mode():
                inputs = torch.cat([prepared[i] for i in chosen])
                audio = torch.cat([chunks[i:i + 1] for i in chosen]).to(device=device, dtype=torch.float16)
                unet_start, unet_end = torch.cuda.Event(True), torch.cuda.Event(True)
                with torch.profiler.profile(activities=[torch.profiler.ProfilerActivity.CUDA]) as prof:
                    unet_start.record()
                    predicted = model(
                        inputs,
                        torch.tensor([0], device=device),
                        encoder_hidden_states=encoding(audio),
                    ).sample
                    unet_end.record()
            torch.cuda.synchronize()
            outer_ms = unet_start.elapsed_time(unet_end)
            rows = [
                (event.self_device_time_total, event.count)
                for event in prof.key_averages()
                if event.self_device_time_total
            ]
            timed = device_time_ms(rows)
            cluster = cluster_intervals(cuda_intervals(prof))
            if cluster["span_ms"] is None:
                raise RuntimeError("device cluster missing")
            del predicted
            return {
                "outer_ms": round(outer_ms, 3),
                "device_ms": round(timed["device_ms"], 3),
                "kernel_calls": timed["kernel_calls"],
                "span_ms": round(cluster["span_ms"], 3),
                "idle_inside_ms": round(cluster["idle_inside_ms"], 3),
                "overlap_ms": round(cluster["overlap_ms"], 3),
                "op_count": cluster["op_count"],
                "kineto_device_ms": round(cluster["device_ms"], 3),
                "gap_count": cluster["gap_count"],
                "max_gap_ms": round(cluster["max_gap_ms"], 3),
                "median_gap_ms": round(cluster["median_gap_ms"], 3),
            }

        diagnostic = [profile_batch(batch_index) for batch_index in range(DIAGNOSTIC_BATCHES)]
        gaps_dir = ROOT / "video-inspection" / f"musetalk-serial-512-gaps-{time.time_ns()}"
        payload = {
            "location_cache": "hit",
            "cache_rewritten": False,
            "runner_untouched": True,
            "latent_source": "960x540 face crop resized to 256; the 512 canvas does not rebuild latents",
            "order": [window["label"]],
            "gpu_query": GPU_QUERY_WITH_MEMORY,
            "sample_interval_s": 0.2,
            "compute_apps_before_load": apps_before_load,
            "compute_apps_before_windows": gpu_apps(),
            "windows": [window],
            "diagnostic": diagnostic,
            "diagnostic_note": (
                "four profiled batches after the counted window; gap_count, max_gap_ms, "
                "and median_gap_ms are idle gaps on the profiler device timeline; "
                "not part of the 32 ms mark"
            ),
            "steady_bar_ms": 32,
            "pytorch_peak_mib": round(torch.cuda.max_memory_allocated() / 1048576, 1),
            "process_rss_mib": round(psutil.Process().memory_info().rss / 1048576, 1),
            "playback_60s": "not run",
            "scope": "one counted serial 512 window plus four profiled idle-gap batches; not public output, OBS, a device playback, or an adopted schedule",
        }
        write(payload, gaps_dir, "serial-512-gaps.json")
        print("serial_512_gaps", json.dumps({
            "ms": window["ms_per_frame"],
            "unet": window["unet_seconds"],
            "gap_count": [row["gap_count"] for row in diagnostic],
            "max_gap": [row["max_gap_ms"] for row in diagnostic],
            "median_gap": [row["median_gap_ms"] for row in diagnostic],
        }, ensure_ascii=False), flush=True)
        print("serial_512_gaps_dir", gaps_dir, flush=True)
        return 0

    if decode:
        torch.cuda.synchronize()
        gc.collect()
        torch.cuda.empty_cache()
        torch.cuda.reset_peak_memory_stats()
        run_serial(2, small_frames, small_boxes, small_materials, False)
        torch.cuda.synchronize()
        sampler = GpuSampler(GPU_QUERY_WITH_MEMORY)
        before = gpu_query(GPU_QUERY_WITH_MEMORY)
        sampler.start()
        window = stage(*run_serial(
            STEADY_FRAMES // BATCH, small_frames, small_boxes, small_materials, True), "512-decode")
        samples = sampler.stop()
        after = gpu_query(GPU_QUERY_WITH_MEMORY)
        window["batches"] = record["last_batches"]
        window["gpu_before"] = before
        window["gpu_after"] = after
        window["gpu_during"] = samples
        window["gpu_summary"] = summarize_gpu_samples(samples)

        def cuda_intervals(prof):
            kineto = prof.profiler.kineto_results
            if kineto is None:
                raise RuntimeError("profiler timeline missing")
            intervals = []
            for event in kineto.events():
                if event.device_type() != DeviceType.CUDA:
                    continue
                duration = event.duration_ns()
                if duration < 0:
                    raise RuntimeError("negative device duration")
                if duration == 0:
                    continue
                intervals.append((event.start_ns(), duration))
            return intervals

        def profile_batch(batch_index):
            chosen = indexes_for(batch_index)
            with torch.inference_mode():
                inputs = torch.cat([prepared[i] for i in chosen])
                audio = torch.cat([chunks[i:i + 1] for i in chosen]).to(device=device, dtype=torch.float16)
                predicted = model(
                    inputs,
                    torch.tensor([0], device=device),
                    encoder_hidden_states=encoding(audio),
                ).sample
            torch.cuda.synchronize()
            with torch.inference_mode():
                decode_start, decode_end = torch.cuda.Event(True), torch.cuda.Event(True)
                with torch.profiler.profile(activities=[torch.profiler.ProfilerActivity.CUDA]) as prof:
                    decode_start.record()
                    faces = vae.decode_latents(predicted)
                    decode_end.record()
            torch.cuda.synchronize()
            outer_ms = decode_start.elapsed_time(decode_end)
            rows = [
                (event.self_device_time_total, event.count)
                for event in prof.key_averages()
                if event.self_device_time_total
            ]
            timed = device_time_ms(rows)
            cluster = cluster_intervals(cuda_intervals(prof))
            if cluster["span_ms"] is None:
                raise RuntimeError("device cluster missing")
            del predicted
            del faces
            return {
                "outer_ms": round(outer_ms, 3),
                "device_ms": round(timed["device_ms"], 3),
                "kernel_calls": timed["kernel_calls"],
                "span_ms": round(cluster["span_ms"], 3),
                "idle_inside_ms": round(cluster["idle_inside_ms"], 3),
                "overlap_ms": round(cluster["overlap_ms"], 3),
                "op_count": cluster["op_count"],
                "kineto_device_ms": round(cluster["device_ms"], 3),
                "gap_count": cluster["gap_count"],
                "max_gap_ms": round(cluster["max_gap_ms"], 3),
                "median_gap_ms": round(cluster["median_gap_ms"], 3),
            }

        diagnostic = [profile_batch(batch_index) for batch_index in range(DIAGNOSTIC_BATCHES)]
        decode_dir = ROOT / "video-inspection" / f"musetalk-serial-512-decode-{time.time_ns()}"
        payload = {
            "location_cache": "hit",
            "cache_rewritten": False,
            "runner_untouched": True,
            "latent_source": "960x540 face crop resized to 256; the 512 canvas does not rebuild latents",
            "order": [window["label"]],
            "gpu_query": GPU_QUERY_WITH_MEMORY,
            "sample_interval_s": 0.2,
            "compute_apps_before_load": apps_before_load,
            "compute_apps_before_windows": gpu_apps(),
            "windows": [window],
            "diagnostic": diagnostic,
            "diagnostic_note": (
                "four profiled decode_latents batches after the counted window; "
                "UNet finishes and the device synchronizes before the decode profiler; "
                "gap fields are idle gaps on the profiler device timeline; "
                "not part of the 32 ms mark"
            ),
            "steady_bar_ms": 32,
            "pytorch_peak_mib": round(torch.cuda.max_memory_allocated() / 1048576, 1),
            "process_rss_mib": round(psutil.Process().memory_info().rss / 1048576, 1),
            "playback_60s": "not run",
            "scope": "one counted serial 512 window plus four profiled decode spans; not public output, OBS, a device playback, or an adopted schedule",
        }
        write(payload, decode_dir, "serial-512-decode.json")
        print("serial_512_decode", json.dumps({
            "ms": window["ms_per_frame"],
            "decode": window["decode_seconds"],
            "gap_count": [row["gap_count"] for row in diagnostic],
            "max_gap": [row["max_gap_ms"] for row in diagnostic],
            "median_gap": [row["median_gap_ms"] for row in diagnostic],
        }, ensure_ascii=False), flush=True)
        print("serial_512_decode_dir", decode_dir, flush=True)
        return 0

    if repeat:
        torch.cuda.synchronize()
        gc.collect()
        torch.cuda.empty_cache()
        torch.cuda.reset_peak_memory_stats()
        first, gpu_before_first, gpu_after_first = counted(
            "512-before-960", small_frames, small_boxes, small_materials)
        between, gpu_before_960, gpu_after_960 = counted(
            "960-between", frames, boxes, materials)
        after, gpu_before_after, gpu_after_after = counted(
            "512-after-960", small_frames, small_boxes, small_materials)
        again, gpu_before_again, gpu_after_again = counted(
            "512-repeat", small_frames, small_boxes, small_materials)
        repeat_dir = ROOT / "video-inspection" / f"musetalk-serial-512-repeat-{time.time_ns()}"
        payload = {
            "location_cache": "hit",
            "cache_rewritten": False,
            "runner_untouched": True,
            "latent_source": "960x540 face crop resized to 256; the 512 canvas does not rebuild latents",
            "mask_policy": "jaw mask prepared at 960x540 and resampled with the frame and box",
            "order": ["512-before-960", "960-between", "512-after-960", "512-repeat"],
            "source_face_width_px": widths(boxes),
            "scaled_face_width_px": widths(small_boxes),
            "composite_512_before_960": first,
            "composite_960_between": between,
            "composite_512_after_960": after,
            "composite_512_repeat": again,
            "steady_bar_ms": 32,
            "pytorch_peak_mib": round(torch.cuda.max_memory_allocated() / 1048576, 1),
            "process_rss_mib": round(psutil.Process().memory_info().rss / 1048576, 1),
            "gpu_before_512_first": gpu_before_first,
            "gpu_after_512_first": gpu_after_first,
            "gpu_before_960": gpu_before_960,
            "gpu_after_960": gpu_after_960,
            "gpu_before_512_after": gpu_before_after,
            "gpu_after_512_after": gpu_after_after,
            "gpu_before_512_repeat": gpu_before_again,
            "gpu_after_512_repeat": gpu_after_again,
            "playback_60s": "not run",
            "scope": "repeated serial 512 blends; not public output, OBS, a device playback, or an adopted schedule",
        }
        write(payload, repeat_dir, "serial-512-repeat.json")
        print("serial_512_repeat", json.dumps({
            "ms_before": first["ms_per_frame"],
            "blend_before": first["blend_seconds"],
            "ms_after": after["ms_per_frame"],
            "blend_after": after["blend_seconds"],
            "ms_repeat": again["ms_per_frame"],
            "blend_repeat": again["blend_seconds"],
            "ms_960": between["ms_per_frame"],
            "blend_960": between["blend_seconds"],
            "out": str(repeat_dir),
        }, ensure_ascii=False), flush=True)
        return 0

    run_serial(2, frames, boxes, materials, False)
    torch.cuda.synchronize()
    gc.collect()
    torch.cuda.empty_cache()
    torch.cuda.reset_peak_memory_stats()
    gpu_before = gpu_line()
    full = stage(*run_serial(STEADY_FRAMES // BATCH, frames, boxes, materials, True), "960x540")
    gpu_after_full = gpu_line()
    run_serial(2, small_frames, small_boxes, small_materials, False)
    small = stage(*run_serial(STEADY_FRAMES // BATCH, small_frames, small_boxes, small_materials, True), "512x288")
    gpu_after_small = gpu_line()
    payload = {
        "location_cache": "hit",
        "cache_rewritten": False,
        "runner_untouched": True,
        "latent_source": "960x540 face crop resized to 256; the 512 canvas does not rebuild latents",
        "mask_policy": "jaw mask prepared at 960x540 and resampled with the frame and box",
        "source_face_width_px": widths(boxes),
        "scaled_face_width_px": widths(small_boxes),
        "composite_960": full,
        "composite_512": small,
        "steady_bar_ms": 32,
        "pytorch_peak_mib": round(torch.cuda.max_memory_allocated() / 1048576, 1),
        "process_rss_mib": round(psutil.Process().memory_info().rss / 1048576, 1),
        "gpu_before_960": gpu_before,
        "gpu_after_960": gpu_after_full,
        "gpu_after_512": gpu_after_small,
        "playback_60s": "not run",
        "scope": "serial research composites; not public output, OBS, a device playback, or an adopted overlap schedule",
    }
    write(payload)
    print("serial_512_result", json.dumps({
        "ms_960": full["ms_per_frame"],
        "ms_512": small["ms_per_frame"],
        "blend_960": full["blend_seconds"],
        "blend_512": small["blend_seconds"],
        "bar_960": full["steady_bar_met"],
        "bar_512": small["steady_bar_met"],
    }, ensure_ascii=False), flush=True)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as exc:
        write({"error": type(exc).__name__, "detail": str(exc)[:500], "cache_rewritten": False, "runner_untouched": True})
        raise
