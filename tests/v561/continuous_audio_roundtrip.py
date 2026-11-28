import sys
from v561 import continuous_audio_pb2 as wire
seal = wire.SegmentSeal.FromString(sys.stdin.buffer.read())
assert seal.identity.script_epoch == 9007199254740993
assert seal.identity.source_start_sample == 9007199254740992
assert seal.samples == 2 and len(seal.sha256) == 32
progress = wire.SegmentProgress(identity=seal.identity, received_samples=2,
    submitted_samples=2, played_samples=1, sealed=True, completed=False)
sys.stdout.buffer.write(progress.SerializeToString(deterministic=True))
