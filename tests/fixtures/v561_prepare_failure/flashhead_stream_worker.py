"""Test-only startup failure injection. Never selected by product profiles."""
import sys
import types
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[3] / 'scripts'))
backend = types.ModuleType('flashhead_resident_backend')
def fail(_config):
    raise MemoryError('test-only private diagnostic')
backend.Backend = fail
warmup = types.ModuleType('avatar_compiled_warmup')
warmup.warmup = lambda *_: None
sys.modules['flashhead_resident_backend'] = backend
sys.modules['avatar_compiled_warmup'] = warmup
from flashhead_stream_worker import main
raise SystemExit(main())
