from pathlib import Path
import sys
from types import SimpleNamespace as NS
from unittest.mock import Mock
import unittest
import torch

sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'scripts'))
from avatar_fp8_ffn_candidate import candidate


class CandidateTests(unittest.TestCase):
    def setUp(self):
        self.sources=[torch.nn.Sequential(torch.nn.Linear(16,32),torch.nn.GELU(approximate='tanh'),
            torch.nn.Linear(32,16)).eval() for _ in range(2)]
        self.raw=[NS(ffn=source) for source in self.sources]
        self.blocks=[NS(_orig_mod=raw) for raw in self.raw]
        self.pipeline=NS(model=NS(blocks=self.blocks.copy()),vae=object())
        self.compile=Mock(side_effect=lambda raw,**kwargs:NS(_orig_mod=raw))
        self.torch=NS(nn=torch.nn,no_grad=torch.no_grad,compile=self.compile)
        class Fixture(torch.nn.Module):
            def __init__(self,source):
                super().__init__();self.source=source
                self.cache_bytes=source[0].weight.numel()+source[2].weight.numel()+8
            def forward(self,x):return self.source(x)
        self.factory=Fixture

    def assert_restored(self):
        self.assertEqual(self.pipeline.model.blocks,self.blocks)
        for raw,source in zip(self.raw,self.sources):self.assertIs(raw.ffn,source)

    def test_scoped_install_and_restore_all_original_objects(self):
        with candidate(self.torch,self.pipeline,[0,1],4096,self.factory) as stats:
            self.assertEqual(stats['static_cache_bytes'],2064)
            self.assertIsNot(self.raw[0].ffn,self.sources[0])
            self.assertIsNot(self.pipeline.model.blocks[0],self.blocks[0])
        self.assert_restored()
        for call in self.compile.call_args_list:self.assertEqual(call.kwargs,{'dynamic':False})

    def test_partial_install_failure_rolls_back_ffns_and_wrappers(self):
        self.compile.side_effect=[object(),RuntimeError('compile failed')]
        with self.assertRaises(RuntimeError):
            with candidate(self.torch,self.pipeline,[0,1],4096,self.factory):pass
        self.assert_restored()

    def test_budget_admission_occurs_before_factory_and_source_changes_detected(self):
        build=Mock()
        with self.assertRaisesRegex(ValueError,'FP8_FFN_CACHE_BYTES'):
            with candidate(self.torch,self.pipeline,[0,1],1,build):pass
        build.assert_not_called();self.assert_restored()
        with self.assertRaisesRegex(ValueError,'FP8_FFN_SOURCE_CHANGED'):
            with candidate(self.torch,self.pipeline,[0],4096,self.factory),torch.no_grad():
                self.sources[0][0].weight.add_(1)
        self.assert_restored()

    def test_duplicate_selection_rejected(self):
        with self.assertRaisesRegex(ValueError,'FP8_FFN_SELECTION'):
            with candidate(self.torch,self.pipeline,[0,0],4096,self.factory):pass
        self.assert_restored()


if __name__=='__main__':unittest.main()
