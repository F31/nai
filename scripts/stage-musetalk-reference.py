"""Stage hash-pinned MuseTalk assets into an isolated, offline research root.

The ResNet source was downloaded over verified TLS from the upstream PyTorch
URL and its filename hash prefix was checked; the full digest below records
the obtained bytes, not an independently published full digest. This script
does not grant a license or declare production readiness.
"""
import hashlib
import os
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "target-windows" / "realistic-avatar-research"
D = ROOT / "downloads"
C = D / "metadata-candidates"
STAGE = ROOT / "offline-reference"
FILES = [
    (D / "musetalk15-unet.pth", "models/musetalkV15/unet.pth", "7ebf6c98c181e20838e4c0054e96e944ac60d5d692cc01db42839fe11b787007"),
    (C / "musetalk.json", "models/musetalkV15/musetalk.json", "5b6923aee04d71692e0e9846c471e0a4ea07a4f686d39545e472bd4ba17e1b47"),
    (D / "dw-ll_ucoco_384.pth", "models/dwpose/dw-ll_ucoco_384.pth", "0d9408b13cd863c4e95a149dd31232f88f2a12aa6cf8964ed74d7d97748c7a07"),
    (D / "s3fd.pth", "models/s3fd/s3fd.pth", "619a31681264d3f7f7fc7a16a42cbbe8b23f31a256f75a366e5a1bcd59b33543"),
    (D / "vae.safetensors", "models/sd-vae/diffusion_pytorch_model.safetensors", "a1d993488569e928462932c8c38a0760b874d166399b14414135bd9c42df5815"),
    (C / "vae-config.json", "models/sd-vae/config.json", "92d3dfb746fca211a2c9e019e285f8597412211728dce3c5bcf4eda0f2d62e7e"),
    (D / "whisper-tiny.bin", "models/whisper/pytorch_model.bin", "9607f98a2b22d9e229ae43c52ecea79dcede9e0c5cfae67e8da6eda86d8aac1d"),
    (C / "whisper-config.json", "models/whisper/config.json", "ffdccec4f3211f4c63310f2b7098f309fe70f3952cedc5e4d11e43f5b2379b98"),
    (C / "whisper-preprocessor.json", "models/whisper/preprocessor_config.json", "9b5cd03a36fbb8a627c64d98a5b5b126ead95a77720723944487311f0110b666"),
    (D / "face-parse.pth", "models/face-parse-bisent/79999_iter.pth", "468e13ca13a9b43cc0881a9f99083a430e9c0a38abd935431d1c28ee94b26567"),
    (C / "resnet18-candidate.pth", "models/face-parse-bisent/resnet18-5c106cde.pth", "5c106cde386e87d4033832f2996f5493238eda96ccf559d1d62760c4de0613f8"),
]


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda: f.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def main():
    for source, _, expected in FILES:
        if not source.is_file() or digest(source) != expected:
            raise RuntimeError(f"missing or changed source: {source.name}")
    for source, relative, expected in FILES:
        dest = STAGE / relative
        dest.parent.mkdir(parents=True, exist_ok=True)
        if dest.exists():
            if not dest.is_file() or digest(dest) != expected:
                raise RuntimeError(f"existing destination differs: {relative}")
            print("verified existing", relative, flush=True)
            continue
        try:
            os.link(source, dest)
        except OSError:
            # Exclusive creation prevents replacing an unrelated prior artifact.
            with source.open("rb") as src, dest.open("xb") as dst:
                shutil.copyfileobj(src, dst, 1024 * 1024)
        if digest(dest) != expected:
            raise RuntimeError(f"staged digest mismatch: {relative}")
        print("verified", relative, flush=True)


if __name__ == "__main__":
    main()
