"""Download explicitly pinned research weights with verified TLS and SHA-256.

The mirror carries public model files only; the expected digests were read
from their original Hugging Face file pages. Never load unverified files.
"""
import argparse
import hashlib
import os
from pathlib import Path
import time

import requests

ROOT = Path(__file__).resolve().parents[1] / "target-windows" / "realistic-avatar-research" / "downloads"
FILES = {
    "vae": ("https://hf-mirror.com/stabilityai/sd-vae-ft-mse/resolve/31f26fdeee1355a5c34592e401dd41e45d25a493/diffusion_pytorch_model.safetensors", "vae.safetensors", "a1d993488569e928462932c8c38a0760b874d166399b14414135bd9c42df5815"),
    "whisper": ("https://hf-mirror.com/openai/whisper-tiny/resolve/169d4a4341b33bc18d8881c4b69c2e104e1cc0af/pytorch_model.bin", "whisper-tiny.bin", "9607f98a2b22d9e229ae43c52ecea79dcede9e0c5cfae67e8da6eda86d8aac1d"),
    "face-parse": ("https://hf-mirror.com/ManyOtherFunctions/face-parse-bisent/resolve/0073b233a5a3c4b1377d4dbf49245017938a72b5/79999_iter.pth", "face-parse.pth", "468e13ca13a9b43cc0881a9f99083a430e9c0a38abd935431d1c28ee94b26567"),
    "dwpose": ("https://hf-mirror.com/yzd-v/DWPose/resolve/1a7144101628d69ee7a3768d1ee3a094070dc388/dw-ll_ucoco_384.pth", "dw-ll_ucoco_384.pth", "0d9408b13cd863c4e95a149dd31232f88f2a12aa6cf8964ed74d7d97748c7a07"),
    # Mirror copy matches the filename digest prefix from face-alignment's
    # original URL; full digest checked against the pinned HF file metadata.
    "s3fd": ("https://hf-mirror.com/chunyu-li/face-alignment/resolve/30a934a95e2f903250ecf929846a1ebc475c2b0f/s3fd-619a316812.pth", "s3fd.pth", "619a31681264d3f7f7fc7a16a42cbbe8b23f31a256f75a366e5a1bcd59b33543"),
    "musetalk15": ("https://hf-mirror.com/TMElyralab/MuseTalk/resolve/2bcb936e2fddb4d86db4c62fd45b387d0c061571/musetalkV15/unet.pth", "musetalk15-unet.pth", "7ebf6c98c181e20838e4c0054e96e944ac60d5d692cc01db42839fe11b787007"),
}

def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("model", choices=FILES)
    parser.add_argument("--max-seconds", type=int, default=900)
    args = parser.parse_args()
    if not 30 <= args.max_seconds <= 3600:
        parser.error("--max-seconds must be between 30 and 3600")
    deadline = time.monotonic() + args.max_seconds
    url, filename, expected = FILES[args.model]
    ROOT.mkdir(parents=True, exist_ok=True)
    target, partial = ROOT / filename, ROOT / (filename + ".partial")
    if target.exists():
        if digest(target) != expected:
            raise RuntimeError("Existing file digest mismatch; preserved for inspection")
        print("Verified existing", filename, flush=True)
        return
    session = requests.Session()
    session.trust_env = False
    for attempt in range(3):
        offset = partial.stat().st_size if partial.exists() else 0
        try:
            with session.get(url, headers={"Range": f"bytes={offset}-"} if offset else {}, stream=True, timeout=(15, 30)) as response:
                response.raise_for_status()
                if offset and (response.status_code != 206 or not response.headers.get("Content-Range", "").startswith(f"bytes {offset}-")):
                    raise RuntimeError("Server rejected verified range; partial preserved")
                print(f"Downloading {filename}, offset {offset}, attempt {attempt + 1}", flush=True)
                with partial.open("ab" if offset else "wb") as stream:
                    for block in response.iter_content(64 * 1024):
                        if time.monotonic() >= deadline:
                            raise TimeoutError("Time budget exhausted; partial preserved")
                        if block:
                            stream.write(block)
                    stream.flush()
                    os.fsync(stream.fileno())
            if digest(partial) != expected:
                raise RuntimeError("Digest mismatch; partial preserved, do not load")
            os.replace(partial, target)
            print("Verified SHA-256", expected, flush=True)
            return
        except (requests.RequestException, OSError) as exc:
            print(f"Download interrupted: {type(exc).__name__}; partial preserved", flush=True)
            if attempt == 2:
                raise
            time.sleep(3)

if __name__ == "__main__":
    main()
