"""Explicit HTTP double; verifies local probe bounds, not model integration."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock,patch

spec=importlib.util.spec_from_file_location('probe',Path(__file__).resolve().parents[1]/'scripts/probe-cosyvoice-streaming.py')
probe=importlib.util.module_from_spec(spec);spec.loader.exec_module(probe)


class ProbeBounds(unittest.TestCase):
    def test_reference_larger_than_output_bound_is_accepted(self):
        with tempfile.TemporaryDirectory() as folder:
            root=Path(folder);ref=root/'ref.wav';ref.write_bytes(bytes(1024))
            headers={'X-Audio-Encoding':'pcm_s16le','X-Audio-Channels':'1','X-Sample-Rate':'24000'}
            response=Mock(status=200)
            response.getheader.side_effect=lambda k,d=None:headers.get(k,d)
            response.getheaders.return_value=list(headers.items())
            response.read1.side_effect=[bytes(4),b'']
            conn=Mock(sock=None);conn.getresponse.return_value=response
            argv=['probe','--endpoint','http://127.0.0.1:50000/inference_zero_shot',
                  '--reference',str(ref),'--prompt-text','test','--text','test',
                  '--output-dir',str(root/'out'),'--max-bytes','32']
            with patch('sys.argv',argv),patch.object(probe.http.client,'HTTPConnection',return_value=conn),contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(probe.main(),0)
            self.assertEqual(json.loads((root/'out/result.json').read_text())['bytes'],4)
            conn.request.assert_called_once()

    def test_reference_bound_refuses_before_http(self):
        with tempfile.TemporaryDirectory() as folder:
            root=Path(folder);ref=root/'ref.wav';ref.write_bytes(bytes(1024))
            argv=['probe','--endpoint','http://127.0.0.1:50000/inference_zero_shot',
                  '--reference',str(ref),'--prompt-text','test','--text','test',
                  '--output-dir',str(root/'out'),'--reference-max-bytes','64']
            with patch('sys.argv',argv),patch.object(probe.http.client,'HTTPConnection') as http:
                with self.assertRaisesRegex(ValueError,'reference exceeds'):probe.main()
                http.assert_not_called()


if __name__=='__main__':unittest.main()
