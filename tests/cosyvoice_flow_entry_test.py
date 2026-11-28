import importlib.util
from pathlib import Path
import unittest

spec=importlib.util.spec_from_file_location('entry',Path(__file__).resolve().parents[1]/'scripts/prepare-cosyvoice-flow-alignment.py')
entry=importlib.util.module_from_spec(spec);spec.loader.exec_module(entry)
SOURCE="""# retained license
if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    args = parser.parse_args()
    cosyvoice = AutoModel(model_dir=args.model_dir)
    _nai_condition_cache = _nai_install_condition_cache(cosyvoice)
    start(cosyvoice)
"""


class EntryTests(unittest.TestCase):
    def test_default_disabled_and_install_after_cache(self):
        s=entry.prepare(SOURCE)
        self.assertIn('type=float, default=None',s)
        self.assertLess(s.index('_nai_condition_cache ='),s.index('if args.flow_reference_min_seconds is not None:'))
        self.assertTrue(s.startswith('# retained license'))
        self.assertIn('AutoModel(model_dir=args.model_dir)',s)

    def test_unknown_and_duplicate_boundaries_refused(self):
        for s in [SOURCE.replace('_nai_install_condition_cache(cosyvoice)','Other(cosyvoice)'),
                  SOURCE.replace('    start(cosyvoice)','    _nai_condition_cache = _nai_install_condition_cache(cosyvoice)'),
                  entry.prepare(SOURCE)]:
            with self.assertRaises(ValueError):entry.prepare(s)


if __name__=='__main__':unittest.main()
