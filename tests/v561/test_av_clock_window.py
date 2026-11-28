"""Windows shared frames + actual silent audio child + visible native window."""
import os
from pathlib import Path
import sys
import unittest
import numpy as np
sys.path.insert(0,str(Path(__file__).resolve().parents[2]/'scripts'))
from flashhead_rust_transport import DiagnosticTransport

class ClockWindowTests(unittest.TestCase):
    def open(self, stop=0, audio=None):
        return DiagnosticTransport(16,16,3,16000,25,5,os.environ['NAI_CLOCK_WINDOW'],
            os.environ['NAI_MAPPING_READER'],os.environ['NAI_SILENT_PCM'],8,
            audio_executor=audio or os.environ['NAI_AUDIO_OUTPUT'],stop_after_ms=stop)

    def frames(self, transport):
        for index in range(48):
            transport.send_rgb(np.full((16,16,3),index,dtype=np.uint8))

    def test_independent_signed_clock_drives_real_window(self):
        transport=self.open()
        try:
            self.frames(transport)
            result=transport.finish()
            self.assertEqual((result['frames_sent'],result['frames_acked']),(48,48))
        except BaseException:
            transport.abort()
            raise

    def test_stop_does_not_finish_as_completed(self):
        transport=self.open(stop=900)
        try:
            with self.assertRaises((OSError,RuntimeError,TimeoutError)):
                self.frames(transport)
                transport.finish()
        finally: transport.abort()

    def test_missing_audio_executor_is_not_success(self):
        transport=self.open(audio=str(Path(os.environ['NAI_AUDIO_OUTPUT']).parent/'missing'/'nai-v561-audio-output.exe'))
        try:
            with self.assertRaises((OSError,RuntimeError,TimeoutError)):
                self.frames(transport)
                transport.finish()
        finally: transport.abort()

    def test_missing_source_frames_are_not_masked_by_audio_completion(self):
        transport=self.open()
        try:
            for index in range(8):
                transport.send_rgb(np.full((16,16,3),index,dtype=np.uint8))
            self.assertNotEqual(transport.process.wait(timeout=6),0)
        finally: transport.abort()

if __name__=='__main__':unittest.main()
