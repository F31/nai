"""One bounded multipart/PCM streaming probe; no retries and no audio playback."""
import argparse
import hashlib
import http.client
import json
import time
import uuid
import wave
from pathlib import Path
from urllib.parse import urlsplit


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--endpoint", required=True)
    parser.add_argument("--reference", required=True, type=Path)
    parser.add_argument("--prompt-text", required=True)
    parser.add_argument("--text", required=True)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--timeout", type=float, default=60)
    parser.add_argument("--max-bytes", type=int, default=10485760)
    parser.add_argument("--reference-max-bytes", type=int, default=10485760)
    parser.add_argument("--check-busy", action="store_true", help="Submit one concurrent request; require 409 without generation")
    parser.add_argument("--disconnect-after-first", action="store_true", help="Close after first PCM and record cancellation, not complete audio")
    args = parser.parse_args()
    url = urlsplit(args.endpoint)
    if url.scheme != "http" or url.username or url.password or url.query or url.fragment:
        raise ValueError("probe requires a plain HTTP endpoint without credentials/query")
    if args.timeout <= 0 or args.max_bytes <= 0 or not 1 <= args.reference_max_bytes <= 10485760:
        raise ValueError("positive bounds required")
    with args.reference.open('rb') as source:
        reference=source.read(args.reference_max_bytes+1)
    if len(reference) > args.reference_max_bytes:
        raise ValueError("reference exceeds probe limit")
    args.output_dir.mkdir(parents=True, exist_ok=False)
    boundary = "nai-probe-" + uuid.uuid4().hex
    body = bytearray()
    for name, value in {"tts_text": args.text, "prompt_text": args.prompt_text, "stream": "true"}.items():
        body.extend(f'--{boundary}\r\nContent-Disposition: form-data; name="{name}"\r\n\r\n{value}\r\n'.encode())
    body.extend(f'--{boundary}\r\nContent-Disposition: form-data; name="prompt_wav"; filename="reference.wav"\r\nContent-Type: audio/wav\r\n\r\n'.encode())
    body.extend(reference)
    body.extend(f'\r\n--{boundary}--\r\n'.encode())
    result = {"endpoint": args.endpoint, "calls": 1, "request_stream": True,
              "reference_sha256": hashlib.sha256(reference).hexdigest(), "text": args.text,
              "started_unix": time.time(), "status": "failed"}
    conn = http.client.HTTPConnection(url.hostname, url.port, timeout=args.timeout)
    started = time.monotonic()
    data = bytearray()
    reads = []
    try:
        conn.request("POST", url.path or "/", body=body, headers={"Content-Type": f"multipart/form-data; boundary={boundary}"})
        response = conn.getresponse()
        result.update(http_status=response.status, headers=dict(response.getheaders()), headers_seconds=time.monotonic() - started)
        if response.status != 200:
            raise RuntimeError(f"HTTP {response.status}; no retry")
        if args.check_busy:
            contender = http.client.HTTPConnection(url.hostname, url.port, timeout=min(5, args.timeout))
            checked = time.monotonic()
            try:
                contender.request("POST", url.path or "/", body=body, headers={"Content-Type": f"multipart/form-data; boundary={boundary}"})
                refused = contender.getresponse()
                result.update(concurrent_http_status=refused.status, concurrent_response_seconds=time.monotonic() - checked)
                refused.read(4096)
                if refused.status != 409:
                    raise RuntimeError("single-flight failed to reject contender")
            finally:
                contender.close()
        if response.getheader("X-Audio-Encoding") != "pcm_s16le" or response.getheader("X-Audio-Channels") != "1":
            raise RuntimeError("missing/unsupported audio format declaration")
        rate = int(response.getheader("X-Sample-Rate", "0"))
        if rate <= 0 or rate > 192000:
            raise RuntimeError("invalid sample rate")
        while True:
            remaining = args.timeout - (time.monotonic() - started)
            if remaining <= 0:
                raise TimeoutError("probe overall deadline exceeded")
            if conn.sock is not None:
                conn.sock.settimeout(remaining)
            chunk = response.read1(4096)
            if not chunk:
                break
            reads.append({"seconds": time.monotonic() - started, "bytes": len(chunk)})
            data.extend(chunk)
            if len(data) > args.max_bytes:
                raise ValueError("output exceeds probe limit")
            if args.disconnect_after_first and len(data) >= 2:
                result.update(status="disconnected", first_pcm_seconds=reads[0]["seconds"], received_bytes=len(data),
                              total_seconds=time.monotonic() - started,
                              note="Intentionally closed delivery after first PCM; backend stop/cleanup must be checked separately.")
                response.close()
                break
        if args.disconnect_after_first:
            if result["status"] != "disconnected":
                raise RuntimeError("no PCM received before intentional disconnect")
            return_code = 0
        else:
            return_code = None
        if return_code is None and (not data or len(data) % 2):
            raise ValueError("empty or partial PCM sample")
        if return_code is not None:
            print(json.dumps(result, ensure_ascii=False, indent=2))
            return return_code
        elapsed = time.monotonic() - started
        duration = len(data) / 2 / rate
        prefix_bytes = 0
        first_pcm = None
        for read in reads:
            prefix_bytes += read["bytes"]
            if prefix_bytes >= 2:
                first_pcm = read["seconds"]
                break
        gaps = [b["seconds"] - a["seconds"] for a, b in zip(reads, reads[1:])]
        result.update(status="complete", bytes=len(data), sample_rate=rate, audio_seconds=duration,
                      first_pcm_seconds=first_pcm, last_pcm_seconds=reads[-1]["seconds"], total_seconds=elapsed,
                      total_rtf=elapsed / duration, max_network_read_gap_seconds=max(gaps, default=0),
                      sha256=hashlib.sha256(data).hexdigest(), reads=reads,
                      note="Network reads are not model chunks; correlate NAI_TTS_CHUNK logs. Not playback evidence.")
        with wave.open(str(args.output_dir / "speech.wav"), "wb") as output:
            output.setnchannels(1)
            output.setsampwidth(2)
            output.setframerate(rate)
            output.writeframes(data)
    except Exception as error:
        result.update(error=f"{type(error).__name__}: {error}", partial_bytes=len(data), total_seconds=time.monotonic() - started)
    finally:
        conn.close()
        (args.output_dir / "result.json").write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({k: v for k, v in result.items() if k != "reads"}, ensure_ascii=False, indent=2))
    return 0 if result["status"] in ("complete", "disconnected") else 1


if __name__ == "__main__":
    raise SystemExit(main())
