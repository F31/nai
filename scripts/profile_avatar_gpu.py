"""Explicit local GPU experiment; in-process frame ACK is not Rust/window acceptance."""
import argparse
from contextlib import nullcontext, ExitStack
import hashlib
import json
import os
from pathlib import Path
import sys
import time
from types import SimpleNamespace as NS


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--config', type=Path, required=True)
    parser.add_argument('--pcm', type=Path, required=True)
    parser.add_argument('--report', type=Path, required=True)
    parser.add_argument('--compile', action='store_true', help='explicit fixed-shape maintenance experiment')
    parser.add_argument('--eager-sdpa', action='store_true', help='explicit compiler boundary around unchanged SDPA')
    parser.add_argument('--compile-blocks', action='store_true', help='bound compilation graphs to individual transformer blocks')
    parser.add_argument('--audio-projection-cache-bytes', type=int,
        help='explicit single-block diagnostic cache budget; requires compiled baseline')
    parser.add_argument('--capture-dir', type=Path, help='new diagnostic pixel directory; timings include capture IO')
    parser.add_argument('--operator-trace', type=Path, help='explicit one-block compiled operator trace; profiler overhead included')
    parser.add_argument('--gemm-autotune', action='store_true', help='explicit denoiser GEMM candidate; original maintenance deadline')
    parser.add_argument('--real-ffn-fp8',action='store_true',help='capture representative actual FFN inputs; never change rendered model outputs')
    parser.add_argument('--ffn-layers',type=int,nargs='+')
    parser.add_argument('--ffn-steps',type=int,nargs='+')
    parser.add_argument('--ffn-probe-bytes',type=int)
    parser.add_argument('--ffn-probe-repeats',type=int)
    parser.add_argument('--fp8-ffn-candidate',action='store_true',help='explicit full-render FFN candidate; no default activation')
    parser.add_argument('--denoiser-graph-bytes',type=int,help='explicit single-shape CUDA graph budget; optionally scoped FP8; no default activation')
    parser.add_argument('--graph-steady-pairs',type=int,help='explicit 1-3 alternating FP8/graph steady pairs; no recapture')
    parser.add_argument('--sample-steps',type=int,choices=[2,3,4],
        help='explicit lite timestep schedule override for the whole diagnostic process; no default change')
    parser.add_argument('--timesteps',type=str,
        help='explicit comma-separated diagnostic timestep list (e.g. 1000,500,250); no default change')
    args = parser.parse_args()
    if args.timesteps is not None and args.sample_steps is not None:
        parser.error('--timesteps and --sample-steps are mutually exclusive')
    if args.graph_steady_pairs is not None and (not 1<=args.graph_steady_pairs<=3
            or not args.denoiser_graph_bytes or not args.fp8_ffn_candidate):
        parser.error('steady pairs require combined FP8/graph and 1-3 pairs')
    if args.denoiser_graph_bytes is not None and (args.denoiser_graph_bytes<1 or not args.compile
            or not args.eager_sdpa or not args.compile_blocks
            or args.gemm_autotune or args.operator_trace or args.capture_dir
            or args.real_ffn_fp8 or args.audio_projection_cache_bytes is not None):
        parser.error('denoiser graph requires isolated compiled rendering and positive byte budget')
    if (args.eager_sdpa or args.compile_blocks) and not args.compile:
        parser.error('compiler boundary options require --compile')
    if args.audio_projection_cache_bytes is not None and (not args.compile
            or not args.eager_sdpa or not args.compile_blocks or args.audio_projection_cache_bytes < 1
            or args.capture_dir):
        parser.error('audio cache requires compile/eager-sdpa/compile-blocks and a positive budget; no capture')
    if args.operator_trace and (not args.compile or not args.eager_sdpa or not args.compile_blocks
            or args.audio_projection_cache_bytes is not None or args.capture_dir
            or not args.operator_trace.is_absolute() or args.operator_trace.parent != args.report.parent
            or args.operator_trace.exists()
            or args.operator_trace.with_name(args.operator_trace.stem+'-fp8.json').exists()):
        parser.error('operator trace requires isolated compiled baseline and new trace beside report')
    if args.gemm_autotune and (not args.compile or not args.eager_sdpa or not args.compile_blocks
            or args.audio_projection_cache_bytes is not None or args.operator_trace):
        parser.error('GEMM candidate requires isolated compile/eager-sdpa/compile-blocks baseline')
    if args.real_ffn_fp8 and (not args.compile or not args.eager_sdpa or not args.compile_blocks
            or args.gemm_autotune or args.operator_trace or args.audio_projection_cache_bytes is not None
            or args.capture_dir or not args.ffn_layers or not args.ffn_steps
            or not args.ffn_probe_bytes or args.ffn_probe_bytes<1
            or args.ffn_probe_repeats is None or not 2<=args.ffn_probe_repeats<=6):
        parser.error('real FFN probe requires isolated compiled baseline, explicit layers/steps/bytes/repeats')
    if args.fp8_ffn_candidate and (not args.compile or not args.eager_sdpa or not args.compile_blocks
            or args.real_ffn_fp8 or args.gemm_autotune or args.audio_projection_cache_bytes is not None
            or not args.ffn_layers or not args.ffn_probe_bytes or args.ffn_probe_bytes<1):
        parser.error('FP8 render requires isolated compiled baseline and explicit layers/cache budget')
    with args.config.open('rb') as source:
        raw = source.read(65537)
    if len(raw) > 65536:
        raise ValueError('CONFIG_LIMIT')
    config = json.loads(raw.decode('utf-8'))
    from flashhead_stream_worker import validate_budgets
    validate_budgets(config)
    with args.pcm.open('rb') as source:
        pcm = source.read(288001)
    if not pcm or len(pcm) > 288000 or len(pcm) % 2:
        raise ValueError('PCM_LIMIT')
    temp = Path(config['temp_dir'])
    if (not temp.is_absolute() or not temp.is_dir() or not args.report.is_absolute()
            or not args.report.parent.is_dir() or args.report.exists()):
        raise ValueError('PROFILE_PATH')
    if args.compile and args.report.with_suffix('.status.json').exists():
        raise ValueError('MAINTENANCE_HISTORY_EXISTS')
    if args.gemm_autotune and args.report.with_suffix('.gemm-status.json').exists():
        raise ValueError('MAINTENANCE_HISTORY_EXISTS')
    if args.fp8_ffn_candidate and args.report.with_suffix('.fp8-status.json').exists():
        raise ValueError('MAINTENANCE_HISTORY_EXISTS')
    if args.denoiser_graph_bytes and args.report.with_suffix('.graph-status.json').exists():
        raise ValueError('MAINTENANCE_HISTORY_EXISTS')
    if args.real_ffn_fp8 and any(args.report.with_suffix(suffix).exists() for suffix in
            ('.ffn-capture-status.json','.ffn-status.json','.ffn.json','.ffn-rows.jsonl','.ffn-capture.json')):
        raise ValueError('MAINTENANCE_HISTORY_EXISTS')
    if args.capture_dir:
        if not args.compile or not args.capture_dir.is_absolute() or args.capture_dir.parent != args.report.parent:
            raise ValueError('CAPTURE_PATH')
        args.capture_dir.mkdir(exist_ok=False)
    for name in ['TEMP', 'TMP', 'HOME', 'USERPROFILE']:
        os.environ[name] = str(temp)
    for name, directory in [('HF_HOME','huggingface'), ('TORCH_HOME','torch'),
                            ('CUDA_CACHE_PATH','cuda'), ('TRITON_CACHE_DIR','triton'), ('MPLCONFIGDIR','matplotlib'),
                            ('TORCHINDUCTOR_CACHE_DIR','inductor'), ('TORCH_EXTENSIONS_DIR','extensions')]:
        os.environ[name] = str(temp / directory)
    os.environ.update(HF_HUB_OFFLINE='1', TRANSFORMERS_OFFLINE='1', USE_TF='0', MPLBACKEND='Agg')
    root = Path(__file__).resolve().parents[1]
    sys.path[:0] = config['dependency_dirs'] + [config['upstream_dir'], str(root/'packages/contracts-python/src/generated')]
    os.chdir(config['upstream_dir'])
    from flashhead_resident_backend import Backend
    from avatar_worker_core import Job
    from avatar_gpu_stages import Stages, timing_sync_candidate
    from v561.media_pb2 import FrameRelease
    compiler_environment = None
    if args.gemm_autotune or args.real_ffn_fp8 or args.fp8_ffn_candidate or args.denoiser_graph_bytes:
        from avatar_compile import configure_windows_compiler
        compiler_environment = configure_windows_compiler()
    backend = Backend(config)
    if args.sample_steps is not None and args.sample_steps != backend.params['sample_steps']:
        from avatar_step_override import override_sample_steps
        override_sample_steps(backend, args.sample_steps)
    if args.timesteps is not None:
        from avatar_step_override import override_timesteps
        try:
            values = [int(v) for v in args.timesteps.split(',')]
        except ValueError:
            raise ValueError('STEP_SCHEDULE') from None
        override_timesteps(backend, values)
    if (args.operator_trace or args.real_ffn_fp8) and len(pcm)//2 != (config['frame_num']-backend.params['motion_frames_num'])*960:
        raise ValueError('PROFILE_REQUIRES_ONE_BLOCK')
    results = []
    modes = ['baseline-cold', 'baseline-warm', 'compile-maintenance', 'compiled-warm'] if args.compile else ['baseline-cold', 'baseline-warm', 'candidate-first', 'candidate-warm']
    if args.audio_projection_cache_bytes is not None:
        modes = ['compile-maintenance', 'compiled-reference', 'compiled-cache', 'compiled-confirm']
    if args.operator_trace:
        modes = ['compile-maintenance', 'compiled-reference', 'compiled-operator-trace', 'compiled-confirm']
    if args.gemm_autotune:
        modes = ['compile-maintenance', 'compiled-reference', 'gemm-maintenance', 'gemm-warm', 'compiled-confirm']
    real_capture=real_result=None
    if args.real_ffn_fp8:
        from avatar_real_ffn_probe import Capture
        real_capture=Capture(backend.torch,backend.pipeline,args.ffn_layers,args.ffn_steps,
            backend.params['sample_steps'],args.ffn_probe_bytes)
        modes=['compile-maintenance','compiled-reference','ffn-capture-maintenance','compiled-confirm']
    fp8_stats=fp8_evidence=None
    graph_stats=None
    graph_handle=None
    if args.denoiser_graph_bytes:
        modes=['compile-maintenance','compiled-reference','graph-maintenance','graph-warm','graph-warm-repeat','compiled-confirm']
    if args.fp8_ffn_candidate:
        modes=['compile-maintenance','compiled-reference','fp8-maintenance','fp8-warm','compiled-confirm']
        if args.operator_trace:
            modes=['compile-maintenance','compiled-reference','compiled-operator-trace',
                   'fp8-maintenance','fp8-warm','fp8-operator-trace','compiled-confirm']
        if args.denoiser_graph_bytes:
            modes=['compile-maintenance','compiled-reference','fp8-maintenance','fp8-warm',
                   'graph-maintenance','graph-warm','graph-warm-repeat','fp8-confirm','compiled-confirm']
            if args.graph_steady_pairs:
                steady=[]
                for index in range(1,args.graph_steady_pairs+1):
                    pair=['fp8-steady-'+str(index),'graph-steady-'+str(index)]
                    steady.extend(pair if index%2 else pair[::-1])
                modes[-2:-2]=steady
    with ExitStack() as candidate_stack, ExitStack() as graph_stack:
        for task, mode in enumerate(modes, 1):
            if mode.startswith(('fp8-steady-','graph-steady-')):
                graph_handle.enabled=mode.startswith('graph-steady-')
            if mode in ('fp8-confirm','compiled-confirm'):
                graph_stack.close()
            if mode == 'compiled-confirm':
                candidate_stack.close()
            # LTX VAE posterior.sample() also consumes torch's global RNG, while the
            # backend resets only its private denoising generator. Isolate all RNGs
            # for this diagnostic comparison; do not alter production task seeding.
            backend.torch.manual_seed(config['seed'])
            job = Job(NS(epoch=task, render=NS(samples=len(pcm)//2)),
                config['maintenance_timeout_ms']/1000 if mode in ('compile-maintenance','gemm-maintenance','ffn-capture-maintenance','fp8-maintenance','graph-maintenance') else config['timeout_ms']/1000)
            digests = []
            capture = None
            capture_report = None
            if args.capture_dir and mode in ('baseline-warm', 'compiled-warm','compiled-reference','gemm-warm','fp8-warm','compiled-confirm'):
                from avatar_pixel_capture import Capture
                capture = Capture(args.capture_dir/(mode+'.bgra'), config['width'], config['height'], len(pcm)//2, task)
            def ack(request, mapping, frame):
                if capture is not None:
                    capture.accept(mapping, frame, job)
                digests.append(frame.content_sha256.hex())
                job.releases.put_nowait(FrameRelease(pool_id=frame.pool_id, epoch=frame.epoch,
                    sequence=frame.sequence, slot_index=frame.slot_index, slot_generation=frame.slot_generation))
            candidate = timing_sync_candidate(backend.pipeline) if mode.startswith('candidate') else nullcontext(None)
            if mode == 'compile-maintenance':
                from avatar_compile_maintenance import Maintenance
                from avatar_compile import install
                candidate = Maintenance(args.report.with_suffix('.status.json'),config['maintenance_timeout_ms']/1000)
            if mode == 'gemm-maintenance':
                from avatar_compile_maintenance import Maintenance
                candidate = Maintenance(args.report.with_suffix('.gemm-status.json'),config['maintenance_timeout_ms']/1000)
            if mode == 'ffn-capture-maintenance':
                from avatar_compile_maintenance import Maintenance
                candidate = Maintenance(args.report.with_suffix('.ffn-capture-status.json'),config['maintenance_timeout_ms']/1000)
            if mode == 'fp8-maintenance':
                from avatar_compile_maintenance import Maintenance
                candidate = Maintenance(args.report.with_suffix('.fp8-status.json'),config['maintenance_timeout_ms']/1000)
            if mode == 'graph-maintenance':
                from avatar_compile_maintenance import Maintenance
                candidate = Maintenance(args.report.with_suffix('.graph-status.json'),config['maintenance_timeout_ms']/1000)
            if mode == 'compiled-cache':
                from avatar_audio_condition_cache import pipeline_candidate
                import math
                max_blocks = math.ceil(math.ceil(len(pcm)/2/960)/(config['frame_num']-backend.params['motion_frames_num']))
                candidate = pipeline_candidate(backend, args.audio_projection_cache_bytes, max_blocks)
            stage_context = nullcontext(NS(report=lambda: {})) if args.compile else Stages(backend.torch, backend.pipeline)
            profiler = None
            range_context = nullcontext()
            if mode in ('compiled-operator-trace','fp8-operator-trace'):
                from avatar_operator_profile import ranges
                profiler = backend.torch.profiler.profile(activities=[backend.torch.profiler.ProfilerActivity.CPU,
                    backend.torch.profiler.ProfilerActivity.CUDA],record_shapes=False,profile_memory=False,with_stack=False)
                range_context = ranges(backend.torch, backend.pipeline)
            with capture or nullcontext(), candidate as removed, stage_context as stages, profiler or nullcontext(), range_context, \
                    real_capture if mode=='ffn-capture-maintenance' else nullcontext():
                started = time.monotonic_ns()
                if mode == 'compile-maintenance':
                    install(backend.torch, backend.pipeline, args.eager_sdpa, args.compile_blocks)
                if mode == 'gemm-maintenance':
                    from avatar_compile import gemm_autotune_candidate
                    from torch._dynamo.utils import counters
                    from torch._inductor import metrics
                    previous_graphs = counters['stats']['unique_graphs']
                    previous_kernels = metrics.generated_kernel_count
                    candidate_stack.enter_context(gemm_autotune_candidate(backend.torch,backend.pipeline))
                if mode == 'fp8-maintenance':
                    from avatar_fp8_ffn_candidate import candidate as fp8_candidate
                    from torch._dynamo.utils import counters
                    from torch._inductor import metrics
                    previous_graphs=counters['stats']['unique_graphs'];previous_kernels=metrics.generated_kernel_count
                    fp8_stats=candidate_stack.enter_context(fp8_candidate(backend.torch,backend.pipeline,
                        args.ffn_layers,args.ffn_probe_bytes))
                if mode == 'graph-maintenance':
                    from avatar_denoiser_graph import candidate as graph_candidate
                    graph_handle=graph_stack.enter_context(graph_candidate(backend.torch,backend.pipeline,
                        args.denoiser_graph_bytes,lambda:job.check()))
                    graph_stats=graph_handle.stats
                count, stats = backend.render(pcm, job, ack)
                wall_ms = (time.monotonic_ns()-started)/1e6
                if mode == 'compile-maintenance':
                    from avatar_compile import require_evidence
                    from torch._dynamo.utils import counters
                    from torch._inductor import metrics
                    require_evidence(counters['stats'], metrics.generated_kernel_count)
                if mode == 'gemm-maintenance':
                    gemm_evidence = dict(new_unique_graphs=counters['stats']['unique_graphs']-previous_graphs,
                        new_generated_kernels=metrics.generated_kernel_count-previous_kernels)
                    if gemm_evidence['new_unique_graphs'] < 1:
                        raise RuntimeError('NO_NEW_GEMM_COMPILE_EVIDENCE')
                if mode == 'fp8-maintenance':
                    fp8_evidence=dict(new_unique_graphs=counters['stats']['unique_graphs']-previous_graphs,
                        new_generated_kernels=metrics.generated_kernel_count-previous_kernels)
                    if fp8_evidence['new_unique_graphs']<1 or fp8_evidence['new_generated_kernels']<1:
                        raise RuntimeError('NO_FP8_FFN_COMPILE_EVIDENCE')
                if capture is not None:
                    capture_report = capture.finish()
            operator_summary = None
            if profiler is not None:
                from avatar_operator_profile import summary, trace_summary
                trace_path=(args.operator_trace.with_name(args.operator_trace.stem+'-fp8.json')
                            if mode=='fp8-operator-trace' else args.operator_trace)
                if trace_path.exists(): raise ValueError('PROFILE_TRACE_EXISTS')
                profiler.export_chrome_trace(str(trace_path))
                operator_summary = summary(profiler.key_averages())
                with trace_path.open(encoding='utf8') as trace_file:
                    operator_summary['actual_kernels']=trace_summary(json.load(trace_file))
            record = {'mode': mode, 'wall_ms': wall_ms, 'frames': count, 'stages': stages.report(),
                      'capture': capture_report,
                      'removed': removed if isinstance(removed, dict) else None, 'frame_digests': digests,
                      'audio_condition_cache': removed if mode == 'compiled-cache' else None,
                      'operator_profile':operator_summary,
                      'compute_ms': stats.compute_ms, 'delivery_ms': stats.delivery_ms, 'overlap_ms': stats.overlap_ms}
            results.append(record)
            print(json.dumps({k:v for k,v in record.items() if k != 'frame_digests'}), flush=True)
            if mode=='ffn-capture-maintenance':
                from avatar_real_ffn_probe import compare
                with args.report.with_suffix('.ffn-capture.json').open('x',encoding='utf8') as f:
                    json.dump(dict(samples=len(real_capture.samples),calls=real_capture.calls,
                        bytes=real_capture.bytes,execution='selected_blocks_eager_for_capture'),f,indent=2)
                try:
                    with Maintenance(args.report.with_suffix('.ffn-status.json'),config['maintenance_timeout_ms']/1000), \
                            args.report.with_suffix('.ffn-rows.jsonl').open('x',encoding='utf8',buffering=1) as progress, \
                            backend.torch.no_grad():
                        real_result=compare(backend.torch,real_capture,args.ffn_probe_repeats,args.ffn_probe_bytes,
                            on_row=lambda row:progress.write(json.dumps(row,allow_nan=False)+'\n'))
                    with args.report.with_suffix('.ffn.json').open('x',encoding='utf8') as f:
                        json.dump(real_result,f,indent=2,allow_nan=False)
                finally:
                    real_capture.clear()
    report = {'scope': 'gpu_stage_diagnostic_inprocess_ack_not_public_output',
              'torch_version': backend.torch.__version__, 'gpu': backend.torch.cuda.get_device_name(0),
              'timings_include_capture_io_not_benchmark': bool(args.capture_dir),
              'cuda_events_mean': 'stream_elapsed_including_idle_not_active_kernel_time',
              'diagnostic_global_and_private_rng_seed': config['seed'],
              'pcm_sha256': hashlib.sha256(pcm).hexdigest(), 'samples': len(pcm)//2,
              'model_loads': 1, 'model_api_calls': 0, 'results': results,
              'sample_steps_effective': backend.params['sample_steps'],
              'sample_steps_override_requested': args.sample_steps,
              'gemm_autotune_requested': args.gemm_autotune,
              'compiler_environment': compiler_environment,
              'actual_ffn_probe':real_result,
              'fp8_ffn_candidate':fp8_stats,'fp8_ffn_compile_evidence':fp8_evidence,
              'denoiser_graph':graph_stats,
              'graph_steady_pairs':args.graph_steady_pairs,
              'graph_pixels_equal_reference': all(r['frame_digests']==next(x['frame_digests'] for x in results
                  if x['mode']==('fp8-warm' if args.fp8_ffn_candidate else 'compiled-reference'))
                  for r in results if r['mode'].startswith(('graph-','fp8-steady-')) or r['mode']=='fp8-confirm')
                  if args.denoiser_graph_bytes else None,
              'baseline_restored':results[-1]['frame_digests']==results[1]['frame_digests'],
              'gemm_compile_evidence': gemm_evidence if args.gemm_autotune else None,
              'candidate_pixels_equal_baseline': all(r['frame_digests'] == results[1]['frame_digests']
                  for r in results[2:] if (not args.gemm_autotune and not args.fp8_ffn_candidate)
                  or r['mode'] in ('gemm-warm','fp8-warm','compiled-confirm'))}
    if args.real_ffn_fp8:
        from avatar_real_ffn_probe import render_facts
        report.update(render_facts(results))
    if args.compile:
        from torch._dynamo.utils import counters
        from torch._inductor import metrics
        report['compile_stats'] = dict(counters['stats'])
        report['generated_kernel_count'] = metrics.generated_kernel_count
        report['eager_sdpa_boundary'] = args.eager_sdpa
        report['transformer_block_compile'] = args.compile_blocks
    with args.report.open('x', encoding='utf-8') as output:
        json.dump(report, output, ensure_ascii=False, indent=2)


if __name__ == '__main__':
    main()
