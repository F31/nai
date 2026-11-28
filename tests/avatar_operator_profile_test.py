import sys
import unittest
from pathlib import Path
from types import SimpleNamespace as NS
from contextlib import contextmanager

sys.path.insert(0, str(Path(__file__).resolve().parents[1]/'scripts'))
from avatar_operator_profile import ranges, summary, trace_summary


class OperatorProfileTests(unittest.TestCase):
    def test_ranges_preserve_value_and_restore_methods_after_failure(self):
        seen=[]
        @contextmanager
        def mark(name):
            seen.append(name)
            yield
        class Model:
            def forward(self, value): return value
        pipeline=NS(preprocess_audio=lambda x:x,model=Model(),
            vae=NS(decode=lambda x:x,encode=lambda x:x))
        originals=(pipeline.preprocess_audio,pipeline.vae.decode,pipeline.vae.encode)
        with self.assertRaises(RuntimeError):
            with ranges(NS(profiler=NS(record_function=mark)),pipeline):
                value=object()
                self.assertIs(pipeline.model.forward(value),value)
                raise RuntimeError('explicit failure')
        self.assertEqual(seen,['nai.denoise'])
        self.assertNotIn('forward',vars(pipeline.model))
        self.assertEqual(originals,(pipeline.preprocess_audio,pipeline.vae.decode,pipeline.vae.encode))

    def test_summary_separates_phase_inclusive_from_ranked_self_time(self):
        def event(name,time):
            return NS(key=name,device_type=NS(name='CPU'),count=1,self_device_time_total=time,device_time_total=time*2,self_cpu_time_total=1)
        gpu=event('same_kernel',100);gpu.device_type=NS(name='CUDA')
        result=summary([event('op'+str(i),i+1) for i in range(30)]+[event('nai.denoise',1000),gpu])
        self.assertEqual(len(result['top_operators']),20)
        self.assertEqual(result['top_operators'][0]['name'],'op29')
        self.assertEqual(result['phases'][0]['inclusive_device_us'],2000)
        self.assertEqual(result['top_device_events'][0]['name'],'same_kernel')
        with self.assertRaisesRegex(ValueError,'PROFILE_CUDA_EVENTS_MISSING'):
            summary([])
        with self.assertRaisesRegex(ValueError,'PROFILE_SUMMARY_VALUE'):
            summary([event('bad',float('nan'))])

    def test_trace_kernel_accounting_excludes_device_annotation_duplicates(self):
        result=trace_summary({'traceEvents':[
            dict(cat='user_annotation',name='nai.denoise',ts=10,dur=20,args={'External id':1}),
            dict(cat='cpu_op',name='aten::mm',ts=12,dur=5,args={'External id':2}),
            dict(cat='kernel',name='gemm',ts=40,dur=7,args={'External id':2}),
            dict(cat='gpu_user_annotation',name='nai.denoise',ts=40,dur=7,args={'External id':1})]})
        self.assertEqual(result['phases']['nai.denoise']['kernel_us'],7)
        self.assertEqual(result['phases']['nai.denoise']['calls'],1)

    def test_trace_does_not_attribute_other_host_thread_to_phase(self):
        result=trace_summary({'traceEvents':[
            dict(cat='user_annotation',name='nai.decode',ts=0,dur=100,pid=1,tid=2,args={}),
            dict(cat='cpu_op',name='aten::mm',ts=10,dur=5,pid=1,tid=3,args={'External id':8}),
            dict(cat='kernel',name='gemm',ts=30,dur=7,args={'External id':8})]})
        self.assertEqual(result['phases']['outside_or_unresolved']['kernel_us'],7)


if __name__=='__main__':unittest.main()
