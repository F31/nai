import ast
import importlib.util
from pathlib import Path
import unittest

spec=importlib.util.spec_from_file_location('precision',Path(__file__).resolve().parents[1]/'scripts/prepare-cosyvoice-precision.py')
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
SOURCE="""# retained license
if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    args = parser.parse_args()
    cosyvoice = AutoModel(model_dir=args.model_dir)
    start(cosyvoice)
"""


class PrecisionTests(unittest.TestCase):
    def test_explicit_flag_keeps_default_false_and_only_wires_model(self):
        text=module.prepare(SOURCE);tree=ast.parse(text)
        calls=[x for x in ast.walk(tree) if isinstance(x,ast.Call)]
        flag=next(x for x in calls if x.args and isinstance(x.args[0],ast.Constant) and x.args[0].value=='--fp16')
        self.assertEqual(ast.literal_eval(flag.keywords[0].value),'store_true')
        self.assertTrue(text.startswith('# retained license'))
        self.assertIn('fp16=args.fp16',text);self.assertIn('start(cosyvoice)',text)

    def test_unknown_duplicate_or_already_modified_source_refused(self):
        for source in [SOURCE.replace('model_dir=args.model_dir','model_dir=args.model_dir, fp16=True'),
                       SOURCE+'\nx=AutoModel(model_dir=args.model_dir)\n',module.prepare(SOURCE),
                       SOURCE.replace('parser.parse_args()','custom.parse_args()')]:
            with self.assertRaises(ValueError):module.prepare(source)

    def test_utf8_ast_offsets_preserve_non_ascii_assignment(self):
        text=module.prepare(SOURCE.replace('cosyvoice =','数字人 ='))
        self.assertIn('数字人 = AutoModel(model_dir=args.model_dir, fp16=args.fp16)',text)


if __name__=='__main__':unittest.main()
