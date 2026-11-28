from contextlib import nullcontext, ExitStack
from pathlib import Path
import sys
from types import SimpleNamespace as NS
from unittest.mock import Mock
import unittest

sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'scripts'))
from avatar_denoiser_graph import Replay,candidate


class Tensor:
    shape=(2,2);dtype='bf16';device=NS(type='cuda')
    def stride(self):return (2,1)
    def numel(self):return 4
    def element_size(self):return 2
    def clone(self,**kw):return Tensor()
    def copy_(self,x):pass


class GraphTests(unittest.TestCase):
    def setUp(self):
        self.graph=NS(replay=Mock(),reset=Mock())
        stream=NS(wait_stream=Mock())
        self.t=NS(is_tensor=lambda x:isinstance(x,Tensor),preserve_format=None,
            cuda=NS(memory_reserved=lambda:0,memory_allocated=lambda:0,current_stream=lambda:stream,Stream=lambda:stream,
                stream=lambda s:nullcontext(),CUDAGraph=lambda:self.graph,
                graph=lambda *a,**k:nullcontext(),synchronize=Mock()))
        self.values={k:Tensor() for k in ('x','timestep','context','y')}

    def test_cancel_before_capture_does_not_submit(self):
        replay=Replay(self.t,Mock(),100,Mock(side_effect=RuntimeError('cancel')))
        with self.assertRaisesRegex(RuntimeError,'cancel'):replay(**self.values)
        self.graph.replay.assert_not_called()

    def test_budget_rejects_before_allocating(self):
        replay=Replay(self.t,Mock(),39,lambda:None)
        with self.assertRaisesRegex(ValueError,'GRAPH_STATIC_BYTES'):replay(**self.values)
        self.assertIsNone(replay.inputs)

    def test_one_graph_reuse_output_owned_and_changed_shape_rejected(self):
        replay=Replay(self.t,lambda **kw:Tensor(),100,lambda:None)
        a=replay(**self.values);b=replay(**self.values)
        self.assertIsNot(a,b)
        self.assertEqual(replay.stats['captures'],1)
        self.assertEqual(replay.stats['replays'],2)
        self.values['x'].shape=(1,4)
        with self.assertRaisesRegex(ValueError,'GRAPH_INPUT_CHANGED'):replay(**self.values)
        self.assertEqual(replay.stats['replays'],2)
        replay.close();self.graph.reset.assert_called_once()

    def test_exception_restores_forward_and_releases_graph(self):
        class Model:
            def forward(self,**kw):return Tensor()
        model=Model()
        with self.assertRaisesRegex(RuntimeError,'failure'):
            with candidate(self.t,NS(model=model),100,lambda:None):
                model.forward(**self.values)
                raise RuntimeError('failure')
        self.assertNotIn('forward',vars(model))
        self.graph.reset.assert_called_once()

    def test_cancel_cached_graph_does_not_replay(self):
        check=Mock()
        replay=Replay(self.t,lambda **kw:Tensor(),100,check)
        replay(**self.values)
        check.side_effect=RuntimeError('cancel cached')
        with self.assertRaisesRegex(RuntimeError,'cancel cached'):replay(**self.values)
        self.assertEqual(replay.stats['replays'],1)
        replay.close()

    def test_bypass_retains_one_graph_and_still_checks_input_identity(self):
        original=Mock(side_effect=lambda **kw:Tensor())
        replay=Replay(self.t,original,100,lambda:None)
        replay(**self.values)
        replay.enabled=False
        replay(**self.values)
        self.assertEqual(replay.stats['replays'],1)
        self.assertEqual(replay.stats['bypass_calls'],1)
        self.values['x'].shape=(1,4)
        with self.assertRaisesRegex(ValueError,'GRAPH_INPUT_CHANGED'):replay(**self.values)
        self.values['x'].shape=(2,2)
        replay.enabled=True;replay(**self.values)
        self.assertEqual(replay.stats['captures'],1)
        self.assertEqual(replay.stats['replays'],2)
        replay.close()

    def test_capture_allocation_over_budget_restores_and_resets(self):
        self.t.cuda.memory_allocated=Mock(side_effect=[0,200])
        original=lambda **kw:Tensor()
        model=NS(forward=original)
        with self.assertRaisesRegex(ValueError,'GRAPH_RESERVED_BYTES'):
            with candidate(self.t,NS(model=model),100,lambda:None):
                model.forward(**self.values)
        self.assertIs(model.forward,original)
        self.graph.replay.assert_not_called()
        self.graph.reset.assert_called_once()

    def test_cancel_during_side_stream_warmup_retires_pending_work(self):
        model=NS(forward=lambda **kw:Tensor())
        original=model.forward
        check=Mock(side_effect=[None,None,RuntimeError('warm cancel')])
        with self.assertRaisesRegex(RuntimeError,'warm cancel'):
            with candidate(self.t,NS(model=model),100,check):
                model.forward(**self.values)
        self.assertIs(model.forward,original)
        self.t.cuda.synchronize.assert_called_once()
        self.graph.replay.assert_not_called()

    def test_combined_failure_resets_graph_before_releasing_ffn(self):
        import torch
        from avatar_fp8_ffn_candidate import candidate as ffn_candidate
        source=torch.nn.Sequential(torch.nn.Linear(16,32),torch.nn.GELU(),torch.nn.Linear(32,16)).eval()
        raw=NS(ffn=source);compiled=NS(_orig_mod=raw)
        model=NS(blocks=[compiled],forward=lambda **kw:Tensor())
        original=model.forward
        self.t.nn=torch.nn;self.t.no_grad=torch.no_grad
        self.t.compile=lambda raw,**kw:NS(_orig_mod=raw)
        class TestFfn(torch.nn.Module):
            def __init__(self,source):
                super().__init__();self.cache_bytes=1032
        def reset():
            self.assertIsNot(raw.ffn,source)
            self.assertIs(model.forward,original)
        self.graph.reset.side_effect=reset
        with self.assertRaisesRegex(RuntimeError,'combined failure'):
            with ExitStack() as ffns, ExitStack() as graphs:
                ffns.enter_context(ffn_candidate(self.t,NS(model=model),[0],2048,factory=TestFfn))
                graphs.enter_context(candidate(self.t,NS(model=model),100,lambda:None))
                model.forward(**self.values)
                raise RuntimeError('combined failure')
        self.graph.reset.assert_called_once()
        self.assertIs(raw.ffn,source)
        self.assertIs(model.blocks[0],compiled)


if __name__=='__main__':unittest.main()
