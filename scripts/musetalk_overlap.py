"""Issue the next UNet before the current decode is finished.

The functions supplied by the caller do the real work. This module stores no
pixels and does not open a device. `start_decode` must return before that
decode finishes, and the next `start_unet` is called before `finish_decode`.
"""


def run_pipeline(batch_count, start_unet, start_decode, finish_decode, blend):
    if type(batch_count) is not int or not 1 <= batch_count <= 512:
        raise ValueError("invalid batch count")
    pending = start_unet(0)
    for index in range(batch_count):
        token = start_decode(index, pending)
        pending = start_unet(index + 1) if index + 1 < batch_count else None
        blend(index, finish_decode(index, token))
