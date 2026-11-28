"""Rust-generated signed control -> Python consumer -> signed EOS event.
No OS raw transport or GPU is exercised by this fixture.
"""
import struct
import sys
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))
from test_avatar_stream_state import KEY, PORTRAIT, PCM, StreamState, wire

data = Path(sys.argv[1]).read_bytes()
state = StreamState(KEY, PORTRAIT, 40, lambda: None, lambda: None)
offset = 0
events = []
while offset < len(data):
    size, = struct.unpack_from('<I', data, offset)
    offset += 4
    request = wire.Request.FromString(data[offset:offset+size])
    offset += size
    if request.WhichOneof('action') == 'chunk':
        assert state.handle(request, 1, PCM) is None
        events.append(state.consumed(1))
    else:
        events.append(state.handle(request))
assert offset == len(data) and state.ended and not state.failed
assert [e.kind for e in events] == [wire.KIND_INPUT_ACCEPTED, wire.KIND_CHUNK_CONSUMED, wire.KIND_EOS_VERIFIED]
Path(sys.argv[2]).write_bytes(events[-1].SerializeToString(deterministic=True))
