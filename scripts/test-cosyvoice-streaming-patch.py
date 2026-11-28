"""Run prepared route helpers with no GPU/model import."""
import argparse
import ast
import asyncio
import logging
import inspect
import time
import unittest
from pathlib import Path
from types import SimpleNamespace
import numpy as np
from fastapi import FastAPI, File, Form, UploadFile, HTTPException
from fastapi.responses import StreamingResponse
from starlette.background import BackgroundTask
try:
    from nai_request_gate import RequestGate, BusyError
except ImportError:
    RequestGate = None


class Tensor:
    def __init__(self, data):
        self.data = np.array([data], dtype=np.float32)
    def detach(self):
        return self
    def cpu(self):
        return self
    def numpy(self):
        return self.data


class Tests(unittest.TestCase):
    def setUp(self):
        self.calls = []
        def model(*args, stream, **kwargs):
            self.calls.append((args, stream, kwargs))
            yield {"tts_speech": Tensor([0, 1, -1])}
            yield {"tts_speech": Tensor([0.5, -0.5])}
        fake = SimpleNamespace(sample_rate=24000, **{"inference_" + name: model for name in ("sft", "zero_shot", "cross_lingual", "instruct", "instruct2")})
        self.ns = dict(globals(), app=FastAPI(), cosyvoice=fake, load_wav=lambda f, rate: (f, rate))
        if RequestGate is not None:
            self.ns["request_gate"] = RequestGate(2)
        exec(compile(SERVICE_TREE, "prepared-helpers", "exec"), self.ns)

    def test_all_routes_stream_and_preserve_upload(self):
        async def run(name, args, stream):
            method = self.ns["inference_" + name]
            optional = {"zero_shot_spk_id": ""} if "zero_shot_spk_id" in inspect.signature(method).parameters else {}
            response = await method(*args, stream=stream, **optional)
            self.assertEqual(response.headers["x-sample-rate"], "24000")
            self.assertEqual(response.headers["x-audio-encoding"], "pcm_s16le")
            self.assertEqual(response.headers["x-model-stream"], str(stream).lower())
            output = [chunk async for chunk in response.body_iterator]
            self.assertEqual(self.calls[-1][1], stream)
            if name in ("zero_shot", "cross_lingual", "instruct2"):
                self.assertIn(("reference", 16000), self.calls[-1][0])
            self.assertEqual(len(output), 2)
            self.assertEqual(np.frombuffer(output[0], dtype="<i2").tolist(), [0, 32767, -32768])
            self.calls.clear()
        upload = SimpleNamespace(file="reference")
        routes = {"sft": ("text", "speaker"), "zero_shot": ("text", "prompt", upload),
                  "cross_lingual": ("text", upload), "instruct": ("text", "speaker", "style"),
                  "instruct2": ("text", "style", upload)}
        for name, args in routes.items():
            for stream in (True, False):
                with self.subTest(route=name, stream=stream):
                    asyncio.run(run(name, args, stream))

    def test_nonfinite_rejected(self):
        with self.assertRaises(ValueError):
            list(self.ns["generate_data"]([{"tts_speech": Tensor([float("nan")])}]))

    def test_first_chunk_precedes_remaining_generation(self):
        consumed = []
        def model():
            consumed.append(1)
            yield {"tts_speech": Tensor([0])}
            consumed.append(2)
            raise RuntimeError("later failure")
        result = self.ns["generate_data"](model())
        self.assertEqual(next(result), b"\0\0")
        self.assertEqual(consumed, [1])
        with self.assertRaises(RuntimeError):
            next(result)

    def test_stereo_rejected(self):
        audio = Tensor([0])
        audio.data = np.zeros((2, 10), dtype=np.float32)
        with self.assertRaises(ValueError):
            list(self.ns["generate_data"]([{"tts_speech": audio}]))

    def test_pcm_rejection_closes_upstream_generator(self):
        closed = []
        def model():
            try:
                yield {"tts_speech": Tensor([float("nan")])}
            finally:
                closed.append(True)
        with self.assertRaises(ValueError):
            list(self.ns["generate_data"](model()))
        self.assertEqual(closed, [True])

    def test_cached_voice_branch_is_preserved_without_upload(self):
        method = self.ns["inference_zero_shot"]
        if "zero_shot_spk_id" not in inspect.signature(method).parameters:
            self.skipTest("this entry has no cached-voice API")
        async def run():
            response = await method("text", "", None, zero_shot_spk_id="fixture-voice", stream=True)
            output = [chunk async for chunk in response.body_iterator]
            self.assertEqual(len(output), 2)
            self.assertEqual(self.calls[-1][2], {"zero_shot_spk_id": "fixture-voice"})
        asyncio.run(run())


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("service", type=Path)
    args = parser.parse_args()
    tree = ast.parse(args.service.read_text(encoding="utf-8-sig"))
    SERVICE_TREE = ast.fix_missing_locations(ast.Module(body=[n for n in tree.body if isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef))], type_ignores=[]))
    unittest.main(argv=[__file__], verbosity=2)
