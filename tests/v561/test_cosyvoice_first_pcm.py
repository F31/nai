import importlib.util
import json
from pathlib import Path
import sys
import threading
import time
from types import SimpleNamespace as NS
import unittest

ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'scripts'))
from cosyvoice_first_pcm import install,decode_metadata,TOKEN_TIME_LIMIT
from unittest.mock import patch


class Gate:
    def start(self,factory,on_complete=None):
        result=list(factory())
        if on_complete:on_complete()
        return result


class FirstPCMTests(unittest.TestCase):
    def test_first_decode_shape_and_token_times_without_content(self):
        model=self.model();model.model.token_hop_len=25;model.model.flow.pre_lookahead_len=3
        model.model.token2wav=lambda **kw:b'\1\0'
        class Llm:
            def inference(self,text):yield from range(TOKEN_TIME_LIMIT+20)
        model.model.llm=Llm()
        records=[];gate=Gate();observer=install(model,gate,records.append)
        def factory():
            list(model.model.llm.inference('PRIVATE'))
            yield model.model.token2wav(token=NS(shape=(1,45)),prompt_token=NS(shape=(1,158)),token_offset=0,finalize=False)
        self.assertEqual(gate.start(factory),[b'\1\0'])
        meta=records[0]['first_decode']
        self.assertEqual((meta['prompt_pad'],meta['required_first_tokens']),(17,45))
        self.assertIsNotNone(meta['required_tokens_ready_ms'])
        self.assertTrue(records[0]['token_timing_truncated'])
        self.assertNotIn('PRIVATE',json.dumps(records));self.assertIsNone(observer.current)

    def test_metadata_error_does_not_change_audio(self):
        model=self.model();model.model.token2wav=lambda **kw:b'\1\0'
        records=[];gate=Gate();install(model,gate,records.append)
        self.assertEqual(gate.start(lambda:iter([model.model.token2wav()])),[b'\1\0'])
        self.assertIsNone(records[0]['first_decode'])

    def test_padding_formula_preserves_declared_hop_and_environment(self):
        core=NS(token_hop_len=25,flow=NS(pre_lookahead_len=3))
        with patch.dict('os.environ',{'COSYVOICE_INITIAL_HOP_TOKENS':'50'}):
            r=decode_metadata(core,dict(token=NS(shape=(1,95)),prompt_token=NS(shape=(1,158))))
        self.assertEqual((r['initial_hop'],r['prompt_pad'],r['required_first_tokens']),(50,42,95))

    def model(self):
        class Llm:
            def inference(self,text):yield 1;yield 2
        class Frontend:
            def frontend_zero_shot(self,text):return object()
        return NS(frontend=Frontend(),model=NS(fp16=False,llm=Llm(),flow=NS(inference=lambda value:value)))

    def test_original_values_callbacks_and_background_first_token(self):
        model=self.model();records=[];gate=Gate();install(model,gate,records.append)
        data=[bytes([1,0]),bytes([2,0])];callbacks=[]
        def factory():
            model.frontend.frontend_zero_shot('DO_NOT_LOG')
            thread=threading.Thread(target=lambda:list(model.model.llm.inference('DO_NOT_LOG')))
            thread.start();thread.join()
            for part in data:yield model.model.flow.inference(part)
        self.assertEqual(gate.start(factory,lambda:callbacks.append(True)),data)
        self.assertEqual(callbacks,[True]);self.assertEqual(len(records),1)
        record=records[0]
        self.assertEqual((record['state'],record['tokens'],record['samples']),('generator_completed',2,2))
        self.assertIsNotNone(record['first_token_ms']);self.assertIsNotNone(record['first_pcm_ms'])
        self.assertNotIn('DO_NOT_LOG',json.dumps(record))

    def test_exception_is_original_and_partial_is_not_complete(self):
        records=[];gate=Gate();install(self.model(),gate,records.append)
        error=RuntimeError('PRIVATE_TEXT_AND_KEY')
        def factory():yield bytes(2);raise error
        with self.assertRaises(RuntimeError) as caught:gate.start(factory)
        self.assertIs(caught.exception,error)
        self.assertEqual(records[0]['state'],'generator_failed')
        self.assertNotIn('PRIVATE_TEXT_AND_KEY',json.dumps(records))

    def test_close_runs_original_cleanup_and_releases_observer(self):
        records=[];gate=Gate();observer=install(self.model(),gate,records.append);cleanup=[]
        def factory():
            try:yield bytes(2);yield bytes(2)
            finally:cleanup.append(True)
        generator=observer.factory(factory,None)();next(generator);generator.close()
        self.assertEqual(cleanup,[True]);self.assertIsNone(observer.current)
        self.assertEqual(records[0]['state'],'generator_failed')

    def test_spans_are_bounded_and_diagnostic_errors_do_not_change_audio(self):
        model=self.model();records=[];gate=Gate();install(model,gate,records.append)
        def factory():
            for _ in range(200):model.model.flow.inference(None)
            yield bytes(2)
        self.assertEqual(gate.start(factory),[bytes(2)])
        self.assertEqual(len(records[0]['spans']),64);self.assertEqual(records[0]['dropped_spans'],136)
        gate=Gate();install(self.model(),gate,lambda record:(_ for _ in ()).throw(OSError('disk')))
        self.assertEqual(gate.start(lambda:iter([bytes(2)])),[bytes(2)])

    def test_load_is_handed_off_once_and_failed_load_does_not_leak(self):
        records=[];gate=Gate();observer=install(self.model(),gate,records.append)
        observer.wrap_load(lambda value:value)('PRIVATE_PATH')
        gate.start(lambda:iter([bytes(2)]));gate.start(lambda:iter([bytes(2)]))
        self.assertEqual([s['stage'] for s in records[0]['spans']],['load_wav'])
        self.assertEqual(records[1]['spans'],[])
        with self.assertRaises(ValueError):observer.wrap_load(lambda:(_ for _ in ()).throw(ValueError()))()
        gate.start(lambda:iter([bytes(2)]));self.assertEqual(records[2]['spans'],[])
        with self.assertRaises(ValueError):install(self.model(),gate)

    def test_real_gate_keeps_busy_rejection_and_completion_callback(self):
        from cosyvoice_request_gate import RequestGate,BusyError
        records=[];gate=RequestGate(2);install(self.model(),gate,records.append)
        entered=threading.Event();release=threading.Event();receipts=[]
        def factory():
            entered.set();release.wait(1);yield bytes(2)
        job=gate.start(factory,receipts.append)
        self.assertTrue(entered.wait(1))
        with self.assertRaises(BusyError):gate.start(lambda:iter([bytes(2)]))
        release.set();self.assertTrue(job.finished.wait(1))
        self.assertEqual(receipts[0]['state'],'completed');self.assertEqual(receipts[0]['samples'],1)
        self.assertEqual(len(records),1)

    def test_real_gate_cancel_still_drains_and_diagnostic_is_not_eos(self):
        from cosyvoice_request_gate import RequestGate
        records=[];gate=RequestGate(2,queue_chunks=1);install(self.model(),gate,records.append)
        generated=[]
        def factory():
            for i in range(4):generated.append(i);yield bytes(2)
        job=gate.start(factory)
        end=time.monotonic()+1
        while len(generated)<2 and time.monotonic()<end:time.sleep(.001)
        job.cancel();self.assertTrue(job.finished.wait(1))
        self.assertEqual(generated,[0,1,2,3]);self.assertEqual(job.completion()['state'],'cancelled')
        self.assertEqual(records[0]['state'],'generator_completed')


class PatchTests(unittest.TestCase):
    def module(self):
        spec=importlib.util.spec_from_file_location('first_pcm_patch',ROOT/'scripts/prepare-cosyvoice-first-pcm.py')
        module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);return module

    def test_only_unique_assignment_is_extended(self):
        source="before = 1\nif True:\n    cosyvoice = AutoModel(model_dir='unchanged')\n    after = 2\n"
        changed=self.module().patch(source)
        self.assertIn("AutoModel(model_dir='unchanged')",changed)
        self.assertTrue(changed.startswith('before = 1\n'))
        self.assertTrue(changed.endswith('    after = 2\n'))
        with self.assertRaises(ValueError):self.module().patch(changed)
        with self.assertRaises(ValueError):self.module().patch(source+source)
        with self.assertRaises(ValueError):self.module().patch('x = OtherModel()\n')


if __name__=='__main__':unittest.main()
