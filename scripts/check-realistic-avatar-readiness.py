"""Read-only inventory for the isolated MuseTalk research environment.

This checks presence and fixed digests, not license, model quality or readiness
for public output. No model is loaded and no remote connection is made.
"""
import hashlib
import json
import socket
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1] / "target-windows" / "realistic-avatar-research"
WEIGHTS = {
    "musetalk15-unet.pth": "7ebf6c98c181e20838e4c0054e96e944ac60d5d692cc01db42839fe11b787007",
    "dw-ll_ucoco_384.pth": "0d9408b13cd863c4e95a149dd31232f88f2a12aa6cf8964ed74d7d97748c7a07",
    "vae.safetensors": "a1d993488569e928462932c8c38a0760b874d166399b14414135bd9c42df5815",
    "whisper-tiny.bin": "9607f98a2b22d9e229ae43c52ecea79dcede9e0c5cfae67e8da6eda86d8aac1d",
    "face-parse.pth": "468e13ca13a9b43cc0881a9f99083a430e9c0a38abd935431d1c28ee94b26567",
    "s3fd.pth": "619a31681264d3f7f7fc7a16a42cbbe8b23f31a256f75a366e5a1bcd59b33543",
}
CONFIGS = {
    "musetalk.json": "5b6923aee04d71692e0e9846c471e0a4ea07a4f686d39545e472bd4ba17e1b47",
    "vae-config.json": "92d3dfb746fca211a2c9e019e285f8597412211728dce3c5bcf4eda0f2d62e7e",
    "whisper-config.json": "ffdccec4f3211f4c63310f2b7098f309fe70f3952cedc5e4d11e43f5b2379b98",
    "whisper-preprocessor.json": "9b5cd03a36fbb8a627c64d98a5b5b126ead95a77720723944487311f0110b666",
}


def digest(path):
    result = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            result.update(block)
    return result.hexdigest()


def main():
    weights = {}
    for name, expected in WEIGHTS.items():
        path = ROOT / "downloads" / name
        if not path.is_file():
            weights[name] = "missing"
            continue
        try:
            weights[name] = "verified" if digest(path) == expected else "digest_mismatch"
        except OSError:
            weights[name] = "unreadable"
    configs = {}
    for name, expected in CONFIGS.items():
        path = ROOT / "downloads" / "metadata-candidates" / name
        if not path.is_file():
            configs[name] = "missing"
        else:
            try:
                configs[name] = "verified" if digest(path) == expected else "digest_mismatch"
            except OSError:
                configs[name] = "unreadable"
    try:
        with socket.create_connection(("127.0.0.1", 8000), timeout=1):
            qwen = "port_open_identity_unverified"
    except OSError as error:
        if getattr(error, "winerror", None) == 10013 or error.errno == 13:
            qwen = "check_denied_by_environment"
        else:
            qwen = "port_closed_or_unreachable"
    print(json.dumps({
        "weights": weights,
        "metadata": configs,
        "qwen_127_0_0_1_8000": qwen,
        "missing_or_unverified": ["resnet18_independent_identity", "authorized_video_not_checked_by_script", "model_dependency_and_license_closure"],
        "public_output_ready": False,
        "note": "Offline inventory only; open port does not prove service identity. Authorized video is not searched automatically.",
    }, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
