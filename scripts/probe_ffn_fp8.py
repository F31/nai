"""Explicit synthetic-shape hardware probe, never a model/visual quality grant."""
import argparse
import json
import os
from pathlib import Path
import statistics
import sys
import time


def plan(avatar, model, budget):
    values=[avatar['width'],avatar['height'],avatar['frame_num'],model['dim'],model['ffn_dim'],
        *model['vae_stride'],*model['patch_size']]
    if (type(budget) is not int or budget < 1 or len(model['vae_stride']) != 3
            or len(model['patch_size']) != 3 or any(type(x) is not int or x < 1 for x in values)
            or not 256 <= avatar['width'] <= 1024 or not 256 <= avatar['height'] <= 1024
            or not 17 <= avatar['frame_num'] <= 97):
        raise ValueError('FP8_PROBE_CONFIG')
    stride, patch=model['vae_stride'],model['patch_size']
    if ((avatar['frame_num']-1)%stride[0] or avatar['width']%stride[2] or avatar['height']%stride[1]):
        raise ValueError('FP8_PROBE_GRID')
    latent=[(avatar['frame_num']-1)//stride[0]+1,avatar['height']//stride[1],avatar['width']//stride[2]]
    if any(x%p for x,p in zip(latent,patch)): raise ValueError('FP8_PROBE_GRID')
    m=1
    for x,p in zip(latent,patch):m*=x//p
    shapes=[(m,model['dim'],model['ffn_dim']),(m,model['ffn_dim'],model['dim'])]
    for m,k,n in shapes:
        if any(x%16 for x in (m,k,n)): raise ValueError('FP8_PROBE_ALIGNMENT')
        # Conservative tensor-only allowance, including conversions and differences.
        if 12*m*k+8*k*n+8*m*n > budget: raise ValueError('FP8_PROBE_MEMORY')
    return shapes


def read_json(path):
    with path.open('rb') as f: data=f.read(65537)
    if len(data)>65536:raise ValueError('FP8_PROBE_CONFIG_SIZE')
    return json.loads(data)


def layer(torch, shape, repeats, *, compile_quant, status, seconds):
    m,k,n=shape
    a=torch.randn(m,k,device='cuda',dtype=torch.bfloat16)
    w=torch.randn(n,k,device='cuda',dtype=torch.bfloat16)/(k**0.5)
    bias=torch.randn(n,device='cuda',dtype=torch.bfloat16)*0.01
    from avatar_fp8_ops import make_quantizer
    tiny=torch.finfo(torch.float32).tiny
    quantize=make_quantizer(torch)
    started=time.perf_counter_ns()
    qw,sw=quantize(w);torch.cuda.synchronize()
    weight_ms=(time.perf_counter_ns()-started)/1e6
    def baseline():return torch.nn.functional.linear(a,w,bias)
    activation_quantize=quantize
    compilation=None
    if compile_quant:
        from torch._dynamo.utils import counters
        from torch._inductor import metrics
        before_graphs=counters['stats']['unique_graphs'];before_kernels=metrics.generated_kernel_count
        activation_quantize=torch.compile(quantize,dynamic=False)
        compiled=activation_quantize(a);activation_quantize(a);torch.cuda.synchronize()
        eager=quantize(a)
        compilation=dict(new_unique_graphs=counters['stats']['unique_graphs']-before_graphs,
            new_generated_kernels=metrics.generated_kernel_count-before_kernels,
            equals_eager=all(torch.equal(x,y) for x,y in zip(eager,compiled)))
        if compilation['new_unique_graphs']<1 or compilation['new_generated_kernels']<1:
            raise RuntimeError('FP8_QUANT_COMPILE_EVIDENCE_MISSING')
    def candidate():
        qa,sa=activation_quantize(a)
        return torch._scaled_mm(qa,qw.t(),scale_a=sa,scale_b=sw,bias=bias,
            out_dtype=torch.bfloat16,use_fast_accum=False)
    qa,sa=quantize(a)
    def matmul_only():
        return torch._scaled_mm(qa,qw.t(),scale_a=sa,scale_b=sw,bias=bias,
            out_dtype=torch.bfloat16,use_fast_accum=False)
    functions=[('bf16',baseline),('fp8_with_activation_quantization',candidate),
        ('activation_quantization_only',lambda:activation_quantize(a)),('fp8_matmul_only',matmul_only)]
    for _ in range(2):
        for _,fn in functions:fn()
    torch.cuda.synchronize()
    def timed(fn):
        start,end=[torch.cuda.Event(enable_timing=True) for _ in range(2)]
        start.record();value=fn();end.record();torch.cuda.synchronize()
        return value,start.elapsed_time(end)
    durations={name:[] for name,_ in functions}
    outputs={}
    from avatar_compile_maintenance import Maintenance
    with Maintenance(status,seconds):
        for index in range(repeats):
            order=functions.copy()
            if index%2:order.reverse()
            for name,fn in order:
                value,ms=timed(fn);outputs[name]=value;durations[name].append(ms)
    ref=outputs['bf16'].float();out=outputs['fp8_with_activation_quantization'].float()
    if not torch.isfinite(ref).all().item() or not torch.isfinite(out).all().item():
        raise RuntimeError('FP8_PROBE_NONFINITE')
    delta=out-ref
    return dict(shape_mkn=list(shape),weight_preprocess_ms=weight_ms,stream_elapsed_ms=durations,
        activation_quantization_compilation=compilation,
        speedup_including_activation_quantization=statistics.median(durations['bf16'])/
            statistics.median(durations['fp8_with_activation_quantization']),
        relative_l2=(delta.norm()/ref.norm().clamp_min(tiny)).item(),
        max_abs=delta.abs().max().item(),mae=delta.abs().mean().item(),
        finite=True,output_dtype=str(outputs['fp8_with_activation_quantization'].dtype))


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--avatar-config',type=Path,required=True)
    p.add_argument('--model-config',type=Path,required=True)
    p.add_argument('--report',type=Path,required=True)
    p.add_argument('--max-tensor-bytes',type=int,required=True)
    p.add_argument('--repeats',type=int,required=True)
    p.add_argument('--compile-activation-quantization',action='store_true')
    args=p.parse_args()
    if (not args.report.is_absolute() or args.report.exists() or not args.report.parent.is_dir()
            or args.report.with_suffix('.status.json').exists() or not 2<=args.repeats<=6):
        raise ValueError('FP8_PROBE_OUTPUT_OR_REPEATS')
    avatar,model=read_json(args.avatar_config),read_json(args.model_config)
    from flashhead_stream_worker import validate_budgets
    validate_budgets(avatar)
    shapes=plan(avatar,model,args.max_tensor_bytes)
    sys.path[:0]=avatar['dependency_dirs']
    temp=Path(avatar['temp_dir']).resolve()
    target=Path(__file__).resolve().parents[1]/'target-windows'
    if not temp.is_dir() or not temp.is_relative_to(target.resolve()):
        raise ValueError('FP8_PROBE_CACHE_ROOT')
    for key in ('TEMP','TMP'):os.environ[key]=str(temp)
    for key,folder in [('TORCHINDUCTOR_CACHE_DIR','inductor'),('TRITON_CACHE_DIR','triton'),
            ('CUDA_CACHE_PATH','cuda'),('TORCH_EXTENSIONS_DIR','extensions')]:
        os.environ[key]=str(temp/folder)
    report=dict(scope='synthetic_ffn_hardware_not_model_quality',model_loads=0,model_api_calls=0,
        precision='e4m3fn_with_fp32_scaling_bf16_output',quality_passed=False,production_activated=False,
        repeats=args.repeats,tensor_budget_bytes=args.max_tensor_bytes)
    report['compiled_activation_quantization']=args.compile_activation_quantization
    from avatar_compile_maintenance import Maintenance
    try:
        limit=avatar['maintenance_timeout_ms'] if args.compile_activation_quantization else avatar['timeout_ms']
        with Maintenance(args.report.with_suffix('.status.json'),limit/1000):
            if args.compile_activation_quantization:
                from avatar_compile import configure_windows_compiler
                report['compiler_environment']=configure_windows_compiler()
            import torch
            if not torch.cuda.is_available() or not hasattr(torch,'_scaled_mm') or not hasattr(torch,'float8_e4m3fn'):
                raise RuntimeError('FP8_API_UNAVAILABLE')
            torch.manual_seed(avatar['seed'])
            report.update(torch=torch.__version__,cuda=torch.version.cuda,gpu=torch.cuda.get_device_name(0),
                capability=list(torch.cuda.get_device_capability(0)))
            with torch.no_grad():
                report['layers']=[layer(torch,shape,args.repeats,compile_quant=args.compile_activation_quantization,
                    status=args.report.with_suffix(f'.layer-{index}.status.json'),seconds=avatar['timeout_ms']/1000)
                    for index,shape in enumerate(shapes)]
            report['status']='hardware_probe_completed'
    except Exception as error:
        report.update(status='unavailable_or_failed',error_type=type(error).__name__,reason=str(error)[:512])
    with args.report.open('x',encoding='utf8') as f:json.dump(report,f,indent=2,allow_nan=False)
    print(json.dumps(report))
    return 0 if report['status']=='hardware_probe_completed' else 1


if __name__=='__main__':raise SystemExit(main())
