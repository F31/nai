"""Index two counted serial-512 windows and the decode cluster after each.

The counted window is the 100-frame measurement. The four diagnostic batches
run UNet first, synchronize, then record one decode_latents forward on the
profiler timeline: span, idle gaps, the longest gap, and the upper-middle
median. Those batches are not part of the 32 ms mark.
"""
from musetalk_unet_gaps import index_gap_traces as index_decode_traces

__all__ = ["index_decode_traces"]
