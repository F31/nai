from pathlib import Path
import subprocess
import sys
import unittest


class OutputTests(unittest.TestCase):
    def test_native_and_python_child_output_cannot_pollute_control(self):
        scripts = str(Path(__file__).resolve().parents[2]/'scripts')
        code = f'''
import os,sys,subprocess
sys.path.insert(0,{scripts!r})
from avatar_control_output import private_control_output
output=private_control_output()
print("PYTHON_LOG",flush=True)
os.write(1,b"NATIVE_LOG\\n")
subprocess.run([sys.executable,"-B","-c","import os;os.write(1,b'CHILD_LOG\\\\n')"],check=True)
output.write(b"ONLY_SIGNED_CONTROL_CHANNEL")
output.flush()
'''
        result = subprocess.run([sys.executable,'-B','-c',code],capture_output=True,check=True,timeout=10)
        self.assertEqual(result.stdout,b'ONLY_SIGNED_CONTROL_CHANNEL')
        for label in (b'PYTHON_LOG',b'NATIVE_LOG',b'CHILD_LOG'):
            self.assertIn(label,result.stderr)


if __name__=='__main__':unittest.main()
