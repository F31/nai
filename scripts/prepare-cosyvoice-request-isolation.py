"""Prepare strict model/server request-isolation patches; never activate them."""
import argparse
import ast
from pathlib import Path


def replace_once(source, old, new, count=1):
    if source.count(old) != count:
        raise ValueError(f"unsupported source: expected {count} occurrences of {old!r}")
    return source.replace(old, new)


def patch_model(source):
    tree = ast.parse(source)
    cls = next(n for n in tree.body if isinstance(n, ast.ClassDef) and n.name == "CosyVoice2Model")
    method = next(n for n in cls.body if isinstance(n, ast.FunctionDef) and n.name == "tts")
    lines = source.splitlines(keepends=True)
    original = "".join(lines[method.lineno - 1:method.end_lineno])
    # Scope mutation to this generator only, retaining the immutable model default.
    body = original.replace("self.token_hop_len", "request_hop_len")
    body = replace_once(body, "        # this_uuid is used", '''        base_hop = self.token_hop_len
        request_hop_len = int(os.environ.get("COSYVOICE_INITIAL_HOP_TOKENS", str(base_hop)))
        if request_hop_len < base_hop or request_hop_len > self.token_max_hop_len or request_hop_len % base_hop:
            raise ValueError("initial hop must be a supported multiple of the model base hop")
        # this_uuid is used''')
    body = replace_once(body, "        if source_speech_token.shape[1] == 0:", '''        generation_errors = []
        def generation_job(job, args):
            try:
                job(*args)
            except BaseException as error:
                generation_errors.append(error)
            finally:
                self.llm_end_dict[this_uuid] = True
        if source_speech_token.shape[1] == 0:''')
    body = replace_once(body, "threading.Thread(target=self.llm_job, args=(text, prompt_text, llm_prompt_speech_token, llm_embedding, this_uuid))", "threading.Thread(target=generation_job, args=(self.llm_job, (text, prompt_text, llm_prompt_speech_token, llm_embedding, this_uuid)))")
    body = replace_once(body, "threading.Thread(target=self.vc_job, args=(source_speech_token, this_uuid))", "threading.Thread(target=generation_job, args=(self.vc_job, (source_speech_token, this_uuid)))")
    body = replace_once(body, "            p.join()", "            p.join()\n            if generation_errors:\n                raise RuntimeError('audio token generation failed') from generation_errors[0]", 2)
    start = body.index("        if stream is True:")
    cleanup = body.index("        with self.lock:", start)
    generation = body[start:cleanup]
    tail = body[cleanup:]
    # Even early close/token2wav failure waits for the actual token worker before
    # retiring its state. A stuck worker retains single-flight admission.
    for name in ("tts_speech_token_dict", "llm_end_dict", "hift_cache_dict"):
        tail = replace_once(tail, f"self.{name}.pop(this_uuid)", f"self.{name}.pop(this_uuid, None)")
    prefix = replace_once(body[:start], "        p.start()\n", "")
    body = prefix + "        try:\n            p.start()\n" + "".join("    " + line if line.strip() else line for line in generation.splitlines(keepends=True)) + "        finally:\n            if p.ident is not None:\n                p.join()\n" + "".join("    " + line if line.strip() else line for line in tail.splitlines(keepends=True))
    result = "".join(lines[:method.lineno - 1]) + body + "".join(lines[method.end_lineno:])
    compile(result, "cosyvoice-model-isolated", "exec")
    return result


def patch_server(source):
    tree = ast.parse(source)
    method = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == "generate_data")
    lines = source.splitlines(keepends=True)
    method_body = "".join(lines[method.body[0].lineno - 1:method.end_lineno])
    cleanup = "    finally:\n        if hasattr(model_output, 'close'):\n            model_output.close()\n"
    source = "".join(lines[:method.body[0].lineno - 1]) + "    try:\n" + "".join("    " + line if line.strip() else line for line in method_body.splitlines(keepends=True)) + cleanup + "".join(lines[method.end_lineno:])
    source = replace_once(source, "import time\n", "import time\nfrom nai_request_gate import RequestGate, BusyError\nfrom starlette.background import BackgroundTask\n")
    source = replace_once(source, "from fastapi import FastAPI, UploadFile, Form, File", "from fastapi import FastAPI, UploadFile, Form, File, HTTPException")
    source = replace_once(source, "app = FastAPI()", "app = FastAPI()\nrequest_gate = RequestGate(float(os.environ.get('COSYVOICE_REQUEST_DEADLINE_SECONDS', '60')))\n\n\n@app.get('/health')\nasync def health():\n    status = request_gate.status()\n    return {'status': 'blocked' if status['deadline_exceeded'] else 'ok', **status, 'sample_rate': cosyvoice.sample_rate}\n")
    old = '    return StreamingResponse(generate_data(model_output), media_type="application/octet-stream",'
    new = '''    try:
        job = request_gate.start(lambda: generate_data(model_output))
    except BusyError:
        raise HTTPException(status_code=409, detail="TTS busy; request was not queued")
    return StreamingResponse(job.iter_bytes(), background=BackgroundTask(job.cancel), media_type="application/octet-stream",'''
    source = replace_once(source, old, new)
    compile(source, "cosyvoice-server-isolated", "exec")
    return source


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", required=True, type=Path)
    parser.add_argument("--server", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    args = parser.parse_args()
    model = patch_model(args.model.read_text(encoding="utf-8"))
    server = patch_server(args.server.read_text(encoding="utf-8"))
    args.output_dir.mkdir(exist_ok=False, parents=True)
    (args.output_dir / "model.py").write_text(model, encoding="utf-8")
    (args.output_dir / "server.py").write_text(server, encoding="utf-8")
    print("Prepared model/server isolation; not deployed")


if __name__ == "__main__":
    main()
