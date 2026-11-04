"""Offline, bounded MuseTalk reference experiment; not a production renderer.

Requires the hash-checked offline-reference tree produced by
stage-musetalk-reference.py and the explicitly authorized local clip/audio.
Writes only a fresh research result directory; never starts public output.
"""
import argparse
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
OUT = ROOT / "video-inspection" / f"musetalk-sample-{time.time_ns()}"
VIDEO = ROOT / "video-inspection" / "authorized-clip-18s-5s.mp4"
AUDIO = ROOT / "video-inspection" / "qwen-nai-welcome.wav"
FFMPEG = ROOT / "venv" / "Lib" / "site-packages" / "imageio_ffmpeg" / "binaries" / "ffmpeg-win-x86_64-v7.1.exe"
os.environ["HF_HUB_OFFLINE"] = "1"
os.environ["TRANSFORMERS_OFFLINE"] = "1"
os.environ["PYTHONDONTWRITEBYTECODE"] = "1"
os.environ["NUMBA_CACHE_DIR"] = str(ROOT / "temp")
sys.path[:0] = [str(SOURCE), str(SOURCE / "musetalk" / "utils")]
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
import musetalk_device_bridge as device_bridge
import musetalk_stream_probe as stream_probe
import musetalk_gpu_pack as gpu_pack
from musetalk_timestamp_queue import source_index
from accelerate import init_empty_weights
from diffusers import UNet2DConditionModel
from mmengine.config import Config
from mmengine.logging.history_buffer import HistoryBuffer
from mmpose.apis import inference_topdown, init_model
from mmpose.structures import merge_data_samples
from transformers import WhisperModel
from face_detection.detection.sfd.sfd_detector import SFDDetector
from musetalk.models.unet import PositionalEncoding
from musetalk.models.vae import VAE
from musetalk.utils.audio_processor import AudioProcessor
from musetalk.utils.blending import get_image
from musetalk.utils.face_parsing import FaceParsing
from musetalk.utils.face_parsing.resnet import Resnet18


def main():
    arguments = argparse.ArgumentParser()
    arguments.add_argument("--batch-size", type=int, choices=(1, 2, 4), default=1)
    arguments.add_argument("--precompute-masks", action="store_true")
    arguments.add_argument("--precompute-latents", action="store_true")
    arguments.add_argument("--gpu-pack", action="store_true")
    arguments.add_argument("--timestamped-probe", action="store_true")
    arguments.add_argument("--steady-prefill", type=int, default=0, choices=(0, 8))
    arguments.add_argument("--stream-probe-seconds", type=int, default=0, choices=(0, 10, 60, 600))
    arguments.add_argument("--device-bridge", type=Path)
    arguments.add_argument("--device-pause-at-seconds", type=float)
    arguments.add_argument("--device-pause-hold-seconds", type=float)
    options = arguments.parse_args()
    pause_at = options.device_pause_at_seconds
    pause_hold = options.device_pause_hold_seconds
    if (pause_at is None) != (pause_hold is None):
        raise ValueError("device pause needs both an audio position and a hold")
    if pause_at is not None and not (0.2 <= pause_at <= 3 and 0.2 <= pause_hold <= 2):
        raise ValueError("device pause is outside the bounded local clip")
    if options.device_bridge is not None:
        if options.stream_probe_seconds or options.gpu_pack or options.steady_prefill or options.timestamped_probe:
            raise ValueError("device bridge cannot be combined with the simulated probe or gpu-pack")
        if not options.precompute_masks or not options.precompute_latents:
            raise ValueError("device bridge requires prepared masks and latents")
        if not options.device_bridge.is_file():
            raise RuntimeError(f"device bridge executable unavailable: {options.device_bridge}")
        device_bridge.require_pcm_wav(AUDIO)
    elif pause_at is not None:
        raise ValueError("device pause requires the device bridge")
    batch_size = options.batch_size
    if not torch.cuda.is_available():
        raise RuntimeError("CUDA unavailable")
    for path in (VIDEO, AUDIO, FFMPEG):
        if not path.is_file():
            raise RuntimeError(f"required input unavailable: {path.name}")
    # Refuse to overwrite, including prior failed attempts.
    OUT.mkdir(parents=True, exist_ok=False)
    started = time.monotonic()
    stages = {}
    process = psutil.Process()
    rss_peak = [process.memory_info().rss]
    sampling_stop = threading.Event()
    def sample_memory():
        while not sampling_stop.wait(0.2):
            rss_peak[0] = max(rss_peak[0], process.memory_info().rss)
    threading.Thread(target=sample_memory, daemon=True).start()
    torch.cuda.reset_peak_memory_stats()
    torch.manual_seed(2026)
    device = torch.device("cuda")
    phase_started = time.monotonic()
    capture = cv2.VideoCapture(str(VIDEO))
    fps = capture.get(cv2.CAP_PROP_FPS)
    if not capture.isOpened() or fps != 30:
        raise RuntimeError("expected 30 fps research clip")
    frames = []
    for i in range(120):
        ok, frame = capture.read()
        if not ok or frame.shape != (540, 960, 3):
            raise RuntimeError(f"missing or unexpected input frame {i}")
        frames.append(frame)
    capture.release()
    config_path = SOURCE / "musetalk/utils/dwpose/rtmpose-l_8xb32-270e_coco-ubody-wholebody-384x288.py"
    identity = {"schema": 1, "video_sha256": cache.file_sha256(VIDEO),
                "dwpose_sha256": cache.file_sha256(STAGE / "models/dwpose/dw-ll_ucoco_384.pth"),
                "s3fd_sha256": cache.file_sha256(STAGE / "models/s3fd/s3fd.pth"),
                "dwpose_config_sha256": cache.file_sha256(config_path),
                "dwpose_base_sha256": cache.file_sha256(config_path.parent / "default_runtime.py"),
                "detector_source_sha256": cache.file_sha256(SOURCE / "musetalk/utils/face_detection/detection/sfd/sfd_detector.py"),
                "runner_sha256": cache.file_sha256(Path(__file__)),
                "opencv": cv2.__version__, "torch": torch.__version__, "mmpose": mmpose.__version__,
                "mmcv": mmcv.__version__, "mmengine": mmengine.__version__, "frames": 120, "size": [960, 540]}
    cache_dir = ROOT / "video-inspection" / "location-cache"
    boxes = cache.load(cache_dir, identity, 960, 540)
    cache_hit = boxes is not None
    stages["video_and_cache_check_seconds"] = time.monotonic() - phase_started
    stages["detector_load_seconds"] = 0.0
    stages["localization_120_frames_seconds"] = 0.0
    if not cache_hit:
        phase_started = time.monotonic()
        config = Config.fromfile(str(config_path))
        config.model.backbone.init_cfg = None  # prevent upstream implicit download
        numpy_types = ["uint8", "uint16", "uint32", "uint64", "int8", "int16", "int32", "int64", "float16", "float32", "float64", "bool"]
        allowed = [np.core.multiarray._reconstruct, np.ndarray, np.dtype, np.core.multiarray.scalar, HistoryBuffer]
        allowed += [type(np.dtype(name)) for name in numpy_types]
        with torch.serialization.safe_globals(allowed):
            pose = init_model(config, "models/dwpose/dw-ll_ucoco_384.pth", device=device)
        detector = SFDDetector(device="cuda", path_to_detector="models/s3fd/s3fd.pth")
        torch.cuda.synchronize()
        stages["detector_load_seconds"] = time.monotonic() - phase_started
        phase_started = time.monotonic()
        boxes = []
        with torch.inference_mode():
            for i, frame in enumerate(frames):
                found = detector.detect_from_batch(np.asarray([frame[..., ::-1].copy()]))[0]
                if len(found) != 1:
                    raise RuntimeError(f"expected exactly one face at frame {i}, got {len(found)}")
                keypoints = merge_data_samples(inference_topdown(pose, frame)).pred_instances.keypoints
                if keypoints.shape != (1, 133, 2) or not np.isfinite(keypoints).all():
                    raise RuntimeError(f"invalid landmarks at frame {i}")
                face = keypoints[0][23:91].astype(np.int32)
                middle = int(face[29, 1])
                upper = max(0, middle - (int(face[:, 1].max()) - middle))
                x1, x2 = int(face[:, 0].min()), int(face[:, 0].max())
                y1, y2 = int(upper), min(frame.shape[0], int(face[:, 1].max()) + 10)
                if not (0 <= x1 < x2 <= frame.shape[1] and 0 <= y1 < y2 <= frame.shape[0]):
                    raise RuntimeError(f"invalid landmark crop at frame {i}")
                boxes.append([x1, y1, x2, y2])
                if (i + 1) % 30 == 0:
                    print("localized", i + 1, "/ 120", flush=True)
        torch.cuda.synchronize()
        stages["localization_120_frames_seconds"] = time.monotonic() - phase_started
        cache.save(cache_dir, identity, boxes, 960, 540)
        del pose, detector
        gc.collect()
        torch.cuda.empty_cache()
    print("location_cache", "hit" if cache_hit else "miss", flush=True)

    phase_started = time.monotonic()
    processor = AudioProcessor(feature_extractor_path="models/whisper")
    whisper = WhisperModel.from_pretrained("models/whisper", local_files_only=True).to(device=device, dtype=torch.float16).eval()
    whisper.requires_grad_(False)
    with torch.inference_mode():
        features, length = processor.get_audio_feature(str(AUDIO))
        chunks = processor.get_whisper_chunk(features, device, torch.float16, whisper, length, fps=30)
    if len(chunks) != len(frames):
        raise RuntimeError(f"audio/video frame mismatch: {len(chunks)} vs {len(frames)}")
    torch.cuda.synchronize()
    stages["whisper_load_and_audio_features_seconds"] = time.monotonic() - phase_started
    del whisper
    gc.collect()
    torch.cuda.empty_cache()

    phase_started = time.monotonic()
    vae = VAE(model_path="models/sd-vae", use_float16=True)
    with open("models/musetalkV15/musetalk.json", encoding="utf-8") as f:
        model_config = json.load(f)
    with init_empty_weights():
        model = UNet2DConditionModel(**model_config)
    state = torch.load("models/musetalkV15/unet.pth", map_location="cpu", weights_only=True, mmap=True)
    model.load_state_dict(state, strict=True, assign=True)
    del state
    model = model.to(dtype=torch.float16).to(device=device).eval()
    encoding = PositionalEncoding(d_model=384).to(device=device, dtype=torch.float16)
    # FaceParsing strictly loads a complete BiSeNet checkpoint immediately
    # after constructing the backbone. Skip only the redundant legacy tar
    # pretrain read, which PyTorch cannot load in weights-only mode. The full
    # face-parse state_dict still loads strictly and includes the backbone.
    original_init_weight = Resnet18.init_weight
    try:
        Resnet18.init_weight = lambda self, model_path: None
        parser = FaceParsing(left_cheek_width=90, right_cheek_width=90)
    finally:
        Resnet18.init_weight = original_init_weight
    torch.cuda.synchronize()
    stages["render_models_load_seconds"] = time.monotonic() - phase_started
    phase_started = time.monotonic()
    materials, mask_bytes = (masks.prepare_all(frames, boxes, parser) if options.precompute_masks else (None, 0))
    torch.cuda.synchronize()
    stages["mask_preparation_seconds"] = time.monotonic() - phase_started if options.precompute_masks else 0.0
    phase_started = time.monotonic()
    prepared_latents = None
    if options.precompute_latents:
        torch.manual_seed(2026)
        prepared_latents = []
        with torch.inference_mode():
            for frame, box in zip(frames, boxes):
                x1, y1, x2, y2 = box
                crop = cv2.resize(frame[y1:y2, x1:x2], (256, 256), interpolation=cv2.INTER_LANCZOS4)
                prepared_latents.append(vae.get_latents_for_unet(crop).to(dtype=torch.float16))
        torch.cuda.synchronize()
    stages["latent_preparation_seconds"] = time.monotonic() - phase_started if options.precompute_latents else 0.0
    phase_started = time.monotonic()
    if prepared_latents is not None:
        with torch.inference_mode():
            warm_condition = encoding(chunks[:1].to(device=device, dtype=torch.float16))
            warm_output = model(prepared_latents[0], torch.tensor([0], device=device), encoder_hidden_states=warm_condition).sample
            vae.decode_latents(warm_output)
            del warm_condition, warm_output
        torch.cuda.synchronize()
    stages["render_warmup_seconds"] = time.monotonic() - phase_started if prepared_latents is not None else 0.0
    device_mode = options.device_bridge is not None
    if options.stream_probe_seconds or device_mode:
        if materials is None or prepared_latents is None:
            raise ValueError("continuous probe requires prepared masks and latents")
        if not device_mode and options.steady_prefill and not options.timestamped_probe:
            raise ValueError("steady prefill requires the timestamped probe")
        if not device_mode and options.steady_prefill and options.gpu_pack:
            raise ValueError("steady prefill measures the existing decoder; gpu-pack remains a separate experiment")
        timestamped = options.timestamped_probe or device_mode
        profile = bool(options.steady_prefill) or device_mode
        stream_timing = {"unet_seconds": 0.0, "decode_seconds": 0.0, "blend_seconds": 0.0, "batches": 0, "generated_frames": 0}
        profile_events = []
        def generate_batch(index):
            indexes = ([source_index(index + k, 30, 25, len(frames)) for k in range(batch_size)]
                       if timestamped else [(index + k) % len(frames) for k in range(batch_size)])
            with torch.inference_mode():
                inputs = torch.cat([prepared_latents[i] for i in indexes])
                features = torch.cat([chunks[i:i+1] for i in indexes]).to(device=device, dtype=torch.float16)
                if profile:
                    unet_start, unet_end, decode_start, decode_end = (torch.cuda.Event(True) for _ in range(4))
                    unet_start.record()
                    predicted = model(inputs, torch.tensor([0], device=device), encoder_hidden_states=encoding(features)).sample
                    unet_end.record()
                    decode_start.record()
                    faces = vae.decode_latents(predicted)
                    decode_end.record()
                    profile_events.append((unet_start, unet_end, decode_start, decode_end))
                else:
                    tick = time.monotonic()
                    predicted = model(inputs, torch.tensor([0], device=device), encoder_hidden_states=encoding(features)).sample
                    torch.cuda.synchronize()
                    stream_timing["unet_seconds"] += time.monotonic() - tick
                    tick = time.monotonic()
                    faces = gpu_pack.decode(vae, predicted) if options.gpu_pack else vae.decode_latents(predicted)
                    torch.cuda.synchronize()
                    stream_timing["decode_seconds"] += time.monotonic() - tick
                tick = time.monotonic()
                result = []
                for i, face in zip(indexes, faces):
                    x, y, x1, y1 = boxes[i]
                    face = cv2.resize(face, (x1-x, y1-y), interpolation=cv2.INTER_LANCZOS4)
                    result.append(masks.blend(frames[i], face, boxes[i], materials[i]))
                stream_timing["blend_seconds"] += time.monotonic() - tick
                stream_timing["batches"] += 1
                stream_timing["generated_frames"] += len(result)
                return result
        if profile:
            for warm in (0, batch_size):
                generate_batch(warm)
            torch.cuda.synchronize()
            profile_events.clear()
            stream_timing.update(unet_seconds=0.0, decode_seconds=0.0, blend_seconds=0.0, batches=0, generated_frames=0)
        def gpu_line():
            try:
                return subprocess.check_output(
                    ["nvidia-smi", "--query-gpu=temperature.gpu,clocks.sm,memory.used,utilization.gpu", "--format=csv,noheader"],
                    text=True, timeout=5).strip()
            except (OSError, subprocess.SubprocessError) as exc:
                return f"unavailable: {type(exc).__name__}"
        gc.collect()
        torch.cuda.empty_cache()
        print("memory_before_probe_rss_mib", round(process.memory_info().rss/1048576, 1),
              "available_mib", round(psutil.virtual_memory().available/1048576, 1), flush=True)
        gpu_before = gpu_line()
        saved_ticks = []
        if device_mode:
            holder = {}

            def on_present(tick, frame):
                if frame is not None and tick in (0, 25, 50, 75):
                    path = OUT / f"device-frame-{tick}.png"
                    if cv2.imwrite(str(path), frame):
                        saved_ticks.append(tick)

            def start_clock():
                bridge = device_bridge.DeviceBridge(options.device_bridge, AUDIO)
                bridge.open()
                holder["bridge"] = bridge
                return bridge

            def before_go():
                holder["gpu_at_go"] = gpu_line()

            try:
                metrics = stream_probe.run_external_clock(
                    generate_batch, start_clock, prefill=8, on_present=on_present, before_go=before_go,
                    pause_at_samples=None if pause_at is None else int(round(pause_at * 24000)),
                    pause_hold_s=pause_hold)
            except Exception as exc:
                sampling_stop.set()
                with (OUT / "device-clock.json").open("x", encoding="utf-8") as stream:
                    json.dump({"error": str(exc), "saved_ticks": saved_ticks,
                               "stderr_tail": holder.get("bridge").stderr_tail if holder.get("bridge") else "",
                               "scope": "local device bridge failed before a completed record"}, stream, ensure_ascii=False, indent=2)
                raise
        else:
            metrics = (stream_probe.run_timestamped(
                           generate_batch, options.stream_probe_seconds, prefill=options.steady_prefill)
                       if options.timestamped_probe else stream_probe.run(generate_batch, options.stream_probe_seconds))
        if profile:
            torch.cuda.synchronize()
            unet_ms = decode_ms = 0.0
            for unet_start, unet_end, decode_start, decode_end in profile_events:
                unet_ms += unet_start.elapsed_time(unet_end)
                decode_ms += decode_start.elapsed_time(decode_end)
            stream_timing["unet_seconds"] = unet_ms / 1000
            stream_timing["decode_seconds"] = decode_ms / 1000
        gpu_after = gpu_line()
        sampling_stop.set()
        if timestamped and not device_mode:
            ticks = metrics["presented"] + metrics["missing"]
            ratio = metrics["presented"] / ticks if ticks else 0.0
            metrics["timed_ticks"] = ticks
            metrics["presented_ratio"] = round(ratio, 4)
            if options.steady_prefill:
                metrics["steady_research_bar"] = "timed window presented ratio >= 0.98; startup prefill excluded; not live, device-audio, or OBS certification"
                metrics["steady_bar_met"] = ratio >= 0.98
        if device_mode:
            played_final = metrics.get("played_final", 0)
            metrics["device_bar"] = "played source samples reach at least 90% of the clip, one allowed clock has played>0, no frame is presented after disallow, and played stays frozen after stop; not OBS, platform push, or approval of the 960x540 frames"
            metrics["device_bar_met"] = bool(
                played_final >= int(metrics["clip_samples"] * 0.9)
                and metrics.get("allowed_seen") and metrics.get("played_peak", 0) > 0
                and metrics.get("presented", 0) > 0 and metrics.get("presented_after_disallow", 1) == 0
                and metrics.get("selection_violations", 1) == 0 and not metrics.get("progress_after_stop", True)
                and not metrics.get("timed_out", True))
            metrics["saved_ticks"] = saved_ticks
            pause = metrics.get("pause")
            if pause is not None:
                metrics["pause_bar"] = "after PAUSED, played and submitted stay put for the hold; resume leaves that played sample behind; frames still follow played samples; not OBS or public output"
                metrics["pause_bar_met"] = bool(
                    not pause.get("played_moved", True) and not pause.get("submitted_moved", True)
                    and metrics.get("played_final", 0) > pause.get("played_at_pause", 0)
                    and metrics.get("max_presented_tick", -1) > pause.get("played_at_pause", 0) * 25 // 24000
                    and metrics.get("selection_violations", 1) == 0
                    and metrics.get("presented_after_disallow", 1) == 0
                    and not metrics.get("progress_after_stop", True))
            metrics["gpu_at_go"] = holder.get("gpu_at_go", "")
            metrics["second_note"] = "index 0 is the first second of played audio; each entry is 25 ticks at 25 fps"
        metrics.update(stream_timing={k: round(v, 3) if isinstance(v, float) else v for k, v in stream_timing.items()},
                       profiling_note=("CUDA event totals include prefill and the timed window, queried after one sync; two warmup batches are excluded"
                                       if profile else
                                       "per-batch CUDA synchronization adds profiling overhead"),
                       gpu_pack=options.gpu_pack, steady_prefill=options.steady_prefill, device_bridge=device_mode,
                       gpu_before=gpu_before, gpu_after=gpu_after,
                       stages={k: round(v, 3) for k, v in stages.items()}, batch_size=batch_size,
                       location_cache="hit" if cache_hit else "miss",
                       process_rss_sampled_peak_mib=round(rss_peak[0]/1048576, 1),
                       pytorch_cuda_peak_allocated_mib=round(torch.cuda.max_memory_allocated()/1048576, 1),
                       scope=("local Qwen wav on the clocked WASAPI experiment; frames follow played samples and the public execution gate; no platform push, OBS, or public-media approval"
                              if device_mode else
                              "repeated prerecorded audio features; simulated clock starts after prefill; no playback, encoding, OBS, or public output"
                              if options.steady_prefill else
                              "repeated prerecorded audio features, simulated 25fps consumer; no audio, encoding, OBS, or public output"))
        record_name = "device-clock.json" if device_mode else "stream-probe.json"
        with (OUT / record_name).open("x", encoding="utf-8") as stream:
            json.dump(metrics, stream, ensure_ascii=False, indent=2)
        print("DEVICE_CLOCK" if device_mode else "STREAM_PROBE", json.dumps(metrics, ensure_ascii=False), flush=True)
        return
    phase_started = time.monotonic()
    silent = OUT / "video-silent.mp4"
    writer = cv2.VideoWriter(str(silent), cv2.VideoWriter_fourcc(*"mp4v"), fps, (frames[0].shape[1], frames[0].shape[0]))
    if not writer.isOpened():
        raise RuntimeError("cannot open video encoder")
    try:
        timing = {"vae_encode": 0.0, "unet": 0.0, "vae_decode": 0.0, "blend_and_write": 0.0}
        torch.manual_seed(2026)  # identical sampling regardless of cache hit
        first_frame_seconds = None
        with torch.inference_mode():
            for start in range(0, len(frames), batch_size):
                end = min(start + batch_size, len(frames))
                latents = []
                if prepared_latents is not None:
                    latents = prepared_latents[start:end]
                else:
                    for frame, box in zip(frames[start:end], boxes[start:end]):
                        x1, y1, x2, y2 = box
                        crop = cv2.resize(frame[y1:y2, x1:x2], (256, 256), interpolation=cv2.INTER_LANCZOS4)
                        part = time.monotonic()
                        latents.append(vae.get_latents_for_unet(crop).to(dtype=torch.float16))
                        torch.cuda.synchronize(); timing["vae_encode"] += time.monotonic() - part
                part = time.monotonic()
                condition = encoding(chunks[start:end].to(device=device, dtype=torch.float16))
                predicted = model(torch.cat(latents), torch.tensor([0], device=device), encoder_hidden_states=condition).sample
                torch.cuda.synchronize(); timing["unet"] += time.monotonic() - part
                part = time.monotonic()
                faces = vae.decode_latents(predicted)
                torch.cuda.synchronize(); timing["vae_decode"] += time.monotonic() - part
                for i, face in enumerate(faces, start):
                    frame, box = frames[i], boxes[i]
                    x1, y1, x2, y2 = box
                    part = time.monotonic()
                    face = cv2.resize(face, (x2 - x1, y2 - y1), interpolation=cv2.INTER_LANCZOS4)
                    writer.write(masks.blend(frame, face, box, materials[i]) if materials is not None
                                 else get_image(frame, face, box, mode="jaw", fp=parser))
                    torch.cuda.synchronize(); timing["blend_and_write"] += time.monotonic() - part
                    if first_frame_seconds is None:
                        first_frame_seconds = time.monotonic() - phase_started
                    if (i + 1) % 30 == 0:
                        print("rendered", i + 1, "/ 120", flush=True)
    finally:
        writer.release()
    torch.cuda.synchronize()
    stages["render_120_frames_and_encode_seconds"] = time.monotonic() - phase_started
    phase_started = time.monotonic()
    output = OUT / "sample.mp4"
    result = subprocess.run([str(FFMPEG), "-nostdin", "-v", "error", "-n", "-i", str(silent), "-i", str(AUDIO), "-map", "0:v:0", "-map", "1:a:0", "-c:v", "copy", "-c:a", "aac", "-shortest", str(output)], capture_output=True, text=True, timeout=60, check=False)
    if result.returncode != 0 or not output.is_file() or output.stat().st_size == 0:
        raise RuntimeError(f"audio mux failed: {result.returncode}: {result.stderr[:500]}")
    check = cv2.VideoCapture(str(output))
    actual = int(check.get(cv2.CAP_PROP_FRAME_COUNT)) if check.isOpened() else 0
    check.release()
    if actual != 120:
        raise RuntimeError(f"output frame count unexpected: {actual}")
    stages["mux_and_validation_seconds"] = time.monotonic() - phase_started
    sampling_stop.set()
    metrics = {"stages": {key: round(value, 3) for key, value in stages.items()},
               "total_seconds": round(time.monotonic() - started, 3),
               "frames": actual, "fps": fps, "batch_size": batch_size,
               "mask_precompute": options.precompute_masks, "mask_bytes": mask_bytes,
               "latent_precompute": options.precompute_latents,
               "first_frame_after_render_start_seconds": round(first_frame_seconds, 3), "location_cache": "hit" if cache_hit else "miss",
               "render_breakdown_seconds": {key: round(value, 3) for key, value in timing.items()},
               "profiling_note": "CUDA synchronization adds profiling overhead; offline experiment, not realtime throughput",
               "process_rss_sampled_peak_mib": round(rss_peak[0] / 1048576, 1),
               "pytorch_cuda_peak_allocated_mib": round(torch.cuda.max_memory_allocated() / 1048576, 1),
               "pytorch_cuda_peak_reserved_mib": round(torch.cuda.max_memory_reserved() / 1048576, 1),
               "scope": "research Python process only; excludes subprocess RSS, system and other GPU allocations"}
    with (OUT / "metrics.json").open("x", encoding="utf-8") as f:
        json.dump(metrics, f, ensure_ascii=False, indent=2)
    print("METRICS", json.dumps(metrics, ensure_ascii=False), flush=True)
    print("SAMPLE_OK", output, "frames", actual, "elapsed_sec", round(time.monotonic() - started, 2), flush=True)


if __name__ == "__main__":
    main()
