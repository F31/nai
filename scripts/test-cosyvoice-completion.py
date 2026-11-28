"""CPU-only tests of application generation completion, no model/API calls."""
import asyncio
import hashlib
import threading
import time
import unittest

from cosyvoice_request_gate import RequestGate, GenerationError


async def consume(job):
    return b"".join([chunk async for chunk in job.iter_bytes()])


class CompletionTests(unittest.TestCase):
    def test_non_finite_deadlines_are_rejected(self):
        for value in (float('nan'), float('inf'), 0, -1):
            with self.assertRaises(ValueError):
                RequestGate(value)

    def finish(self, factory, **options):
        job = RequestGate(2, **options).start(factory)
        data = asyncio.run(consume(job))
        self.assertTrue(job.finished.wait(1))
        return job, data

    def test_complete_hash_and_post_eos_cleanup(self):
        job, data = self.finish(lambda: iter([b"\x01\x00", b"\x02\x00"]))
        expected = dict(state="completed", samples=2, sha256=hashlib.sha256(data).hexdigest())
        self.assertEqual(job.completion(), expected)
        job.cancel()
        self.assertFalse(job.cancelled.is_set())
        copy = job.completion()
        copy["state"] = "failed"
        self.assertEqual(job.completion(), expected)

    def test_empty_is_not_complete(self):
        job, _ = self.finish(lambda: iter([]))
        self.assertEqual(job.completion()["state"], "failed")

    def test_partial_then_exception_is_not_complete(self):
        def broken():
            yield b"\0\0"
            raise RuntimeError("private backend detail")
        job = RequestGate(2).start(broken)
        with self.assertRaises(GenerationError):
            asyncio.run(consume(job))
        self.assertTrue(job.finished.wait(1))
        self.assertEqual(job.completion()["state"], "failed")
        self.assertNotIn("private", str(job.completion()))

    def test_cancel_remains_cancelled_after_drain(self):
        release = threading.Event()
        def slow():
            release.wait(1)
            yield b"\0\0"
        gate = RequestGate(2)
        job = gate.start(slow)
        self.assertIsNone(job.completion())
        job.cancel()
        self.assertTrue(gate.status()["busy"])
        release.set()
        self.assertTrue(job.finished.wait(1))
        self.assertEqual(job.completion()["state"], "cancelled")
        self.assertFalse(gate.status()["busy"])

    def test_cleanup_failure_overrides_normal_iteration(self):
        class BrokenClose:
            def __iter__(self):
                return self
            def __next__(self):
                raise StopIteration
            def close(self):
                raise RuntimeError("cleanup")
        job = RequestGate(2).start(BrokenClose)
        with self.assertRaises(GenerationError):
            asyncio.run(consume(job))
        self.assertEqual(job.completion()["state"], "failed")

    def test_final_block_after_deadline_is_not_complete(self):
        def late():
            time.sleep(0.04)
            yield b"\0\0"
        job = RequestGate(0.01).start(late)
        self.assertTrue(job.finished.wait(1))
        self.assertNotEqual(job.completion()["state"], "completed")

    def test_odd_and_sample_limit_fail(self):
        for chunks in ([b"x"], [b"\0\0" * 3]):
            job = RequestGate(2, max_samples=2).start(lambda: iter(chunks))
            with self.assertRaises(GenerationError):
                asyncio.run(consume(job))
            self.assertEqual(job.completion()["state"], "failed")


if __name__ == "__main__":
    unittest.main(verbosity=2)
