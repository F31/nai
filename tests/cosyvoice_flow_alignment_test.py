from pathlib import Path
import sys
from types import SimpleNamespace as NS
import unittest
import torch

sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'scripts'))
from cosyvoice_flow_alignment import align,install,FLOW_FIELDS


class AlignmentTests(unittest.TestCase):
    def setUp(self):
        token=torch.arange(87,dtype=torch.int32).reshape(1,-1)
        length=torch.tensor([87],dtype=torch.int32)
        self.c=dict(flow_prompt_speech_token=token,flow_prompt_speech_token_len=length,
            prompt_speech_feat=torch.arange(174*80,dtype=torch.float32).reshape(1,174,80),
            prompt_speech_feat_len=torch.tensor([174],dtype=torch.int32),
            llm_prompt_speech_token=token,llm_prompt_speech_token_len=length,
            prompt_text=torch.tensor([1]),llm_embedding=torch.ones(1,192),flow_embedding=torch.ones(1,192))
        self.args=dict(hop=25,ratio=2,frame_rate=25,feature_dim=80,minimum_seconds=3,
            max_bytes=1048576,check=lambda:None)

    def test_four_fields_aligned_without_mutating_llm_voice_or_cache_aliases(self):
        r,m=align(self.c,**self.args)
        self.assertEqual((r['flow_prompt_speech_token'].shape[1],r['prompt_speech_feat'].shape[1]),(75,150))
        self.assertEqual((r['flow_prompt_speech_token_len'].item(),r['prompt_speech_feat_len'].item()),(75,150))
        for k in self.c:
            if k not in FLOW_FIELDS:self.assertIs(r[k],self.c[k])
        r['flow_prompt_speech_token'].fill_(99);r['prompt_speech_feat'].zero_()
        self.assertEqual(self.c['llm_prompt_speech_token'][0,0].item(),0)
        self.assertEqual(self.c['prompt_speech_feat'][0,0,1].item(),1)
        self.assertEqual(m['original_tokens'],87)

    def test_minimum_context_budget_and_wrong_lengths_refused(self):
        for changes,error in [({'minimum_seconds':4},'TOO_SHORT'),({'max_bytes':10},'BYTES'),({'ratio':3},'SHAPE')]:
            with self.assertRaisesRegex(ValueError,error):align(self.c,**(self.args|changes))
        self.c['flow_prompt_speech_token_len']=torch.tensor([86])
        with self.assertRaisesRegex(ValueError,'LENGTH'):align(self.c,**self.args)

    def test_cancel_before_and_after_copy_never_returns_conditions(self):
        for at in (1,3):
            calls=[]
            def check():
                calls.append(1)
                if len(calls)==at:raise RuntimeError('cancel')
            with self.assertRaisesRegex(RuntimeError,'cancel'):align(self.c,**(self.args|{'check':check}))
            self.assertEqual(self.c['flow_prompt_speech_token'].shape[1],87)

    def test_unknown_closure_is_not_guessed(self):
        del self.c['flow_embedding']
        with self.assertRaisesRegex(ValueError,'CLOSURE'):align(self.c,**self.args)

    def test_wrapper_restores_and_repeated_requests_do_not_pollute_source(self):
        frontend=NS(frontend_zero_shot=lambda *a,**kw:self.c)
        original=frontend.frontend_zero_shot
        model=NS(frontend=frontend,model=NS(token_hop_len=25,
            flow=NS(token_mel_ratio=2,input_frame_rate=25,output_size=80)))
        close=install(model,minimum_seconds=3,max_bytes=1048576,sink=lambda r:None)
        frontend.frontend_zero_shot()['flow_prompt_speech_token'].zero_()
        self.assertEqual(frontend.frontend_zero_shot()['flow_prompt_speech_token'][0,1].item(),1)
        with self.assertRaises(ValueError):install(model,minimum_seconds=3,max_bytes=1048576)
        close();self.assertIs(frontend.frontend_zero_shot,original)


if __name__=='__main__':unittest.main()
