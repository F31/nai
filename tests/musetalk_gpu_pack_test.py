import sys
import unittest
from pathlib import Path
import numpy as np
import torch
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
import musetalk_gpu_pack

class PackTest(unittest.TestCase):
    def test_matches_reference_for_float16_and_bounds(self):
        torch.manual_seed(8)
        value = (torch.rand(4,3,23,19,dtype=torch.float16) * 4 - 2)
        reference = (value / 2 + 0.5).clamp(0,1).detach().cpu().permute(0,2,3,1).float().numpy()
        reference = (reference * 255).round().astype("uint8")[...,::-1]
        np.testing.assert_array_equal(musetalk_gpu_pack.pack(value), reference)
    @unittest.skipUnless(torch.cuda.is_available(), "CUDA unavailable")
    def test_gpu_matches_cpu_reference(self):
        torch.manual_seed(7)
        value = torch.rand(2,3,23,19,dtype=torch.float16) * 3 - 1.5
        reference = (value/2+0.5).clamp(0,1).permute(0,2,3,1).float().numpy()
        reference = (reference*255).round().astype("uint8")[...,::-1]
        np.testing.assert_array_equal(musetalk_gpu_pack.pack(value.cuda()), reference)
if __name__ == "__main__": unittest.main()
