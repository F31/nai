from pathlib import Path
import sys
from types import SimpleNamespace as NS
from unittest.mock import Mock
from unittest.mock import patch
import tempfile
import os
import unittest

sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'scripts'))
from avatar_compile import gemm_autotune_candidate, configure_windows_compiler


class GemmCandidateTests(unittest.TestCase):
    def test_only_denoise_wrappers_change_and_restore(self):
        blocks=[NS(_orig_mod=object()),NS(_orig_mod=object())]
        pipeline=NS(model=NS(blocks=blocks.copy()),vae=object())
        compile=Mock(side_effect=lambda model,**kwargs:NS(_orig_mod=model))
        with gemm_autotune_candidate(NS(compile=compile),pipeline):
            self.assertIsNot(pipeline.model.blocks[0],blocks[0])
            self.assertIs(pipeline.model.blocks[0]._orig_mod,blocks[0]._orig_mod)
        self.assertEqual(pipeline.model.blocks,blocks)
        self.assertEqual(compile.call_count,2)
        for call in compile.call_args_list:
            self.assertEqual(call.kwargs,{'dynamic':False,'options':{
                'max_autotune_gemm':True,'max_autotune_gemm_backends':'TRITON,ATEN',
                'autotune_local_cache':False}})

    def test_compiler_is_discovered_before_changing_process_environment(self):
        root=Path(__file__).resolve().parents[1]/'target-windows/tool-cache/tmp/gemm-tests'
        root.mkdir(parents=True,exist_ok=True)
        with tempfile.TemporaryDirectory(dir=root) as directory, patch.dict(os.environ,{'PATH':'original'},clear=True):
            cc=Path(directory)/'cl.exe';cc.write_bytes(b'')
            result=configure_windows_compiler(lambda:dict(compiler=str(cc),includes=[directory],libraries=[directory]))
            self.assertEqual(os.environ['CC'],str(cc))
            self.assertEqual(result['include_paths'],1)
            snapshot=dict(os.environ)
            with self.assertRaisesRegex(RuntimeError,'GEMM_NATIVE_COMPILER_UNAVAILABLE'):
                configure_windows_compiler(lambda:dict(compiler=None,includes=[],libraries=[]))
            self.assertEqual(dict(os.environ),snapshot)

    def test_partial_install_and_execution_exception_restore_every_block(self):
        blocks=[NS(_orig_mod=object()),NS(_orig_mod=object())]
        pipeline=NS(model=NS(blocks=blocks.copy()))
        compile=Mock(side_effect=[object(),RuntimeError('compile failed')])
        with self.assertRaises(RuntimeError):
            with gemm_autotune_candidate(NS(compile=compile),pipeline): pass
        self.assertEqual(pipeline.model.blocks,blocks)
        with self.assertRaises(RuntimeError):
            with gemm_autotune_candidate(NS(compile=lambda *a,**k:object()),pipeline):
                raise RuntimeError('execute failed')
        self.assertEqual(pipeline.model.blocks,blocks)

    def test_uncompiled_model_is_rejected(self):
        with self.assertRaisesRegex(ValueError,'GEMM_REQUIRES_COMPILED_BLOCKS'):
            with gemm_autotune_candidate(NS(),NS(model=NS(blocks=[object()]))): pass


if __name__=='__main__':unittest.main()
