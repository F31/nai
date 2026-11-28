import sys
import unittest
from pathlib import Path
from types import SimpleNamespace
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
from avatar_weight_loader import weight_loader


class LoaderTests(unittest.TestCase):
    def test_selection_preserves_device_and_restores(self):
        calls = []
        def load(path, **kwargs):
            calls.append((path, kwargs))
            return {'tensor': 1}
        module = SimpleNamespace(load_file=load)
        with weight_loader('pread', module):
            self.assertEqual(module.load_file('weights', device='cpu'), {'tensor': 1})
        self.assertIs(module.load_file, load)
        self.assertEqual(calls, [('weights', {'device': 'cpu', 'backend': 'pread'})])

    def test_exception_restores_without_fallback(self):
        def load(*args, **kwargs):
            raise OSError('failed')
        module = SimpleNamespace(load_file=load)
        with self.assertRaises(OSError):
            with weight_loader('pread', module):
                module.load_file('weights')
        self.assertIs(module.load_file, load)

    def test_unknown_and_conflicting_backend_rejected(self):
        module = SimpleNamespace(load_file=lambda *a, **k: None)
        with self.assertRaises(ValueError):
            with weight_loader('unknown', module):
                pass
        with weight_loader('pread', module):
            with self.assertRaises(ValueError):
                module.load_file('weights', backend='mmap')


if __name__ == '__main__':
    unittest.main()
