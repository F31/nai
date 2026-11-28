from pathlib import Path
import sys
from types import SimpleNamespace as NS
import unittest
import torch

sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'scripts'))
from avatar_real_ffn_probe import Capture, render_facts
from avatar_fp8_ops import make_quantizer


class CaptureTests(unittest.TestCase):
    def test_eager_capture_differences_do_not_claim_fp8_visual_result(self):
        facts=render_facts([dict(mode='compiled-reference',frame_digests=['a']),
            dict(mode='ffn-capture-maintenance',frame_digests=['b']),
            dict(mode='compiled-confirm',frame_digests=['a'])])
        self.assertIsNone(facts['candidate_pixels_equal_baseline'])
        self.assertFalse(facts['fp8_outputs_rendered'])
        self.assertFalse(facts['capture_pixels_equal_baseline'])
        self.assertTrue(facts['restored_baseline_pixels_equal'])
    def setUp(self):
        self.ffn=torch.nn.Sequential(torch.nn.Linear(16,32,dtype=torch.bfloat16),
            torch.nn.GELU(approximate='tanh'),torch.nn.Linear(32,16,dtype=torch.bfloat16)).eval()
        self.pipeline=NS(model=NS(blocks=[NS(_orig_mod=NS(ffn=self.ffn))]))
        self.wrapper=self.pipeline.model.blocks[0]
        self.x=torch.ones(1,16,16,dtype=torch.bfloat16)

    def test_capture_keeps_actual_inputs_without_mutating_outputs_or_aliasing(self):
        with torch.no_grad():
            expected=self.ffn(self.x)
            with Capture(torch,self.pipeline,[0],[0,1],2,1024) as capture:
                self.assertTrue(torch.equal(self.ffn(self.x),expected))
                self.x.add_(1)
                self.ffn(self.x)
            self.assertEqual(len(capture.samples),2)
            self.assertTrue(torch.equal(capture.samples[0]['input'],torch.ones_like(self.x)))
            self.assertEqual(capture.bytes,1024)
            self.assertFalse(self.ffn._forward_pre_hooks)
            self.assertIs(self.pipeline.model.blocks[0],self.wrapper)
            capture.clear();self.assertEqual(capture.bytes,0)

    def test_budget_failure_and_call_overrun_restore_hooks_and_release_snapshots(self):
        for budget,calls,total in [(1,1,1),(1024,2,1)]:
            capture=Capture(torch,self.pipeline,[0],[0],total,budget)
            with torch.no_grad(),self.assertRaises(ValueError):
                with capture:
                    for _ in range(calls):self.ffn(self.x)
            self.assertFalse(self.ffn._forward_pre_hooks)
            self.assertIs(self.pipeline.model.blocks[0],self.wrapper)
            self.assertEqual(capture.samples,[])
        with self.assertRaises(ValueError):Capture(torch,self.pipeline,[1],[0],1,1024)

    def test_fake_tensor_cannot_count_as_actual_input(self):
        from torch._subclasses.fake_tensor import FakeTensorMode
        with FakeTensorMode():x=torch.ones(1,16,16,dtype=torch.bfloat16)
        with self.assertRaisesRegex(ValueError,'FFN_CAPTURE_FAKE_INPUT'):
            with Capture(torch,self.pipeline,[0],[0],1,1024):self.ffn(x)
        self.assertFalse(self.ffn._forward_pre_hooks)

    def test_zero_activation_quantizes_finitely_without_zero_scale(self):
        q,scale=make_quantizer(torch)(torch.zeros(2,16,dtype=torch.bfloat16))
        self.assertTrue(torch.isfinite(q.float()).all().item())
        self.assertGreater(scale.item(),0)
        self.assertEqual(q.float().abs().max().item(),0)


if __name__=='__main__':unittest.main()
