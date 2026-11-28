from pathlib import Path
import sys
import unittest

sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'scripts'))
from probe_ffn_fp8 import plan


class PlanTests(unittest.TestCase):
    def setUp(self):
        self.avatar=dict(width=768,height=768,frame_num=41)
        self.model=dict(dim=1536,ffn_dim=8960,vae_stride=[8,32,32],patch_size=[1,1,1])

    def test_current_geometry_is_derived_without_model_loading(self):
        self.assertEqual(plan(self.avatar,self.model,1073741824),[(3456,1536,8960),(3456,8960,1536)])

    def test_memory_and_unsupported_geometry_are_rejected_before_allocation(self):
        with self.assertRaisesRegex(ValueError,'FP8_PROBE_MEMORY'):plan(self.avatar,self.model,1)
        with self.assertRaisesRegex(ValueError,'FP8_PROBE_GRID'):plan(dict(self.avatar,frame_num=42),self.model,1073741824)
        with self.assertRaisesRegex(ValueError,'FP8_PROBE_ALIGNMENT'):plan(self.avatar,dict(self.model,dim=1537),1073741824)
        with self.assertRaisesRegex(ValueError,'FP8_PROBE_CONFIG'):plan(self.avatar,dict(self.model,dim=True),1073741824)


if __name__=='__main__':unittest.main()
