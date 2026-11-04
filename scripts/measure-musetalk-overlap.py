"""Measure overlapped MuseTalk batches on the resident research models.

Does not modify the reference runner and does not rewrite the location cache.
A cache miss stops before face localization. This is not a public renderer,
a 60-second playback, or an OBS result.
"""
import gc
import json
import os
from pathlib import Path
import socket
import sys
import threading
import time
import urllib.request
import wave

ROOT = Path(__file__).resolve().parents[1] / "target-windows" / "realistic-avatar-research"
SOURCE = ROOT / "MuseTalk"
STAGE = ROOT / "offline-reference"
RUNNER = Path(__file__).resolve().parent / "run-musetalk-reference-sample.py"
VIDEO = ROOT / "video-inspection" / "authorized-clip-18s-5s.mp4"
AUDIO = ROOT / "video-inspection" / "qwen-nai-welcome.wav"
OUT = ROOT / "video-inspection" / f"musetalk-overlap-{time.time_ns()}"
os.environ["HF_HUB_OFFLINE"] = "1"
os.environ["TRANSFORMERS_OFFLINE"] = "1"
os.environ["PYTHONDONTWRITEBYTECODE"] = "1"
os.environ["NUMBA_CACHE_DIR"] = str(ROOT / "temp")
sys.path[:0] = [str(SOURCE), str(SOURCE / "musetalk" / "utils"), str(Path(__file__).resolve().parent)]
os.chdir(STAGE)

import cv2
import psutil
import torch
import mmcv
import mmengine
import mmpose
import musetalk_gpu_pack as gpu_pack
import musetalk_reference_cache as cache
import musetalk_reference_masks as masks
from musetalk_overlap import run_pipeline
from musetalk_timestamp_queue import source_index
from accelerate import init_empty_weights
from diffusers import UNet2DConditionModel
from musetalk.models.unet import PositionalEncoding
from musetalk.models.vae import VAE
from musetalk.utils.audio_processor import AudioProcessor
from musetalk.utils.face_parsing import FaceParsing
from musetalk.utils.face_parsing.resnet import Resnet18
from transformers import WhisperModel

BATCH = 4
STEADY_FRAMES = 100
CHUNK_SAMPLES = 9600  # 400 ms at 24 kHz


def gpu_line():
    import subprocess
    try:
        return subprocess.check_output(
            ["nvidia-smi", "--query-gpu=temperature.gpu,clocks.sm,memory.used,utilization.gpu", "--format=csv,noheader"],
            text=True, timeout=5).strip()
    except (OSError, subprocess.SubprocessError) as exc:
        return "unavailable: " + type(exc).__name__


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


def write(payload):
    OUT.mkdir(parents=True, exist_ok=True)
    with (OUT / "overlap.json").open("x", encoding="utf-8") as stream:
        json.dump(payload, stream, ensure_ascii=False, indent=2)
        stream.write("\n")


def qwen_open():
    try:
        with socket.create_connection(("127.0.0.1", 8000), 1):
            return True
    except OSError:
        return False


def qwen_speech(holder):
    body = json.dumps({"input": "测一下同时合成。", "voice": "serena", "language": "chinese"}).encode()
    request = urllib.request.Request(
        "http://127.0.0.1:8000/v1/audio/speech",
        data=body,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    started = time.monotonic()
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    try:
        with opener.open(request, timeout=30) as response:
            payload = response.read(1_048_576)
            holder.update(state="completed", status=response.status, bytes=len(payload))
    except Exception as exc:
        holder.update(state="error", error=type(exc).__name__, detail=str(exc)[:300])
    holder["seconds"] = round(time.monotonic() - started, 3)


def main():
    if not torch.cuda.is_available():
        raise RuntimeError("CUDA unavailable")
    if not RUNNER.is_file() or not VIDEO.is_file() or not AUDIO.is_file():
        raise RuntimeError("runner, video, or audio is missing")
    boxes = cache.load(ROOT / "video-inspection" / "location-cache", identity(), 960, 540)
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
        if not ok or frame.shape != (540, 960, 3):
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
    chunk_wav = OUT / "chunk-400ms.wav"
    OUT.mkdir(parents=True, exist_ok=True)
    with wave.open(str(AUDIO), "rb") as source, wave.open(str(chunk_wav), "wb") as target:
        if source.getnchannels() != 1 or source.getframerate() != 24000 or source.getsampwidth() != 2:
            raise RuntimeError("research audio is not 24 kHz mono s16")
        target.setnchannels(1)
        target.setsampwidth(2)
        target.setframerate(24000)
        target.writeframes(source.readframes(CHUNK_SAMPLES))
    chunk_seconds = None
    feature_delta = None
    short_features = short_chunks = None
    if os.environ.get("MUSE_SKIP_CHUNK") != "1":
        tick = time.monotonic()
        with torch.inference_mode():
            short_features, short_length = processor.get_audio_feature(str(chunk_wav))
            short_chunks = processor.get_whisper_chunk(short_features, device, torch.float16, whisper, short_length, fps=30)
        torch.cuda.synchronize()
        chunk_seconds = time.monotonic() - tick
        if short_chunks is not None and len(short_chunks) >= 1 and tuple(short_chunks[0].shape) == tuple(chunks[0].shape):
            feature_delta = float((short_chunks[0].detach().float().cpu() - chunks[0].detach().float().cpu()).abs().max().item())
        del short_features, short_chunks
    del whisper
    gc.collect()
    torch.cuda.empty_cache()
    shown_chunk = None if chunk_seconds is None else round(chunk_seconds, 3)
    print("chunk_features", shown_chunk, feature_delta, flush=True)
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
    unet_stream = torch.cuda.Stream()
    decode_stream = torch.cuda.Stream()
    record = {"on": False, "unet": [], "decode": [], "blend": 0.0, "frames": 0}

    def indexes_for(batch_index):
        start = batch_index * BATCH
        return [source_index(start + offset, 30, 25, len(frames)) for offset in range(BATCH)]

    def start_unet(batch_index):
        chosen = indexes_for(batch_index)
        with torch.inference_mode(), torch.cuda.stream(unet_stream):
            inputs = torch.cat([prepared[i] for i in chosen])
            audio = torch.cat([chunks[i:i + 1] for i in chosen]).to(device=device, dtype=torch.float16)
            started_event, ended_event = torch.cuda.Event(True), torch.cuda.Event(True)
            started_event.record()
            predicted = model(inputs, torch.tensor([0], device=device), encoder_hidden_states=encoding(audio)).sample
            ended_event.record()
        if record["on"]:
            record["unet"].append((started_event, ended_event))
        return {"predicted": predicted, "ended": ended_event, "indexes": chosen}

    def start_decode(batch_index, pending):
        del batch_index
        with torch.inference_mode(), torch.cuda.stream(decode_stream):
            decode_stream.wait_event(pending["ended"])
            started_event, ended_event = torch.cuda.Event(True), torch.cuda.Event(True)
            started_event.record()
            latents = (1 / vae.scaling_factor) * pending["predicted"]
            image = vae.vae.decode(latents.to(vae.vae.dtype)).sample
            ended_event.record()
        if record["on"]:
            record["decode"].append((started_event, ended_event))
        return {"image": image, "ended": ended_event, "indexes": pending["indexes"]}

    def finish_decode(batch_index, token):
        del batch_index
        token["ended"].synchronize()
        with torch.cuda.stream(decode_stream):
            return gpu_pack.pack(token["image"]), token["indexes"]

    def blend(batch_index, payload):
        del batch_index
        faces, chosen = payload
        tick = time.monotonic()
        for index, face in zip(chosen, faces):
            x, y, x1, y1 = boxes[index]
            face = cv2.resize(face, (x1 - x, y1 - y), interpolation=cv2.INTER_LANCZOS4)
            masks.blend(frames[index], face, boxes[index], materials[index])
            record["frames"] += 1
        if record["on"]:
            record["blend"] += time.monotonic() - tick

    def event_seconds():
        torch.cuda.synchronize()
        unet = sum(start.elapsed_time(end) for start, end in record["unet"]) / 1000
        decode = sum(start.elapsed_time(end) for start, end in record["decode"]) / 1000
        blend_seconds = record["blend"]
        frames = record["frames"]
        record["unet"].clear()
        record["decode"].clear()
        record["blend"] = 0.0
        record["frames"] = 0
        return unet, decode, blend_seconds, frames

    def timed(batches):
        record["on"] = True
        started = time.monotonic()
        run_pipeline(batches, start_unet, start_decode, finish_decode, blend)
        wall = time.monotonic() - started
        record["on"] = False
        unet, decode, blend_seconds, frames = event_seconds()
        return wall, unet, decode, blend_seconds, frames

    run_pipeline(2, start_unet, start_decode, finish_decode, blend)
    torch.cuda.synchronize()
    record["frames"] = 0
    gc.collect()
    torch.cuda.empty_cache()
    torch.cuda.reset_peak_memory_stats()
    prefill_wall, _, _, _, prefill_frames = timed(2)

    def serial_batch(batch_index):
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
        record["unet"].append((unet_start, unet_end))
        record["decode"].append((decode_start, decode_end))
        tick = time.monotonic()
        for index, face in zip(chosen, faces):
            x, y, x1, y1 = boxes[index]
            face = cv2.resize(face, (x1 - x, y1 - y), interpolation=cv2.INTER_LANCZOS4)
            masks.blend(frames[index], face, boxes[index], materials[index])
        record["blend"] += time.monotonic() - tick
        record["frames"] += len(chosen)

    def serial_timed(batches):
        started = time.monotonic()
        for batch_index in range(batches):
            serial_batch(batch_index)
        wall = time.monotonic() - started
        unet, decode, blend_seconds, frames = event_seconds()
        return wall, unet, decode, blend_seconds, frames

    serial_wall, serial_unet, serial_decode, serial_blend, serial_frames = serial_timed(STEADY_FRAMES // BATCH)
    gpu_before = gpu_line()
    steady_wall, unet_s, decode_s, blend_s, steady_frames = timed(STEADY_FRAMES // BATCH)
    gpu_after = gpu_line()
    qwen_holder = {"state": "not_started"}
    concurrent = None
    if os.environ.get("MUSE_SKIP_QWEN") == "1":
        qwen_holder["state"] = "skipped"
    elif qwen_open():
        qwen_holder["state"] = "started"
        thread = threading.Thread(target=qwen_speech, args=(qwen_holder,), daemon=True)
        thread.start()
        concurrent_wall, concurrent_unet, concurrent_decode, concurrent_blend, concurrent_frames = timed(STEADY_FRAMES // BATCH)
        thread.join(timeout=30)
        qwen_holder["joined"] = not thread.is_alive()
        concurrent = {
            "frames": concurrent_frames,
            "wall_seconds": round(concurrent_wall, 3),
            "ms_per_frame": round(concurrent_wall * 1000 / concurrent_frames, 2) if concurrent_frames else None,
            "unet_seconds": round(concurrent_unet, 3),
            "decode_seconds": round(concurrent_decode, 3),
            "blend_seconds": round(concurrent_blend, 3),
        }
    else:
        qwen_holder["state"] = "unreachable"
    ms_per_frame = steady_wall * 1000 / steady_frames if steady_frames else None
    serial = unet_s + decode_s + blend_s
    payload = {
        "location_cache": "hit",
        "cache_rewritten": False,
        "runner_untouched": True,
        "chunk_ms": 400,
        "chunk_feature_seconds": None if chunk_seconds is None else round(chunk_seconds, 3),
        "chunk_matches_full_file_first_frame": feature_delta == 0,
        "chunk_first_frame_max_abs_diff": feature_delta,
        "prefill_frames": prefill_frames,
        "prefill_seconds": round(prefill_wall, 3),
        "chunk_plus_prefill_seconds": None if chunk_seconds is None else round(chunk_seconds + prefill_wall, 3),
        "steady_frames": steady_frames,
        "steady_wall_seconds": round(steady_wall, 3),
        "steady_ms_per_frame": None if ms_per_frame is None else round(ms_per_frame, 2),
        "unet_seconds": round(unet_s, 3),
        "decode_seconds": round(decode_s, 3),
        "blend_seconds": round(blend_s, 3),
        "serial_stage_seconds": round(serial, 3),
        "overlap_saved_seconds": round(serial - steady_wall, 3),
        "steady_bar_ms": 32,
        "steady_bar_met": ms_per_frame is not None and ms_per_frame <= 32,
        "serial_frames": serial_frames,
        "serial_wall_seconds": round(serial_wall, 3),
        "serial_ms_per_frame": round(serial_wall * 1000 / serial_frames, 2) if serial_frames else None,
        "serial_unet_seconds": round(serial_unet, 3),
        "serial_decode_seconds": round(serial_decode, 3),
        "serial_blend_seconds": round(serial_blend, 3),
        "first_frame_bar_seconds": 0.8,
        "first_frame_bar_met": chunk_seconds is not None and chunk_seconds + prefill_wall <= 0.8,
        "pytorch_peak_mib": round(torch.cuda.max_memory_allocated() / 1048576, 1),
        "process_rss_mib": round(psutil.Process().memory_info().rss / 1048576, 1),
        "gpu_before_steady": gpu_before,
        "gpu_after_steady": gpu_after,
        "qwen": qwen_holder,
        "steady_while_qwen": concurrent,
        "playback_60s": "not run; the device bridge check still expects the 4.0 s clip",
        "scope": "overlapped research batches on the cached identity; not public output, OBS, or a device playback",
    }
    write(payload)
    print("overlap_result", json.dumps({
        "ms_per_frame": payload["steady_ms_per_frame"],
        "serial_ms": payload["serial_ms_per_frame"],
        "saved": payload["overlap_saved_seconds"],
        "chunk_plus_prefill": payload["chunk_plus_prefill_seconds"],
        "chunk_diff": feature_delta,
        "qwen": qwen_holder.get("state"),
        "while_qwen_ms": None if concurrent is None else concurrent["ms_per_frame"],
        "steady_bar": payload["steady_bar_met"],
        "first_bar": payload["first_frame_bar_met"],
    }, ensure_ascii=False), flush=True)
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as exc:
        write({"error": type(exc).__name__, "detail": str(exc)[:500], "cache_rewritten": False, "runner_untouched": True})
        raise
