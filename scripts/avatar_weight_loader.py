"""Scoped selection of the installed safetensors storage backend.

Called only during single-worker initialization, before model task threads exist.
No fallback: a rejected or unavailable backend must fail preparation visibly.
"""
from contextlib import contextmanager
from functools import wraps


@contextmanager
def weight_loader(backend, module=None):
    if backend not in ('mmap', 'pread'):
        raise ValueError('WEIGHT_LOADER_BACKEND')
    if module is None:
        import safetensors.torch as module
    original = module.load_file

    @wraps(original)
    def load(filename, device='cpu', **kwargs):
        if 'backend' in kwargs and kwargs['backend'] != backend:
            raise ValueError('WEIGHT_LOADER_CONFLICT')
        kwargs['backend'] = backend
        return original(filename, device=device, **kwargs)

    module.load_file = load
    try:
        yield
    finally:
        module.load_file = original
