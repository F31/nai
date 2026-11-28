"""Explicit lite-model timestep schedule override for diagnostics; no production default."""


def config_sample_steps(config):
    steps = config.get('sample_steps')
    if steps is None:
        return None
    if type(steps) is not int or steps not in (2, 3, 4):
        raise ValueError('STEP_SCHEDULE')
    return steps


_SCHEDULES = {2: [1000, 500], 3: [1000, 500, 250], 4: [1000, 750, 500, 250]}


def override_sample_steps(backend, steps, transform=None):
    if steps not in _SCHEDULES:
        raise ValueError('STEP_SCHEDULE')
    params = backend.params
    if steps == params['sample_steps']:
        return False
    if transform is None:
        from flash_head.src.pipeline import flash_head_pipeline
        transform = flash_head_pipeline.timestep_transform
    pipeline = backend.pipeline
    base = _SCHEDULES[steps]
    # Upstream order: append the terminal 0 first, then transform every entry.
    timesteps = [backend.torch.tensor([t], device=pipeline.device) for t in base + [0.]]
    if pipeline.use_timestep_transform:
        shift = backend.inference.infer_params['sample_shift']
        timesteps = [transform(t, shift=shift, num_timesteps=pipeline.num_timesteps) for t in timesteps]
    pipeline.timesteps = timesteps
    params['sample_steps'] = steps
    backend.inference.infer_params['sample_steps'] = steps
    return True


def override_timesteps(backend, values, transform=None):
    if (type(values) is not list or not 1 <= len(values) <= 8
            or any(type(v) is not int or not 0 < v <= 1000 for v in values)
            or any(a <= b for a, b in zip(values, values[1:]))):
        raise ValueError('STEP_SCHEDULE')
    params = backend.params
    if transform is None:
        from flash_head.src.pipeline import flash_head_pipeline
        transform = flash_head_pipeline.timestep_transform
    pipeline = backend.pipeline
    # Same order as upstream: append the terminal 0 first, then transform every entry.
    timesteps = [backend.torch.tensor([t], device=pipeline.device) for t in values + [0.]]
    if pipeline.use_timestep_transform:
        shift = backend.inference.infer_params['sample_shift']
        timesteps = [transform(t, shift=shift, num_timesteps=pipeline.num_timesteps) for t in timesteps]
    pipeline.timesteps = timesteps
    params['sample_steps'] = len(values)
    backend.inference.infer_params['sample_steps'] = len(values)
    return True
