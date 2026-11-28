import hashlib
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace as NS
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
from avatar_worker_core import Owner, InvalidInput, Cancelled, pcm_snapshot

class WorkerCoreTests(unittest.TestCase):
    def test_cancel_retains_busy_until_cleanup(self):
        owner=Owner(1); owner.ready=True
        job=owner.admit(NS(task_id=1, epoch=1))
        self.assertFalse(owner.cancel(2, 1))
        self.assertTrue(owner.cancel(1, 1))
        with self.assertRaises(Cancelled): job.check()
        with self.assertRaises(InvalidInput): owner.admit(NS(task_id=2, epoch=2))
        owner.finish(job)
        with self.assertRaises(InvalidInput): owner.admit(NS(task_id=1, epoch=2))
        owner.admit(NS(task_id=2, epoch=2))

    def test_stop_never_reopens(self):
        owner=Owner(1); owner.ready=True
        job=owner.admit(NS(task_id=1, epoch=1)); owner.stop(); owner.finish(job)
        with self.assertRaises(Cancelled): job.check()
        with self.assertRaises(InvalidInput): owner.admit(NS(task_id=2, epoch=2))

    def test_pcm_identity_and_owned_snapshot(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'audio.pcm'; data=b'\x01\x00'*24; path.write_bytes(data)
            render=NS(pcm_path=str(path),sample_rate=24000,samples=24,pcm_sha256=hashlib.sha256(data).digest())
            snapshot=pcm_snapshot(render); path.write_bytes(b'\x00'*48)
            self.assertEqual(snapshot,data)
            with self.assertRaises(InvalidInput): pcm_snapshot(render)
            render.samples=144001
            with self.assertRaises(InvalidInput): pcm_snapshot(render)

if __name__=='__main__': unittest.main()
