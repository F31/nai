"""Shared diagnostic FP8 operations. Not installed into the live model."""


def make_quantizer(torch):
    maximum=torch.finfo(torch.float8_e4m3fn).max
    tiny=torch.finfo(torch.float32).tiny
    def quantize(x):
        f=x.float()
        scale=(f.abs().amax()/maximum).clamp_min(tiny)
        return (f/scale).clamp(-maximum,maximum).to(torch.float8_e4m3fn),scale
    return quantize


class Linear:
    def __init__(self, torch, module, activation_quantize):
        if (module.training or torch.is_grad_enabled() or module.weight.device.type!='cuda'
                or module.weight.dtype!=torch.bfloat16):
            raise ValueError('FP8_LINEAR_CONFIG')
        self.torch,self.module,self.quantize=torch,module,activation_quantize
        self.weight_version=module.weight._version
        self.bias_version=module.bias._version if module.bias is not None else None
        self.weight,self.scale=make_quantizer(torch)(module.weight)
        self.bytes=self.weight.numel()*self.weight.element_size()+self.scale.numel()*self.scale.element_size()

    def __call__(self, x):
        t,m=self.torch,self.module
        if (x.dtype!=t.bfloat16 or x.device!=m.weight.device or x.shape[-1]!=m.in_features
                or m.weight._version!=self.weight_version
                or (m.bias is not None and m.bias._version!=self.bias_version)):
            raise ValueError('FP8_LINEAR_CHANGED')
        qa,sa=self.quantize(x.reshape(-1,m.in_features))
        value=t._scaled_mm(qa,self.weight.t(),scale_a=sa,scale_b=self.scale,bias=m.bias,
            out_dtype=t.bfloat16,use_fast_accum=False)
        return value.reshape(*x.shape[:-1],m.out_features)
