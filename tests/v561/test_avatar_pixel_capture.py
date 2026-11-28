import hashlib
from multiprocessing import shared_memory
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace as NS
import unittest

root=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(root/'scripts'))
sys.path.insert(0,str(root/'packages/contracts-python/src/generated'))
from v561.media_pb2 import FrameRef, PIXEL_FORMAT_BGRA8
from avatar_pixel_capture import Capture
from avatar_pixel_metrics import compare


class PixelTests(unittest.TestCase):
    def test_real_mapping_digest_and_complete_file(self):
        size=256*256*4
        memory=shared_memory.SharedMemory(create=True,size=size*3)
        data=bytes([3,2,1,255])*(256*256)
        memory.buf[:size]=data
        frame=FrameRef(pool_id='pool',epoch=7,sequence=0,source_sample=0,source_sample_rate=24000,
            pts_ns=0,width=256,height=256,stride_bytes=1024,pixel_format=PIXEL_FORMAT_BGRA8,
            slot_index=0,slot_generation=1,payload_bytes=size,content_sha256=hashlib.sha256(data).digest())
        try:
            with tempfile.TemporaryDirectory() as directory:
                path=Path(directory)/'frame.bgra'
                with Capture(path,256,256,960,7) as capture:
                    with self.assertRaisesRegex(ValueError,'INCOMPLETE'): capture.finish()
                    capture.accept(memory.name,frame,NS(check=lambda:None))
                    result=capture.finish()
                self.assertEqual(path.read_bytes(),data)
                self.assertEqual(result['sha256'],hashlib.sha256(data).hexdigest())
                self.assertFalse(capture.partial.exists())
                for field, value in [('epoch',8),('sequence',1),('slot_generation',2),('content_sha256',b'x'*32)]:
                    bad=FrameRef();bad.CopyFrom(frame);setattr(bad,field,value)
                    failed_path=Path(directory)/(field+'.bgra')
                    with Capture(failed_path,256,256,960,7) as failed:
                        with self.assertRaises(ValueError): failed.accept(memory.name,bad,NS(check=lambda:None))
                        with self.assertRaisesRegex(ValueError,'CAPTURE_INVALID'): failed.accept(memory.name,frame,NS(check=lambda:None))
                        with self.assertRaisesRegex(ValueError,'INCOMPLETE'): failed.finish()
                    self.assertFalse(failed_path.exists())
        finally:
            memory.close();memory.unlink()

    def test_incomplete_keeps_partial_and_bounds_reject(self):
        with tempfile.TemporaryDirectory() as directory:
            path=Path(directory)/'frame.bgra'
            with Capture(path,256,256,960,1): pass
            self.assertFalse(path.exists())
            self.assertTrue(path.with_suffix('.bgra.partial').exists())
            with self.assertRaisesRegex(ValueError,'CAPTURE_LIMIT'): Capture(Path(directory)/'large',1024,1024,144000,1)

    def test_metrics_exact_difference_and_integrity(self):
        with tempfile.TemporaryDirectory() as directory:
            a,b=Path(directory)/'a',Path(directory)/'b'
            a.write_bytes(bytes([0,0,0,255])*4)
            b.write_bytes(bytes([3,3,3,255])*4)
            hashes=[hashlib.sha256(p.read_bytes()).hexdigest() for p in (a,b)]
            result=compare(a,b,2,2,1,hashes)
            self.assertEqual(result['mae_0_255'],3.)
            self.assertEqual(result['rmse_0_255'],3.)
            self.assertAlmostEqual(result['psnr_db'],38.5883785,places=5)
            with self.assertRaisesRegex(ValueError,'METRICS_HASH'): compare(a,b,2,2,1,['bad','bad'])
            b.write_bytes(b'')
            with self.assertRaisesRegex(ValueError,'TRUNCATED'): compare(a,b,2,2,1,hashes)


if __name__=='__main__':unittest.main()
