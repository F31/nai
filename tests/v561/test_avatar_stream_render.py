"""Explicit CPU test double for lifecycle; never real GPU acceptance."""
from pathlib import Path
import queue
import sys
import threading
from types import SimpleNamespace as NS
import unittest
from unittest.mock import patch

ROOT=Path(__file__).resolve().parents[2]
sys.path[:0]=[str(ROOT/'scripts'),str(ROOT/'packages/contracts-python/src/generated')]
import numpy as np
import torch
from avatar_stream_render import StreamRender
from avatar_worker_core import Job, Cancelled
from v561.media_pb2 import FrameRelease


class StreamRenderTests(unittest.TestCase):
    def test_portrait_posterior_is_seeded_before_initial_cache(self):
        from flashhead_resident_backend import Backend
        calls=[]
        backend=object.__new__(Backend)
        backend.config=dict(seed=42);backend.pipeline=object()
        backend.torch=NS(manual_seed=lambda seed:calls.append(('seed',seed)))
        backend.inference=NS(get_base_data=lambda *args:calls.append(('portrait',args)))
        backend._prepare_base_data('authorized.png')
        self.assertEqual(calls[0],('seed',42))
        self.assertEqual(calls[1][0],'portrait')
        self.assertEqual(calls[1][1][1:],('authorized.png',42,False))
    def setUp(self):
        self.sync=patch('torch.cuda.synchronize',lambda:None);self.sync.start();self.addCleanup(self.sync.stop)
        self.resets=[];self.histories=[]
        def infer(history):
            self.histories.append(list(history))
            return torch.zeros((2,2,2,3),dtype=torch.float32)
        self.backend=NS(np=np,torch=torch,config=dict(seed=42,height=2,width=2),
            params=dict(sample_rate=16000,tgt_fps=25,frame_num=11,motion_frames_num=9,cached_audio_duration=1,sample_steps=4),
            pipeline=NS(person_name='fixture',reset_person_name=lambda name:self.resets.append(name),generator=torch.Generator()),
            _infer_block=infer,report_compute_profile=lambda epoch,block:None)

    def stream(self, emit=None):
        job=Job(NS(epoch=1),4)
        def accept(request,mapping,frame):
            job.releases.put_nowait(FrameRelease(pool_id=frame.pool_id,epoch=frame.epoch,sequence=frame.sequence,
                slot_index=frame.slot_index,slot_generation=frame.slot_generation))
        stream=StreamRender(self.backend,job,emit or accept,144000)
        self.addCleanup(stream.close)
        return stream,job

    def test_history_and_seed_reset_only_once_and_frames_are_exact(self):
        stream,job=self.stream()
        stream.push(bytes(7680));stream.push(bytes(7680))
        self.assertGreater(stream.sent,0)
        count,stats=stream.finish(7680)
        self.assertEqual(count,8);self.assertEqual(stats.feature_samples,5120)
        self.assertEqual(self.resets,['fixture']);self.assertTrue(all(len(x)==16000 for x in self.histories))
        self.assertTrue(stream.closed);self.assertFalse(self.backend._stream_active)
        self.assertFalse(job.cancelled.is_set())

    def test_short_eos_trims_generated_tail(self):
        stream,_=self.stream();stream.push(b'\x01\x00')
        self.assertEqual(stream.sent,0)
        count,stats=stream.finish(1)
        self.assertEqual((count,stats.generated_frames,stats.tail_frames),(1,2,1))
        self.assertEqual(stats.feature_samples,1)

    def test_bad_eos_cancels_and_releases_exclusive_session(self):
        stream,job=self.stream();stream.push(bytes(2))
        with self.assertRaises(ValueError):stream.finish(2)
        self.assertTrue(job.cancelled.is_set());self.assertFalse(self.backend._stream_active)
        recovered,_=self.stream();recovered.push(bytes(2));self.assertEqual(recovered.finish(1)[0],1)

    def test_second_stream_rejected_until_owner_cleanup(self):
        stream,_=self.stream()
        with self.assertRaisesRegex(ValueError,'GPU_STREAM_BUSY'):self.stream()
        stream.close();recovered,_=self.stream();recovered.close()

    def test_delivery_cancel_does_not_return_complete(self):
        holder={}
        def emit(*args):holder['job'].cancelled.set()
        stream,job=self.stream(emit);holder['job']=job
        with self.assertRaises(Cancelled):
            stream.push(bytes(15360));stream.finish(7680)
        self.assertTrue(stream.closed);self.assertFalse(self.backend._stream_active)
        self.assertFalse(stream.finished)

    def test_control_thread_may_cancel_but_cannot_release_gpu_owner(self):
        stream,job=self.stream();errors=[]
        def wrong_owner():
            try:stream.close()
            except RuntimeError as e:errors.append(str(e))
        thread=threading.Thread(target=wrong_owner);thread.start();thread.join()
        self.assertEqual(errors,['GPU_STREAM_OWNER'])
        self.assertTrue(self.backend._stream_active);self.assertTrue(job.cancelled.is_set())
        stream.close();self.assertFalse(self.backend._stream_active)


if __name__=='__main__':unittest.main()
