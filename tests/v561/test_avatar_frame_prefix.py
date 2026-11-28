from pathlib import Path
import sys
import unittest
import os
import torch

sys.path.insert(0,str(Path(__file__).resolve().parents[2]/'scripts'))
from avatar_frame_prefix import to_host_prefix

class PrefixTests(unittest.TestCase):
    def test_tail_prefix_matches_full_conversion_pixel_for_pixel(self):
        source=torch.linspace(0,255,40*4*4*3).reshape(40,4,4,3)
        old=source.to(dtype=torch.uint8).cpu().numpy()[:8]
        new=to_host_prefix(source,8,torch.uint8)
        self.assertEqual(old.tobytes(),new.tobytes())
        self.assertEqual(new.shape,(8,4,4,3))
        self.assertEqual(source.shape,(40,4,4,3))

    def test_whole_block_and_oversized_remaining_preserve_values(self):
        source=torch.tensor([0.0,0.9,1.0,254.9,255.0,127.5]).reshape(2,1,1,3)
        for count in [2,10]:self.assertEqual(to_host_prefix(source,count,torch.uint8).tobytes(),source.to(dtype=torch.uint8).numpy().tobytes())

    def test_invalid_empty_prefix_rejected(self):
        for count in [0,-1]:
            with self.assertRaises(ValueError):to_host_prefix(torch.zeros(2,1,1,3),count,torch.uint8)

    @unittest.skipUnless(os.environ.get('NAI_TEST_GPU_PREFIX')=='1','explicit GPU prefix equivalence')
    def test_actual_cuda_prefix_matches_full_transfer(self):
        self.assertTrue(torch.cuda.is_available(),'CUDA_REQUIRED')
        source=torch.linspace(0,255,49*4*4*3,device='cuda').reshape(49,4,4,3)[9:]
        old=source.to(dtype=torch.uint8).cpu().numpy()[:8]
        new=to_host_prefix(source,8,torch.uint8)
        self.assertEqual(old.tobytes(),new.tobytes())

if __name__=='__main__':unittest.main()
