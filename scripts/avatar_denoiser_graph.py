"""Explicit one-shape CUDA graph diagnostic, never a production default."""
from contextlib import contextmanager


def signature(torch, values):
    if set(values) != {'x','timestep','context','y'}:
        raise ValueError('GRAPH_INPUT_KEYS')
    if any(not torch.is_tensor(x) or x.device.type!='cuda' for x in values.values()):
        raise ValueError('GRAPH_INPUT_DEVICE')
    return {k:(tuple(v.shape),tuple(v.stride()),str(v.dtype),str(v.device))
            for k,v in values.items()}


class Replay:
    def __init__(self,torch,original,budget,check):
        if type(budget) is not int or budget<1: raise ValueError('GRAPH_BUDGET')
        self.torch,self.original,self.budget,self.check=torch,original,budget,check
        self.graph=None;self.inputs=None;self.output=None;self.key=None
        self.enabled=True
        self.stats=dict(captures=0,replays=0,static_bytes=0,reserved_delta_bytes=0,
                        allocated_delta_bytes=0,bypass_calls=0,cache_budget_bytes=budget,production_activated=False)

    def __call__(self,*args,**values):
        if args: raise ValueError('GRAPH_KEYWORD_INPUTS_REQUIRED')
        self.check()
        t=self.torch;key=signature(t,values)
        if self.key is not None and key!=self.key: raise ValueError('GRAPH_INPUT_CHANGED')
        if not self.enabled:
            result=self.original(**values)
            self.stats['bypass_calls']+=1
            self.check()
            return result
        if self.graph is None:
            size=sum(x.numel()*x.element_size() for x in values.values())
            # Output has the same geometry/dtype as x in this denoiser.
            size+=values['x'].numel()*values['x'].element_size()
            if size>self.budget: raise ValueError('GRAPH_STATIC_BYTES')
            before=t.cuda.memory_reserved()
            allocated_before=t.cuda.memory_allocated()
            self.key=key
            self.inputs={k:v.clone(memory_format=t.preserve_format) for k,v in values.items()}
            current=t.cuda.current_stream();stream=t.cuda.Stream()
            stream.wait_stream(current)
            with t.cuda.stream(stream):
                for _ in range(2):
                    self.check();self.original(**self.inputs)
            current.wait_stream(stream)
            self.check()
            self.graph=t.cuda.CUDAGraph()
            with t.cuda.graph(self.graph,stream=stream):
                self.output=self.original(**self.inputs)
            if (not t.is_tensor(self.output) or self.output.shape!=values['x'].shape
                    or self.output.dtype!=values['x'].dtype):
                raise ValueError('GRAPH_OUTPUT_IDENTITY')
            self.stats.update(captures=1,static_bytes=size,
                allocated_delta_bytes=max(0,t.cuda.memory_allocated()-allocated_before),
                reserved_delta_bytes=max(0,t.cuda.memory_reserved()-before))
            if max(self.stats['reserved_delta_bytes'],self.stats['allocated_delta_bytes'])>self.budget:
                raise ValueError('GRAPH_RESERVED_BYTES')
        self.check()
        for k,v in values.items(): self.inputs[k].copy_(v)
        self.graph.replay()
        self.stats['replays']+=1
        result=self.output.clone()
        self.check()
        return result

    def close(self):
        if self.inputs is not None:
            self.torch.cuda.synchronize()
        if self.graph is not None:
            self.graph.reset()
        self.graph=self.inputs=self.output=None


@contextmanager
def candidate(torch,pipeline,budget,check):
    owner=pipeline.model;original=owner.forward;own='forward' in vars(owner)
    replay=Replay(torch,original,budget,check)
    owner.forward=replay
    try:
        yield replay
    finally:
        if own: owner.forward=original
        else: delattr(owner,'forward')
        replay.close()
