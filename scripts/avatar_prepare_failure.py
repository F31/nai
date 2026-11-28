"""Safe startup failure taxonomy; exception details stay in the worker log."""


def classify(error):
    current = error
    for _ in range(4):
        if isinstance(current, MemoryError) or getattr(current, 'winerror', None) == 1455:
            return 'HOST_MEMORY_EXHAUSTED'
        current = current.__cause__ or current.__context__
        if current is None:
            break
    message = str(error).lower()
    if isinstance(error, TimeoutError):
        return 'PREPARATION_TIMEOUT'
    if 'cuda' in message:
        if 'out of memory' in message:
            return 'CUDA_MEMORY_EXHAUSTED'
        return 'CUDA_PREPARATION_FAILED'
    return 'MODEL_PREPARATION_FAILED'
