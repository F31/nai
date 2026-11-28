import hashlib
from pathlib import Path
import sys
from types import SimpleNamespace as NS
import unittest

root=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(root/'scripts'))
sys.path.insert(0,str(root/'packages/contracts-python/src/generated'))
import numpy as np
from v561.media_pb2 import FrameRelease
from avatar_worker_core import Job,Cancelled
from avatar_owner_frames import OwnerFrames

class FrameTests(unittest.TestCase):
    def test_exact_release_and_real_shared_bytes(self):
        from multiprocessing import shared_memory
        job=Job(NS(epoch=7),2)
        def emit(request,name,frame):
            memory=shared_memory.SharedMemory(name=name)
            try:
                offset=frame.slot_index*frame.payload_bytes
                pixels=bytes(memory.buf[offset:offset+frame.payload_bytes])
                self.assertEqual(hashlib.sha256(pixels).digest(),frame.content_sha256)
                self.assertEqual(pixels[:4],bytes([3,2,1,255]))
            finally: memory.close()
            job.releases.put_nowait(FrameRelease(pool_id=frame.pool_id,epoch=frame.epoch,sequence=frame.sequence,slot_index=frame.slot_index,slot_generation=frame.slot_generation))
        frames=OwnerFrames(256,256,job,emit)
        try:
            for _ in range(4):frames.send_rgb(np.tile(np.array([1,2,3],dtype=np.uint8),(256,256,1)))
            self.assertEqual(frames.finish()['frames_acked'],4)
        finally:frames.abort()

    def test_wrong_release_does_not_reuse_slot(self):
        job=Job(NS(epoch=7),2)
        def emit(request,name,frame):job.releases.put_nowait(FrameRelease(pool_id=frame.pool_id,epoch=7,sequence=1,slot_index=0,slot_generation=1))
        frames=OwnerFrames(256,256,job,emit)
        try:
            with self.assertRaises(ValueError):frames.send_rgb(np.zeros((256,256,3),dtype=np.uint8))
            self.assertEqual(frames.sent,0)
        finally:frames.abort()

    def test_cancel_during_release_wait_discards_result(self):
        job=Job(NS(epoch=7),2)
        frames=OwnerFrames(256,256,job,lambda *_:job.cancelled.set())
        try:
            with self.assertRaises(Cancelled):frames.send_rgb(np.zeros((256,256,3),dtype=np.uint8))
            self.assertEqual(frames.sent,0)
        finally:frames.abort()

if __name__=='__main__':unittest.main()
