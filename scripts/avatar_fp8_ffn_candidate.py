"""Explicit full-render candidate; scoped FFN/compiled-block ownership."""
from contextlib import contextmanager
from avatar_fp8_ops import make_quantizer


def make_ffn_type(torch):
    quantize=make_quantizer(torch)
    class Fp8Ffn(torch.nn.Module):
        def __init__(self,source):
            super().__init__()
            if source.training or any(layer.weight.device.type!='cuda'
                    or layer.weight.dtype!=torch.bfloat16 for layer in (source[0],source[2])):
                raise ValueError('FP8_FFN_DEVICE_OR_DTYPE')
            self.activation=source[1]
            self.input_features=source[0].in_features
            self.hidden_features=source[0].out_features
            self.output_features=source[2].out_features
            for index in (0,2):
                weight,scale=quantize(source[index].weight)
                self.register_buffer('weight'+str(index),weight)
                self.register_buffer('scale'+str(index),scale)
                self.register_buffer('bias'+str(index),source[index].bias)
            self.cache_bytes=sum(value.numel()*value.element_size() for value in
                (self.weight0,self.weight2,self.scale0,self.scale2))

        def forward(self,x):
            leading=x.shape[:-1]
            a,scale=quantize(x.reshape(-1,self.input_features))
            h=torch._scaled_mm(a,self.weight0.t(),scale_a=scale,scale_b=self.scale0,
                bias=self.bias0,out_dtype=torch.bfloat16,use_fast_accum=False)
            h=self.activation(h)
            a,scale=quantize(h)
            out=torch._scaled_mm(a,self.weight2.t(),scale_a=scale,scale_b=self.scale2,
                bias=self.bias2,out_dtype=torch.bfloat16,use_fast_accum=False)
            return out.reshape(*leading,self.output_features)
    return Fp8Ffn


@contextmanager
def candidate(torch,pipeline,layers,budget,factory=None):
    if (not layers or len(set(layers))!=len(layers) or type(budget) is not int or budget<1
            or any(type(i) is not int or not 0<=i<len(pipeline.model.blocks) for i in layers)):
        raise ValueError('FP8_FFN_SELECTION')
    saved=[];versions=[];required=0
    for index in layers:
        compiled=pipeline.model.blocks[index]
        if not hasattr(compiled,'_orig_mod'):raise ValueError('FP8_FFN_REQUIRES_COMPILED_BLOCK')
        raw=compiled._orig_mod;ffn=raw.ffn
        if (len(ffn)!=3 or not isinstance(ffn[0],torch.nn.Linear)
                or not isinstance(ffn[1],torch.nn.GELU) or not isinstance(ffn[2],torch.nn.Linear)):
            raise ValueError('FP8_FFN_STRUCTURE')
        required+=ffn[0].weight.numel()+ffn[2].weight.numel()+8
        if required>budget:raise ValueError('FP8_FFN_CACHE_BYTES')
        versions.extend((p,p._version) for p in ffn.parameters())
        saved.append((index,compiled,raw,ffn))
    build=factory or make_ffn_type(torch)
    stats=dict(layers=list(layers),static_cache_bytes=0,cache_budget_bytes=budget,
        precision='fp8_e4m3fn_ffn_bf16_output',production_activated=False)
    try:
        with torch.no_grad():
            for index,compiled,raw,ffn in saved:
                replacement=build(ffn).eval()
                stats['static_cache_bytes']+=replacement.cache_bytes
                if stats['static_cache_bytes']>budget:raise ValueError('FP8_FFN_CACHE_BYTES')
                raw.ffn=replacement
                pipeline.model.blocks[index]=torch.compile(raw,dynamic=False)
        yield stats
    finally:
        for index,compiled,raw,ffn in saved:
            raw.ffn=ffn
            pipeline.model.blocks[index]=compiled
        if any(p._version!=version for p,version in versions):
            raise ValueError('FP8_FFN_SOURCE_CHANGED')
