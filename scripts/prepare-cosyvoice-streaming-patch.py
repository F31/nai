"""Prepare a minimal streaming patch; preserve the mounted opencode audio fix."""
import argparse
import ast
import hashlib
from pathlib import Path


def prepare(source):
    def replace(old, new, count=1):
        nonlocal source
        if source.count(old) != count:
            raise ValueError(f"unsupported service: expected {count} occurrences of {old!r}")
        source = source.replace(old, new)
    replace("import logging\n", "import logging\nimport time\n")
    replace("spk_id: str = Form()):", "spk_id: str = Form(), stream: bool = Form(True)):")
    replace("instruct_text: str = Form()):", "instruct_text: str = Form(), stream: bool = Form(True)):")
    replace("prompt_wav: UploadFile = File()):", "prompt_wav: UploadFile = File(), stream: bool = Form(True)):", 3)
    for method, args in [("sft", "tts_text, spk_id"),
                         ("zero_shot", "tts_text, prompt_text, prompt_speech_16k"),
                         ("cross_lingual", "tts_text, prompt_speech_16k"),
                         ("instruct", "tts_text, spk_id, instruct_text"),
                         ("instruct2", "tts_text, instruct_text, prompt_speech_16k")]:
        replace(f"cosyvoice.inference_{method}({args})", f"cosyvoice.inference_{method}({args}, stream=stream)")
    replace("""def generate_data(model_output):
    for i in model_output:
        tts_audio = (i['tts_speech'].numpy() * (2 ** 15)).astype(np.int16).tobytes()
        yield tts_audio
""", '''def generate_data(model_output):
    started = time.monotonic()
    count = 0
    samples = 0
    for item in model_output:
        audio = item['tts_speech'].detach().cpu().numpy()
        if audio.ndim != 2 or audio.shape[0] != 1 or not np.isfinite(audio).all():
            raise ValueError("expected finite mono model audio")
        tts_audio = (np.clip(audio, -1, 32767 / 32768) * 32768).astype('<i2').tobytes()
        count += 1
        samples += audio.shape[1]
        logging.info("NAI_TTS_CHUNK index=%d samples=%d cumulative_samples=%d elapsed=%.4f",
                     count, audio.shape[1], samples, time.monotonic() - started)
        yield tts_audio
    logging.info("NAI_TTS_END chunks=%d samples=%d elapsed=%.4f", count, samples, time.monotonic() - started)


def pcm_response(model_output, stream):
    return StreamingResponse(generate_data(model_output), media_type="application/octet-stream",
                             headers={"X-Sample-Rate": str(cosyvoice.sample_rate),
                                      "X-Audio-Channels": "1", "X-Audio-Encoding": "pcm_s16le",
                                      "X-Model-Stream": str(stream).lower()})
''')
    replace("return StreamingResponse(generate_data(model_output))", "return pcm_response(model_output, stream)", 5)
    routes = [n for n in ast.parse(source).body if isinstance(n, ast.AsyncFunctionDef) and n.name.startswith("inference_")]
    if len(routes) != 5:
        raise ValueError("expected five routes")
    for route in routes:
        calls = [n for n in ast.walk(route) if isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute) and n.func.attr.startswith("inference_")]
        if len(calls) != 1 or not any(k.arg == "stream" for k in calls[0].keywords):
            raise ValueError("missing model stream argument")
    compile(source, "cosyvoice-candidate", "exec")
    return source


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    if args.source.resolve() == args.output.resolve() or args.output.exists():
        raise ValueError("output must be a new separate file")
    raw = args.source.read_bytes()
    candidate = prepare(raw.decode("utf-8-sig").replace("\r\n", "\n"))
    args.output.write_text(candidate, encoding="utf-8")
    print("Prepared; not deployed. Source SHA256=" + hashlib.sha256(raw).hexdigest())


if __name__ == "__main__":
    main()
