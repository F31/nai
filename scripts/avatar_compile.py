"""Fixed-shape compilation assembly and execution evidence, no readiness grant."""
from contextlib import contextmanager
import os
from pathlib import Path


def configure_windows_compiler(discover=None):
    """Discover installed MSVC/SDK for this diagnostic process only."""
    if discover is None:
        from avatar_windows_compiler import discover_compiler
        discover = discover_compiler
    found = discover()
    cc, includes, libraries = found['compiler'], found['includes'], found['libraries']
    if (not cc or not Path(cc).is_file() or not includes or not libraries
            or any(not Path(p).is_dir() for p in [*includes,*libraries])):
        raise RuntimeError('GEMM_NATIVE_COMPILER_UNAVAILABLE')
    os.environ['CC'] = str(cc)
    os.environ['PATH'] = str(Path(cc).parent)+os.pathsep+os.environ.get('PATH','')
    os.environ['INCLUDE'] = os.pathsep.join(includes)
    os.environ['LIB'] = os.pathsep.join(libraries)
    os.environ.update(found.get('environment', {}))
    return dict(compiler=str(cc), include_paths=len(includes), library_paths=len(libraries))


@contextmanager
def gemm_autotune_candidate(torch, pipeline):
    saved = list(pipeline.model.blocks)
    if not saved or any(not hasattr(block, '_orig_mod') for block in saved):
        raise ValueError('GEMM_REQUIRES_COMPILED_BLOCKS')
    try:
        for index, block in enumerate(saved):
            pipeline.model.blocks[index] = torch.compile(block._orig_mod, dynamic=False,
                options={'max_autotune_gemm':True,'max_autotune_gemm_backends':'TRITON,ATEN',
                    'autotune_local_cache':False})
        yield
    finally:
        for index, block in enumerate(saved):
            pipeline.model.blocks[index] = block

def install(torch, pipeline, eager_sdpa=False, blocks=False):
    torch._dynamo.config.suppress_errors = False
    if eager_sdpa:
        # Explicit measured graph boundary: keep the actual cuDNN SDPA call in
        # eager mode so its real output layout is observed on graph re-entry.
        # Never disable stride assertions or substitute a different algorithm.
        torch._dynamo.disallow_in_graph(torch.nn.functional.scaled_dot_product_attention)
    if blocks:
        if not len(pipeline.model.blocks):
            raise ValueError('NO_TRANSFORMER_BLOCKS')
        for index, block in enumerate(pipeline.model.blocks):
            pipeline.model.blocks[index] = torch.compile(block, dynamic=False)
    else:
        pipeline.model = torch.compile(pipeline.model, dynamic=False)
    # Match upstream LTX compilation boundary, outside latent sampling/normalization.
    pipeline.vae.model.encode = torch.compile(pipeline.vae.model.encode, dynamic=False)
    pipeline.vae.model.decode = torch.compile(pipeline.vae.model.decode, dynamic=False)


def require_evidence(counters, kernel_count):
    if counters.get('unique_graphs', 0) < 1 or kernel_count < 1:
        raise RuntimeError('NO_COMPILED_EXECUTION_EVIDENCE')


