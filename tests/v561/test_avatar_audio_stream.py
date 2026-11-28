import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]/'scripts'))
import numpy as np
import librosa
from avatar_audio_stream import AudioInput


class AudioStreamTests(unittest.TestCase):
    def test_partition_invariance_against_current_whole_source(self):
        rng = np.random.default_rng(57)
        for count in [1, 2, 3, 959, 960, 38400, 73921, 144000]:
            pcm = rng.integers(-32768, 32767, size=count, dtype=np.int16)
            expected = librosa.resample(pcm.astype(np.float32)/32768.0, orig_sr=24000, target_sr=16000)
            for stride in [158, 1920, 76800]:
                stream = AudioInput(40, 144000, lambda: None)
                blocks = []
                raw = pcm.astype('<i2').tobytes()
                for start in range(0, len(raw), stride):
                    stream.push(raw[start:start+stride], blocks.append)
                    self.assertLess(len(stream.pending), stream.block_size)
                stream.finish(count, blocks.append)
                actual = np.concatenate([b.samples[:b.valid] for b in blocks])
                np.testing.assert_allclose(actual, expected, rtol=0, atol=2e-7)
                self.assertEqual(len(actual), len(expected))
                offset = 0
                for block in blocks:
                    self.assertEqual(block.start, offset)
                    self.assertEqual(block.samples.dtype, np.float32)
                    self.assertEqual(len(block.samples), 25600)
                    self.assertTrue(np.all(block.samples[block.valid:] == 0))
                    offset += block.valid

    def test_first_packet_can_fill_32_frames_without_eos_padding(self):
        for frames, expected in [(32, 1), (40, 0)]:
            stream = AudioInput(frames, 144000, lambda: None)
            blocks = []
            stream.push(bytes(32640 * 2), blocks.append)
            self.assertEqual(len(blocks), expected)
            self.assertTrue(all(b.valid == frames * 640 for b in blocks))
            self.assertFalse(stream.ended)

    def test_output_before_eos_and_no_early_padding(self):
        stream = AudioInput(40, 144000, lambda: None)
        blocks = []
        for _ in range(3):
            stream.push(bytes(76800), blocks.append)
        self.assertGreater(len(blocks), 0)
        self.assertTrue(all(b.valid == 25600 for b in blocks))
        stream.finish(115200, blocks.append)
        self.assertEqual(sum(b.valid for b in blocks), 76800)

    def test_invalid_eos_format_and_limit_are_terminal(self):
        for bad in ['eos', 'format', 'limit']:
            stream = AudioInput(40, 960, lambda: None)
            blocks = []
            stream.push(bytes(100), blocks.append)
            with self.assertRaises(ValueError):
                if bad == 'eos': stream.finish(49, blocks.append)
                elif bad == 'format': stream.push(b'1', blocks.append)
                else: stream.push(bytes(2000), blocks.append)
            self.assertEqual(blocks, [])
            with self.assertRaisesRegex(ValueError, 'STREAM_ENDED'):
                stream.finish(50, blocks.append)

    def test_cancel_and_consumer_error_prevent_further_output(self):
        for consumer_error in [False, True]:
            cancelled = [False]
            def check():
                if cancelled[0]: raise RuntimeError('CANCELLED')
            stream = AudioInput(1, 144000, check)
            calls = []
            def emit(block):
                calls.append(block)
                if consumer_error: raise RuntimeError('CONSUMER')
                cancelled[0] = True
            with self.assertRaises(RuntimeError): stream.push(bytes(76800), emit)
            self.assertEqual(len(calls), 1)
            self.assertTrue(stream.ended)
            self.assertEqual(len(stream.pending), 0)

    def test_eos_is_single_and_empty_or_bad_geometry_rejected(self):
        for frames in [0, 98, True]:
            with self.assertRaises(ValueError): AudioInput(frames, 100, lambda: None)
        stream = AudioInput(1, 100, lambda: None)
        with self.assertRaises(ValueError): stream.finish(0, lambda b: None)
        stream = AudioInput(1, 100, lambda: None)
        stream.push(bytes(2), lambda b: None)
        stream.finish(1, lambda b: None)
        with self.assertRaises(ValueError): stream.finish(1, lambda b: None)


if __name__ == '__main__': unittest.main()
