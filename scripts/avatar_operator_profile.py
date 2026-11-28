"""Explicit profiler ranges and numeric summaries; no input tensor recording."""
from contextlib import contextmanager
import math


@contextmanager
def ranges(torch, pipeline):
    originals = []
    try:
        for name, owner, attr in [('feature',pipeline,'preprocess_audio'),
                ('denoise',pipeline.model,'forward'), ('decode',pipeline.vae,'decode'),
                ('motion_encode',pipeline.vae,'encode')]:
            original = getattr(owner, attr)
            own = attr in vars(owner)
            def measured(*args, _original=original, _name=name, **kwargs):
                with torch.profiler.record_function('nai.'+_name):
                    return _original(*args, **kwargs)
            originals.append((owner, attr, original, own))
            setattr(owner, attr, measured)
        yield
    finally:
        for owner, attr, original, own in reversed(originals):
            if own: setattr(owner, attr, original)
            else: delattr(owner, attr)


def summary(events, limit=20):
    operators, phases, devices = [], [], []
    if not 1 <= limit <= 20:
        raise ValueError('PROFILE_SUMMARY_LIMIT')
    for event in events:
        values = (event.self_device_time_total, event.device_time_total,
                  event.self_cpu_time_total)
        if not all(math.isfinite(x) and x >= 0 for x in values):
            raise ValueError('PROFILE_SUMMARY_VALUE')
        device = event.device_type.name
        row = dict(name=event.key, device_type=device, calls=event.count, self_device_us=values[0],
            inclusive_device_us=values[1], self_cpu_us=values[2])
        if event.key.startswith('nai.'):
            phases.append(row)
        elif values[0] > 0 and device == 'CPU':
            operators.append(row)
        elif values[0] > 0 and device == 'CUDA':
            devices.append(row)
    if not operators:
        raise ValueError('PROFILE_CUDA_EVENTS_MISSING')
    return dict(scope='profiler_overhead_not_realtime_benchmark', phases=phases,
        top_operators=sorted(operators,key=lambda row:row['self_device_us'],reverse=True)[:limit],
        top_device_events=sorted(devices,key=lambda row:row['self_device_us'],reverse=True)[:limit])


def trace_summary(document, limit=20):
    """Rank actual kernels once; assign phase via host external-id ancestry."""
    if not 1 <= limit <= 20: raise ValueError('PROFILE_SUMMARY_LIMIT')
    events=document['traceEvents']
    ranges=[e for e in events if e.get('cat')=='user_annotation' and e.get('name','').startswith('nai.')]
    hosts={e['args']['External id']:e for e in events
        if e.get('cat') in ('cpu_op','user_annotation') and 'External id' in e.get('args',{})}
    phases={}
    for e in events:
        if e.get('cat')!='kernel': continue
        duration=e['dur']
        if not math.isfinite(duration) or duration < 0: raise ValueError('PROFILE_SUMMARY_VALUE')
        host=hosts.get(e.get('args',{}).get('External id'))
        phase='outside_or_unresolved'
        if host:
            matched=[r for r in ranges if r.get('pid')==host.get('pid')
                     and r.get('tid')==host.get('tid')
                     and r['ts']<=host['ts']<r['ts']+r['dur']]
            if matched: phase=min(matched,key=lambda r:r['dur'])['name']
        table=phases.setdefault(phase,{})
        row=table.setdefault(e['name'],dict(name=e['name'],calls=0,kernel_us=0.))
        row['calls']+=1;row['kernel_us']+=duration
    if not phases: raise ValueError('PROFILE_CUDA_EVENTS_MISSING')
    return dict(scope='actual_kernel_sums_not_wall_time',phases={name:dict(
        kernel_us=sum(r['kernel_us'] for r in rows.values()),calls=sum(r['calls'] for r in rows.values()),
        top_kernels=sorted(rows.values(),key=lambda r:r['kernel_us'],reverse=True)[:limit])
        for name,rows in phases.items()})
