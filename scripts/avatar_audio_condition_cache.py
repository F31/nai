"""Explicit diagnostic-only, one-block audio projection reuse. No model fallback."""
from contextlib import contextmanager


@contextmanager
def projection_slot(torch, module, calls_limit, byte_limit):
    if (module.training or torch.is_grad_enabled() or type(calls_limit) is not int
            or calls_limit < 1 or type(byte_limit) is not int or byte_limit < 1):
        raise ValueError('CONDITION_CACHE_CONFIG')
    original = module.forward
    own = 'forward' in vars(module)
    inputs = result = None
    output_version = None
    failed = False
    stats = dict(calls=0, misses=0, hits=0, resident_bytes=0)
    def compute(*args, **kwargs):
        nonlocal inputs, result, output_version
        if kwargs or len(args) != 2 or not all(torch.is_tensor(x) for x in args):
            raise ValueError('CONDITION_CACHE_INPUT')
        if stats['calls'] >= calls_limit:
            raise ValueError('CONDITION_CACHE_CALL_LIMIT')
        stats['calls'] += 1
        if inputs is None:
            size = sum(x.numel()*x.element_size() for x in args)
            if size > byte_limit:
                raise ValueError('CONDITION_CACHE_BYTES')
            inputs = tuple(x.detach().clone() for x in args)
            result = original(*args)
            if not torch.is_tensor(result) or result.requires_grad:
                raise ValueError('CONDITION_CACHE_OUTPUT')
            size += result.numel()*result.element_size()
            if size > byte_limit:
                raise ValueError('CONDITION_CACHE_BYTES')
            output_version = result._version
            stats.update(misses=1, resident_bytes=size)
        else:
            if (result._version != output_version or any(
                    x.shape != saved.shape or x.dtype != saved.dtype or x.device != saved.device
                    or not torch.equal(x, saved) for x, saved in zip(args, inputs))):
                raise ValueError('CONDITION_CACHE_CHANGED')
            stats['hits'] += 1
        return result
    def forward(*args, **kwargs):
        nonlocal failed, inputs, result
        if failed:
            raise ValueError('CONDITION_CACHE_FAILED')
        try:
            return compute(*args, **kwargs)
        except BaseException:
            failed = True
            inputs = result = None
            raise
    module.forward = forward
    try:
        yield stats
    finally:
        if own:
            module.forward = original
        else:
            delattr(module, 'forward')
        inputs = result = None


@contextmanager
def pipeline_candidate(backend, byte_limit, max_blocks):
    """Fresh slot per run_pipeline call; immutable numeric summary only."""
    if type(max_blocks) is not int or max_blocks < 1:
        raise ValueError('CONDITION_CACHE_BLOCK_LIMIT')
    original = backend.inference.run_pipeline
    stats = dict(blocks=0, calls=0, misses=0, hits=0, peak_resident_bytes=0)
    def run(pipeline, embedding):
        if pipeline is not backend.pipeline or stats['blocks'] >= max_blocks:
            raise ValueError('CONDITION_CACHE_BLOCK_LIMIT')
        stats['blocks'] += 1
        with projection_slot(backend.torch, pipeline.model.audio_proj,
                backend.params['sample_steps'], byte_limit) as block:
            value = original(pipeline, embedding)
        for field in ('calls', 'misses', 'hits'):
            stats[field] += block[field]
        stats['peak_resident_bytes'] = max(stats['peak_resident_bytes'], block['resident_bytes'])
        return value
    backend.inference.run_pipeline = run
    try:
        yield stats
    finally:
        backend.inference.run_pipeline = original
