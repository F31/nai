"""Bounded, content-free wall-clock observation of one single-flight service.
No CUDA synchronization, model settings, tensor copies or new HTTP surface.
"""
import functools
import json
import os
import sys
import threading
import time

TOKEN_TIME_LIMIT=256


def decode_metadata(core,kwargs):
    base=int(core.token_hop_len)
    initial=int(os.environ.get('COSYVOICE_INITIAL_HOP_TOKENS',str(base)))
    prompt=int(kwargs['prompt_token'].shape[-1])
    tokens=int(kwargs['token'].shape[-1])
    lookahead=int(core.flow.pre_lookahead_len)
    if min(base,initial)<1 or min(prompt,tokens,lookahead)<0:
        raise ValueError('HOP_METADATA')
    pad=(-prompt)%initial
    return dict(base_hop=base,initial_hop=initial,prompt_tokens=prompt,prompt_pad=pad,
        lookahead=lookahead,required_first_tokens=initial+pad+lookahead,
        decoder_input_tokens=tokens,token_offset=int(kwargs.get('token_offset',0)),
        finalize=bool(kwargs.get('finalize',False)))


class Trace:
    def __init__(self, sequence, started, unix_ms, precision):
        self.sequence,self.started,self.unix_ms,self.precision=sequence,started,unix_ms,precision
        self.lock=threading.Lock()
        self.spans=[];self.dropped=0;self.first_pcm=None;self.first_token=None;self.tokens=0;self.samples=0
        self.token_times=[];self.first_decode=None

    def span(self,name,start,end):
        with self.lock:
            if len(self.spans)<64:
                self.spans.append(dict(stage=name,start_ms=round((start-self.started)*1000,3),
                    end_ms=round((end-self.started)*1000,3),wall_ms=round((end-start)*1000,3)))
            else:self.dropped+=1

    def snapshot(self,state,end):
        with self.lock:
            return dict(protocol='nai.tts.stages.v1',sequence=self.sequence,started_unix_ms=self.unix_ms,
                precision=self.precision,state=state,elapsed_ms=round((end-self.started)*1000,3),
                first_pcm_ms=self.first_pcm,first_token_ms=self.first_token,tokens=self.tokens,
                samples=self.samples,spans=list(self.spans),dropped_spans=self.dropped,
                first_decode=self.first_decode,
                token_timing_truncated=self.tokens>len(self.token_times),
                timing='python_wall_not_cuda_event')


class Observer:
    def __init__(self,model,sink=None,clock=time.monotonic):
        self.clock=clock;self.local=threading.local();self.lock=threading.Lock()
        self.current=None;self.sequence=0
        self.core=getattr(model,'model',None)
        value=getattr(getattr(model,'model',None),'fp16',None)
        self.precision='fp16' if value is True else 'fp32' if value is False else 'unknown'
        self.sink=sink or self._log

    @staticmethod
    def _log(record):
        try:print('NAI_FIRST_PCM_TRACE '+json.dumps(record,separators=(',',':')),file=sys.stderr,flush=True)
        except OSError:pass

    def wrap_load(self,original):
        @functools.wraps(original)
        def load(*args,**kwargs):
            start=self.clock();unix_ms=int(time.time()*1000)
            try:result=original(*args,**kwargs)
            except BaseException:self.local.pre=None;raise
            self.local.pre=(start,self.clock(),unix_ms)
            return result
        return load

    def wrap(self,obj,name,stage,generator=False):
        if obj is None or not hasattr(obj,name):return
        original=getattr(obj,name)
        if generator:
            @functools.wraps(original)
            def call(*args,**kwargs):
                trace=self.current;start=self.clock()
                try:
                    for value in original(*args,**kwargs):
                        if trace is not None:
                            with trace.lock:
                                trace.tokens+=1
                                at=round((self.clock()-trace.started)*1000,3)
                                if len(trace.token_times)<TOKEN_TIME_LIMIT:trace.token_times.append(at)
                                if trace.first_token is None:trace.first_token=at
                        yield value
                finally:
                    if trace is not None:trace.span(stage,start,self.clock())
        else:
            @functools.wraps(original)
            def call(*args,**kwargs):
                trace=self.current;start=self.clock()
                if trace is not None and stage=='token2wav':
                    try:
                        metadata=decode_metadata(self.core,kwargs)
                        with trace.lock:
                            if trace.first_decode is None:
                                required=metadata['required_first_tokens']
                                metadata.update(start_ms=round((start-trace.started)*1000,3),
                                    observed_llm_tokens=trace.tokens,
                                    required_tokens_ready_ms=trace.token_times[required-1]
                                        if 1<=required<=len(trace.token_times) else None,
                                    base_tokens_ready_ms=trace.token_times[metadata['initial_hop']-1]
                                        if metadata['initial_hop']<=len(trace.token_times) else None)
                                trace.first_decode=metadata
                    except Exception:pass  # metadata observation cannot alter synthesis
                try:return original(*args,**kwargs)
                finally:
                    if trace is not None:trace.span(stage,start,self.clock())
        setattr(obj,name,call)

    def factory(self,factory,pre):
        def observed():
            with self.lock:
                if self.current is not None:raise RuntimeError('DIAGNOSTIC_SINGLE_FLIGHT')
                self.sequence+=1
                start=pre[0] if pre else self.clock()
                trace=Trace(self.sequence,start,pre[2] if pre else int(time.time()*1000),self.precision)
                self.current=trace
            if pre:trace.span('load_wav',pre[0],pre[1])
            state='generator_failed';iterator=None
            try:
                iterator=iter(factory())
                for part in iterator:
                    with trace.lock:
                        if trace.first_pcm is None:trace.first_pcm=round((self.clock()-trace.started)*1000,3)
                        if isinstance(part,bytes):trace.samples+=len(part)//2
                    yield part
                state='generator_completed'
            finally:
                try:
                    if iterator is not None and hasattr(iterator,'close'):iterator.close()
                except BaseException:
                    state='generator_failed';raise
                finally:
                    record=trace.snapshot(state,self.clock())
                    with self.lock:self.current=None
                    try:self.sink(record)
                    except Exception:pass  # diagnostic output cannot change synthesis
        return observed


def install(model,gate,sink=None):
    if hasattr(gate,'_nai_first_pcm'):raise ValueError('DIAGNOSTIC_ALREADY_INSTALLED')
    observer=Observer(model,sink)
    gate._nai_first_pcm=observer
    frontend=getattr(model,'frontend',None);core=getattr(model,'model',None)
    for name in ['text_normalize','frontend_zero_shot','_extract_speech_feat','_extract_speech_token','_extract_spk_embedding']:
        observer.wrap(frontend,name,name)
    observer.wrap(core,'llm_job','llm_job')
    observer.wrap(core,'token2wav','token2wav')
    observer.wrap(getattr(core,'flow',None),'inference','flow')
    observer.wrap(getattr(core,'hift',None),'inference','hift')
    observer.wrap(getattr(core,'llm',None),'inference','llm_tokens',generator=True)
    original=gate.start
    @functools.wraps(original)
    def start(factory,*args,**kwargs):
        pre=getattr(observer.local,'pre',None);observer.local.pre=None
        return original(observer.factory(factory,pre),*args,**kwargs)
    gate.start=start
    return observer
