"""Fault tests for actual patched model method and stdlib request gate; no GPU."""
import argparse
import ast
import asyncio
import importlib.util
import os
import sys
import threading
import time
import unittest
import uuid
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
import numpy as np
import torch as cpu_torch


def module(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


async def collect(job):
    return [chunk async for chunk in job.iter_bytes()]


class GateTests(unittest.TestCase):
    def test_concurrent_is_rejected_without_invoking_factory(self):
        release = threading.Event()
        gate = GATE.RequestGate(2)
        def slow():
            release.wait(1)
            yield b"\0\0"
        job = gate.start(slow)
        called = []
        try:
            with self.assertRaises(GATE.BusyError):
                gate.start(lambda: called.append(True))
            self.assertEqual(called, [])
            self.assertTrue(gate.status()["busy"])
        finally:
            release.set()
        self.assertEqual(asyncio.run(collect(job)), [b"\0\0"])
        self.assertTrue(job.finished.wait(1))

    def test_disconnect_retains_admission_until_model_finishes(self):
        release = threading.Event()
        gate = GATE.RequestGate(2)
        def slow():
            yield b"\0\0"
            release.wait(1)
            yield b"\1\0"
        job = gate.start(slow)
        async def disconnect():
            iterator = job.iter_bytes()
            self.assertEqual(await anext(iterator), b"\0\0")
            await iterator.aclose()
        try:
            asyncio.run(disconnect())
            self.assertTrue(job.cancelled.is_set())
            with self.assertRaises(GATE.BusyError):
                gate.start(lambda: iter(()))
        finally:
            release.set()
        self.assertTrue(job.finished.wait(1))
        self.assertFalse(gate.status()["busy"])
        next_job = gate.start(lambda: iter([b"\2\0"]))
        self.assertEqual(asyncio.run(collect(next_job)), [b"\2\0"])

    def test_deadline_does_not_release_a_stuck_backend(self):
        release = threading.Event()
        gate = GATE.RequestGate(0.06)
        def stuck():
            release.wait(1)
            yield b"\0\0"
        job = gate.start(stuck)
        try:
            with self.assertRaises(GATE.GenerationError):
                asyncio.run(collect(job))
            self.assertTrue(gate.status()["deadline_exceeded"])
            with self.assertRaises(GATE.BusyError):
                gate.start(lambda: iter(()))
        finally:
            release.set()
        self.assertTrue(job.finished.wait(1))

    def test_model_error_is_not_success_and_admission_recovers(self):
        def broken():
            raise RuntimeError("failure")
            yield b""
        gate = GATE.RequestGate(2)
        with self.assertRaises(GATE.GenerationError):
            asyncio.run(collect(gate.start(broken)))
        self.assertFalse(gate.status()["busy"])
        self.assertEqual(asyncio.run(collect(gate.start(lambda: iter([b"\0\0"])))), [b"\0\0"])

    def test_bounded_queue_and_oversized_chunk(self):
        gate = GATE.RequestGate(2, queue_chunks=1, max_chunk_bytes=4)
        made = []
        def many():
            for _ in range(20):
                made.append(1)
                yield b"\0\0"
        job = gate.start(many)
        time.sleep(0.1)
        self.assertLessEqual(len(made), 2)
        self.assertLessEqual(job.queue.qsize(), 1)
        job.cancel()
        self.assertTrue(job.finished.wait(1))
        with self.assertRaises(GATE.GenerationError):
            asyncio.run(collect(gate.start(lambda: iter([b"x" * 6]))))

    def test_asgi_disconnect_cancels_delivery_and_keeps_backend_busy(self):
        from starlette.responses import StreamingResponse
        from starlette.background import BackgroundTask
        release = threading.Event()
        gate = GATE.RequestGate(2)
        def slow():
            yield b"\0\0"
            release.wait(1)
            yield b"\0\0"
        job = gate.start(slow)
        async def exercise():
            sent = asyncio.Event()
            async def send(message):
                if message["type"] == "http.response.body":
                    sent.set()
            async def receive():
                await sent.wait()
                return {"type": "http.disconnect"}
            response = StreamingResponse(job.iter_bytes(), background=BackgroundTask(job.cancel))
            await response({"type": "http", "asgi": {"spec_version": "2.0"}}, receive, send)
        try:
            asyncio.run(exercise())
            self.assertTrue(job.cancelled.is_set())
            self.assertTrue(gate.status()["busy"])
        finally:
            release.set()
        self.assertTrue(job.finished.wait(1))


class ModelTests(unittest.TestCase):
    def model(self):
        obj = SimpleNamespace(token_hop_len=25, token_max_hop_len=100, stream_scale_factor=2,
                              lock=threading.Lock(), tts_speech_token_dict={}, llm_end_dict={}, hift_cache_dict={},
                              flow=SimpleNamespace(pre_lookahead_len=3))
        obj.calls = []
        def llm(text, prompt_text, speech, embedding, key):
            obj.tts_speech_token_dict[key] = list(range(120))
        def decode(**kwargs):
            obj.calls.append((kwargs["token_offset"], kwargs["token"].shape[1], kwargs["finalize"]))
            return cpu_torch.zeros(1, 2)
        obj.llm_job = llm
        obj.vc_job = lambda *args: None
        obj.token2wav = decode
        return obj

    def check_clean(self, model):
        for name in ("tts_speech_token_dict", "llm_end_dict", "hift_cache_dict"):
            self.assertEqual(getattr(model, name), {})

    def test_three_requests_do_not_grow_model_default(self):
        obj = self.model()
        first = []
        for _ in range(3):
            obj.calls.clear()
            list(TTS(obj, stream=True))
            first.append(obj.calls[0])
            self.assertEqual(obj.token_hop_len, 25)
            self.check_clean(obj)
        self.assertEqual(first, [(0, 28, False)] * 3)

    def test_explicit_initial_hop_and_invalid_setting(self):
        obj = self.model()
        with patch.dict(os.environ, {"COSYVOICE_INITIAL_HOP_TOKENS": "50"}):
            list(TTS(obj, stream=True))
        self.assertEqual(obj.calls[0], (0, 53, False))
        with patch.dict(os.environ, {"COSYVOICE_INITIAL_HOP_TOKENS": "26"}):
            with self.assertRaises(ValueError):
                list(TTS(obj, stream=True))
        self.check_clean(obj)

    def test_token_worker_failure_propagates_and_cleans(self):
        obj = self.model()
        def broken(*args):
            raise RuntimeError("token failure")
        obj.llm_job = broken
        with self.assertRaises(RuntimeError):
            list(TTS(obj, stream=True))
        self.check_clean(obj)

    def test_decoder_failure_and_generator_close_clean(self):
        obj = self.model()
        def broken(**kwargs):
            raise RuntimeError("decoder failure")
        original = obj.token2wav
        obj.token2wav = broken
        with self.assertRaises(RuntimeError):
            list(TTS(obj, stream=True))
        self.check_clean(obj)
        obj.token2wav = original
        generator = TTS(obj, stream=True)
        next(generator)
        generator.close()
        self.check_clean(obj)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", required=True, type=Path)
    parser.add_argument("--gate", required=True, type=Path)
    args = parser.parse_args()
    GATE = module(args.gate, "nai_gate_test")
    tree = ast.parse(args.model.read_text())
    cls = next(n for n in tree.body if isinstance(n, ast.ClassDef) and n.name == "CosyVoice2Model")
    method = next(n for n in cls.body if isinstance(n, ast.FunctionDef) and n.name == "tts")
    scope = dict(globals(), torch=SimpleNamespace(zeros=cpu_torch.zeros, tensor=cpu_torch.tensor,
                                                int32=cpu_torch.int32, cuda=SimpleNamespace(is_available=lambda: False)))
    exec(compile(ast.fix_missing_locations(ast.Module(body=[method], type_ignores=[])), "patched-model-method", "exec"), scope)
    TTS = scope["tts"]
    unittest.main(argv=[__file__], verbosity=2)
