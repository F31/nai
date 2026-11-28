import importlib.util
from pathlib import Path
import sys
from types import SimpleNamespace as NS
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
from cosyvoice_condition_cache import install


class Tensor:
    dtype = 'float32'
    def __init__(self, values): self.values = list(values); self.shape = (1, len(self.values))
    def numel(self): return len(self.values)
    def element_size(self): return 4
    def detach(self): return self
    def cpu(self): return self
    def contiguous(self): return self
    def numpy(self): return self
    def tobytes(self): return bytes(self.values)
    def clone(self): return Tensor(self.values)


class Frontend:
    def __init__(self): self.calls = 0; self.fail = False; self.condition_size = 2
    def _extract_text_token(self, text): return Tensor(text.encode()), Tensor([len(text)])
    def frontend_zero_shot(self, tts_text, prompt_text, prompt_wav, resample_rate, zero_shot_spk_id):
        self.calls += 1
        if self.fail: raise RuntimeError('frontend failure')
        text, length = self._extract_text_token(tts_text)
        return dict(text=text, text_len=length, embedding=Tensor(range(self.condition_size)))


class CacheTests(unittest.TestCase):
    def setUp(self):
        self.frontend = Frontend(); self.model = NS(frontend=self.frontend)
        self.cache = install(self.model)
    def call(self, text='one', prompt='prompt', wav=None, rate=24000, speaker=''):
        return self.frontend.frontend_zero_shot(text, prompt, Tensor([1,2]) if wav is None else wav, rate, speaker)
    def test_same_voice_different_reply_never_reuses_text(self):
        a = self.call(); b = self.call('two')
        self.assertEqual(b['text'].values, list(b'two'))
        self.assertEqual(a['text'].values, list(b'one'))
        self.assertEqual(self.frontend.calls, 1)
        self.assertEqual(self.cache.stats()['hits'], 1)
    def test_original_and_hit_mutations_do_not_pollute_cache(self):
        self.call()['embedding'].values[0] = 99
        a = self.call(); self.assertEqual(a['embedding'].values[0], 0)
        a['embedding'].values[0] = 88
        self.assertEqual(self.call()['embedding'].values[0], 0)
    def test_content_prompt_and_rate_changes_each_invalidate(self):
        self.call(); self.call(wav=Tensor([1,3])); self.call(prompt='new'); self.call(rate=16000)
        self.assertEqual(self.frontend.calls, 4)
        self.assertEqual(self.cache.stats()['entries'], 1)
    def test_single_entry_evicts_previous_reference(self):
        self.call(); self.call(prompt='other'); self.call()
        self.assertEqual(self.frontend.calls, 3)
    def test_native_speaker_bypasses_cache(self):
        self.call(speaker='native'); self.call(speaker='native')
        self.assertEqual(self.cache.stats()['entries'], 0)
        self.assertEqual(self.cache.stats()['bypasses'], 2)
    def test_disabled_and_large_input_bypass(self):
        self.cache.enabled = False; self.call(); self.call()
        self.cache.enabled = True; self.cache.max_bytes = 4; self.call(); self.call()
        self.assertEqual(self.frontend.calls, 4)
        self.assertEqual(self.cache.stats()['entries'], 0)
    def test_large_condition_not_retained(self):
        self.cache.max_bytes = 8; self.frontend.condition_size = 3
        self.call(); self.call()
        self.assertEqual(self.frontend.calls, 2)
        self.assertEqual(self.cache.stats()['entries'], 0)
    def test_failure_does_not_cache_partial_condition(self):
        self.frontend.fail = True
        with self.assertRaises(RuntimeError): self.call()
        self.assertEqual(self.cache.stats()['entries'], 0)
        self.frontend.fail = False; self.call()
        self.assertEqual(self.cache.stats()['misses'], 2)
    def test_duplicate_install_and_limits_rejected(self):
        with self.assertRaisesRegex(ValueError, 'ALREADY'): install(self.model)
        for limit in [0, True, 16*1024*1024+1]:
            with self.assertRaisesRegex(ValueError, 'LIMIT'): install(NS(frontend=Frontend()), limit)
    def test_strict_patcher(self):
        spec = importlib.util.spec_from_file_location('cache_patcher', ROOT/'scripts/prepare-cosyvoice-condition-cache.py')
        module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
        source = "if __name__ == '__main__':\n    _nai_first_pcm = observer(cosyvoice)\n    run()\n"
        patched = module.patch(source)
        self.assertLess(patched.index('_nai_install_condition_cache(cosyvoice)'), patched.index('    run()'))
        with self.assertRaisesRegex(ValueError, 'ALREADY'): module.patch(patched)
        with self.assertRaisesRegex(ValueError, 'EXPECTED_ONE'): module.patch('run()\n')
    def test_instance_isolation(self):
        self.call()
        other = Frontend(); other.condition_size = 3
        install(NS(frontend=other))
        result = other.frontend_zero_shot('one', 'prompt', Tensor([1,2]), 24000, '')
        self.assertEqual(len(result['embedding'].values), 3)
        self.assertEqual(other.calls, 1)
    def test_existing_stage_observer_still_completes_on_hit(self):
        from cosyvoice_first_pcm import install as observe
        frontend = Frontend(); model = NS(frontend=frontend)
        class Gate:
            def start(self, factory): return list(factory())
        gate = Gate(); records = []
        observe(model, gate, records.append)
        cache = install(model)
        def generate():
            frontend.frontend_zero_shot('one', 'prompt', Tensor([1,2]), 24000, '')
            yield b'\0\0'
        gate.start(generate); gate.start(generate)
        self.assertEqual(cache.stats()['hits'], 1)
        self.assertEqual([r['state'] for r in records], ['generator_completed']*2)
        self.assertEqual([r['samples'] for r in records], [1,1])


if __name__ == '__main__': unittest.main()
