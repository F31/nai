from pathlib import Path
import sys
import time
from types import SimpleNamespace as NS
import unittest
from unittest.mock import patch

root=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(root/'scripts'))
sys.path.insert(0,str(root/'packages/contracts-python/src/generated'))
from avatar_compiled_warmup import warmup


class WarmupTests(unittest.TestCase):
    def config(self):
        return {'warmup_samples':76800,'frame_num':49,'maintenance_timeout_ms':240000}

    def test_only_completed_internal_frames_and_compile_evidence_become_ready(self):
        def render(pcm,job,emit):
            self.assertEqual(len(pcm),153600)
            self.assertFalse(any(pcm))
            self.assertEqual(job.request.epoch,0)
            emit(job.request,'local-only',NS(pool_id='pool',epoch=0,sequence=0,slot_index=0,slot_generation=1))
            self.assertEqual(job.releases.get_nowait().epoch,0)
            return 80,NS(retained_frames=80,blocks=2,block_frames=40,motion_frames=9)
        backend=NS(torch=None,pipeline=None,render=render)
        modules={'torch._dynamo.utils':NS(counters={'stats':{'unique_graphs':13}}),
                 'torch._inductor':NS(metrics=NS(generated_kernel_count=300))}
        with patch('avatar_compile.install') as install, patch.dict(sys.modules,modules):
            result=warmup(backend,self.config(),time.monotonic())
            install.assert_called_once_with(None,None,eager_sdpa=True,blocks=True)
            self.assertEqual((result.frames_acked,result.blocks,result.unique_graphs),(80,2,13))
            backend.render=lambda *_:(80,NS(retained_frames=80,blocks=1))
            with self.assertRaisesRegex(ValueError,'WARMUP_FRAMES'): warmup(backend,self.config(),time.monotonic())
            modules['torch._inductor'].metrics.generated_kernel_count=0
            with self.assertRaisesRegex(RuntimeError,'NO_COMPILED'): warmup(backend,self.config(),time.monotonic())

    def test_invalid_or_expired_maintenance_never_installs(self):
        with patch('avatar_compile.install') as install:
            config=self.config();config['warmup_samples']=960
            with self.assertRaisesRegex(ValueError,'WARMUP_SAMPLES'): warmup(None,config,time.monotonic())
            with self.assertRaises(TimeoutError): warmup(None,self.config(),time.monotonic()-241)
            install.assert_not_called()


if __name__=='__main__':unittest.main()
