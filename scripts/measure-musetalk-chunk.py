"""Time one 400 ms Whisper chunk against the full 4 s research audio.

Whisper only. Loading this chunk and then the UNet in one process crashed
with an access violation, so this stays a separate process. It does not
rewrite the location cache or the reference runner.
"""
import json
import os
from pathlib import Path
import sys
import time
import wave

ROOT = Path(__file__).resolve().parents[1] / "target-windows" / "realistic-avatar-research"
SOURCE = ROOT / "MuseTalk"
STAGE = ROOT / "offline-reference"
AUDIO = ROOT / "video-inspection" / "qwen-nai-welcome.wav"
OUT = ROOT / "video-inspection" / f"musetalk-chunk-{time.time_ns()}"
os.environ["HF_HUB_OFFLINE"] = "1"
os.environ["TRANSFORMERS_OFFLINE"] = "1"
os.environ["PYTHONDONTWRITEBYTECODE"] = "1"
sys.path[:0] = [str(SOURCE), str(SOURCE / "musetalk" / "utils")]
os.chdir(STAGE)

import torch
from musetalk.utils.audio_processor import AudioProcessor
from transformers import WhisperModel

CHUNK_SAMPLES = 9600


def main():
    device = torch.device("cuda")
    processor = AudioProcessor(feature_extractor_path="models/whisper")
    whisper = WhisperModel.from_pretrained("models/whisper", local_files_only=True).to(device=device, dtype=torch.float16).eval()
    whisper.requires_grad_(False)
    with torch.inference_mode():
        features, length = processor.get_audio_feature(str(AUDIO))
        full = processor.get_whisper_chunk(features, device, torch.float16, whisper, length, fps=30)
    torch.cuda.synchronize()
    OUT.mkdir(parents=True, exist_ok=False)
    chunk_wav = OUT / "chunk-400ms.wav"
    with wave.open(str(AUDIO), "rb") as source, wave.open(str(chunk_wav), "wb") as target:
        if (source.getnchannels(), source.getframerate(), source.getsampwidth()) != (1, 24000, 2):
            raise RuntimeError("research audio is not 24 kHz mono s16")
        target.setnchannels(1)
        target.setsampwidth(2)
        target.setframerate(24000)
        target.writeframes(source.readframes(CHUNK_SAMPLES))
    started = time.perf_counter()
    with torch.inference_mode():
        short_features, short_length = processor.get_audio_feature(str(chunk_wav))
        short = processor.get_whisper_chunk(short_features, device, torch.float16, whisper, short_length, fps=30)
    torch.cuda.synchronize()
    seconds = time.perf_counter() - started
    delta = None
    if len(short) >= 1 and tuple(short[0].shape) == tuple(full[0].shape):
        delta = float((short[0].detach().float().cpu() - full[0].detach().float().cpu()).abs().max().item())
    payload = {
        "chunk_ms": 400,
        "chunk_feature_seconds": seconds,
        "chunk_frames": len(short),
        "full_frames": len(full),
        "first_frame_max_abs_diff": delta,
        "matches_full_file_first_frame": delta == 0.0,
        "runner_untouched": True,
        "cache_rewritten": False,
        "scope": "whisper chunk timing only; not a frame render, device playback, or public output",
    }
    with (OUT / "chunk.json").open("x", encoding="utf-8") as stream:
        json.dump(payload, stream, ensure_ascii=False, indent=2)
        stream.write("\n")
    print("chunk_result", json.dumps({
        "seconds": round(seconds, 6),
        "frames": len(short),
        "diff": delta,
    }, ensure_ascii=False), flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
