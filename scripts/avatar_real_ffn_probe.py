"""Bounded actual-layer capture and isolated FFN comparison, never feedback to rendering."""
import statistics
from avatar_fp8_ops import Linear, make_quantizer


def render_facts(results):
    by_mode={row['mode']:row for row in results}
    baseline=by_mode['compiled-reference']['frame_digests']
    return dict(candidate_pixels_equal_baseline=None,fp8_outputs_rendered=False,
        capture_pixels_equal_baseline=by_mode['ffn-capture-maintenance']['frame_digests']==baseline,
        restored_baseline_pixels_equal=by_mode['compiled-confirm']['frame_digests']==baseline)


class Capture:
    def __init__(self,torch,pipeline,layers,steps,total_steps,budget):
        if (not layers or not steps or len(set(layers))!=len(layers) or len(set(steps))!=len(steps)
                or any(type(x) is not int or not 0<=x<len(pipeline.model.blocks) for x in layers)
                or any(type(x) is not int or not 0<=x<total_steps for x in steps)
                or type(budget) is not int or budget<1):
            raise ValueError('FFN_CAPTURE_CONFIG')
        self.torch,self.pipeline,self.layers,self.steps=torch,pipeline,layers,set(steps)
        self.total_steps,self.budget=total_steps,budget
        self.samples=[];self.bytes=0;self.handles=[];self.saved={};self.calls={index:0 for index in layers}

    def __enter__(self):
        try:
            for index in self.layers:
                compiled=self.pipeline.model.blocks[index]
                block=compiled._orig_mod
                self.saved[index]=compiled
                self.pipeline.model.blocks[index]=block
                ffn=block.ffn
                if (len(ffn)!=3 or not isinstance(ffn[0],self.torch.nn.Linear)
                        or not isinstance(ffn[1],self.torch.nn.GELU) or not isinstance(ffn[2],self.torch.nn.Linear)):
                    raise ValueError('FFN_CAPTURE_STRUCTURE')
                def hook(module,args,_index=index):
                    step=self.calls[_index];self.calls[_index]+=1
                    if step>=self.total_steps:raise ValueError('FFN_CAPTURE_CALL_LIMIT')
                    if step not in self.steps:return
                    from torch._subclasses.fake_tensor import FakeTensor
                    x=args[0]
                    if isinstance(x,FakeTensor):raise ValueError('FFN_CAPTURE_FAKE_INPUT')
                    size=x.numel()*x.element_size()
                    if size>self.budget-self.bytes:raise ValueError('FFN_CAPTURE_BYTES')
                    self.samples.append(dict(layer=_index,step=step,input=x.detach().clone(),module=module))
                    self.bytes+=size
                self.handles.append(ffn.register_forward_pre_hook(self.torch._dynamo.disable(hook)))
            return self
        except BaseException:
            self.__exit__(Exception,None,None)
            raise

    def __exit__(self,kind,*_):
        for handle in self.handles:handle.remove()
        self.handles.clear()
        for index,block in self.saved.items():self.pipeline.model.blocks[index]=block
        self.saved.clear()
        if kind:self.clear()

    def clear(self):
        self.samples.clear();self.bytes=0


def compare(torch,capture,repeats,budget,on_row=None):
    if not 2<=repeats<=6 or len(capture.samples)!=len(capture.layers)*len(capture.steps):
        raise ValueError('FFN_PROBE_SAMPLES')
    quantizer=torch.compile(make_quantizer(torch),dynamic=False)
    candidates={};rows=[];static_bytes=0
    def timed(fn):
        start,end=[torch.cuda.Event(enable_timing=True) for _ in range(2)]
        start.record();out=fn();end.record();torch.cuda.synchronize()
        return out,start.elapsed_time(end)
    for sample in capture.samples:
        module,x=sample['module'],sample['input']
        if x.dtype!=torch.bfloat16 or not torch.isfinite(x).all().item():
            raise ValueError('FFN_PROBE_INPUT')
        # Same conservative scratch allowance as the hardware gate, plus retained captures/weights.
        m=x.numel()//x.shape[-1];k=module[0].in_features;n=module[0].out_features
        scratch=max(12*m*k+8*k*n+8*m*n,12*m*n+8*n*k+8*m*k)
        if capture.bytes+static_bytes+scratch>budget:raise ValueError('FFN_PROBE_BYTES')
        if sample['layer'] not in candidates:
            required=module[0].weight.numel()+module[2].weight.numel()+8
            if capture.bytes+static_bytes+required+scratch>budget:raise ValueError('FFN_PROBE_BYTES')
            first,second=Linear(torch,module[0],quantizer),Linear(torch,module[2],quantizer)
            static_bytes+=first.bytes+second.bytes
            candidates[sample['layer']]=(first,second)
        first,second=candidates[sample['layer']]
        def baseline():return module(x)
        def candidate():return second(module[1](first(x)))
        reference=baseline();candidate();candidate();torch.cuda.synchronize()
        times={'bf16':[],'fp8_with_both_activation_quantizations':[]};out=None
        for repeat in range(repeats):
            funcs=[('bf16',baseline),('fp8_with_both_activation_quantizations',candidate)]
            if repeat%2:funcs.reverse()
            for name,fn in funcs:
                value,ms=timed(fn);times[name].append(ms)
                if name!='bf16':out=value
        if out is None or not torch.isfinite(reference).all().item() or not torch.isfinite(out).all().item():
            raise RuntimeError('FFN_PROBE_NONFINITE')
        delta=out.float()-reference.float();tiny=torch.finfo(torch.float32).tiny
        rows.append(dict(layer=sample['layer'],step=sample['step'],input_shape=list(x.shape),
            finite=True,relative_l2=(delta.norm()/reference.float().norm().clamp_min(tiny)).item(),
            max_abs=delta.abs().max().item(),mae=delta.abs().mean().item(),
            input_max_abs=x.float().abs().max().item(),stream_elapsed_ms=times,
            median_speedup=statistics.median(times['bf16'])/statistics.median(times['fp8_with_both_activation_quantizations'])))
        if on_row is not None:on_row(rows[-1])
    return dict(scope='actual_trained_ffn_not_full_avatar_quality',samples=rows,captured_bytes=capture.bytes,
        static_fp8_weight_bytes=static_bytes,captured_calls=capture.calls,
        capture_execution='selected_blocks_eager_for_input_capture_then_restored',
        quality_passed=False,production_activated=False)
