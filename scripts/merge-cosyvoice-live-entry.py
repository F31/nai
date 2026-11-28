"""Merge verified streaming helpers into the inspected voice-ID service revision.

Preserves /spks and both zero-shot branches. Writes candidates only, no deploy.
"""
import argparse
import ast
import runpy
from pathlib import Path


def prepare(source, verified):
    tree = ast.parse(source)
    existing = {n.name: n for n in tree.body if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef))}
    borrowed = {n.name: n for n in ast.parse(verified).body if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef))}
    if "health" in existing or "pcm_response" in existing or "request_gate" in source:
        raise ValueError("entry is already modified or structurally unknown")
    if "list_spks" not in existing:
        raise ValueError("expected the inspected voice-ID revision")
    changes = []
    for name in ("inference_sft", "inference_zero_shot", "inference_cross_lingual", "inference_instruct", "inference_instruct2"):
        node = existing[name]
        if any(arg.arg == "stream" for arg in node.args.args):
            raise ValueError("partial streaming entry must be reviewed")
        node.args.args.append(ast.arg(arg="stream", annotation=ast.Name(id="bool", ctx=ast.Load())))
        node.args.defaults.append(ast.Call(func=ast.Name(id="Form", ctx=ast.Load()), args=[ast.Constant(True)], keywords=[]))
        calls = 0
        responses = 0
        for call in ast.walk(node):
            if not isinstance(call, ast.Call):
                continue
            if isinstance(call.func, ast.Attribute) and isinstance(call.func.value, ast.Name) and call.func.value.id == "cosyvoice" and call.func.attr == name:
                if any(k.arg == "stream" for k in call.keywords):
                    raise ValueError("unexpected existing stream argument")
                call.keywords.append(ast.keyword(arg="stream", value=ast.Name(id="stream", ctx=ast.Load())))
                calls += 1
            if isinstance(call.func, ast.Name) and call.func.id == "StreamingResponse":
                call.func.id = "pcm_response"
                call.args = [ast.Name(id="model_output", ctx=ast.Load()), ast.Name(id="stream", ctx=ast.Load())]
                responses += 1
        if calls != (2 if name == "inference_zero_shot" else 1) or responses != 1:
            raise ValueError("inference route structure changed")
        start = min([node.lineno] + [n.lineno for n in node.decorator_list])
        changes.append((start - 1, node.end_lineno, ast.unparse(ast.fix_missing_locations(node)) + "\n"))
    old = existing["generate_data"]
    helpers = "\n\n".join(ast.unparse(borrowed[n]) for n in ("generate_data", "pcm_response", "health")) + "\n"
    changes.append((old.lineno - 1, old.end_lineno, helpers))
    lines = source.splitlines(keepends=True)
    for start, end, code in sorted(changes, reverse=True):
        lines[start:end] = [code]
    result = "".join(lines)
    if result.count("app = FastAPI()") != 1:
        raise ValueError("unknown app construction")
    result = result.replace("import os\n", "import os\nimport time\nfrom nai_request_gate import RequestGate, BusyError\nfrom starlette.background import BackgroundTask\n", 1)
    result = result.replace("from fastapi import FastAPI, UploadFile, Form, File", "from fastapi import FastAPI, UploadFile, Form, File, HTTPException", 1)
    result = result.replace("app = FastAPI()", "app = FastAPI()\nrequest_gate = RequestGate(float(os.environ.get('COSYVOICE_REQUEST_DEADLINE_SECONDS', '60')))", 1)
    compile(result, "merged-cosyvoice-service", "exec")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for arg in ("server", "model", "verified-server", "output-dir"):
        parser.add_argument("--" + arg, type=Path, required=True)
    args = parser.parse_args()
    server = prepare(args.server.read_text(encoding="utf-8"), args.verified_server.read_text(encoding="utf-8"))
    module = runpy.run_path(str(Path(__file__).with_name("prepare-cosyvoice-request-isolation.py")))
    model = module["patch_model"](args.model.read_text(encoding="utf-8"))
    args.output_dir.mkdir(parents=True, exist_ok=False)
    (args.output_dir / "server.py").write_text(server, encoding="utf-8")
    (args.output_dir / "model.py").write_text(model, encoding="utf-8")
    print("Prepared voice-ID preserving merge; not activated")


if __name__ == "__main__":
    main()
