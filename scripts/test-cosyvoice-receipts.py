"""Bounded receipt state tests; no model imports."""
import hashlib
import unittest
from unittest.mock import patch
from cosyvoice_completion_service import Receipts


class Tests(unittest.TestCase):
    def test_identity_copy_and_terminal_latch(self):
        r = Receipts()
        key = r.begin()
        self.assertEqual(r.get(key)['state'], 'running')
        completed = dict(state='completed', samples=1, sha256=hashlib.sha256(b'\0\0').hexdigest())
        r.finish(key, completed)
        view = r.get(key)
        view['state'] = 'failed'
        r.finish(key, dict(state='failed', samples=0, sha256='0' * 64))
        self.assertEqual(r.get(key)['state'], 'completed')
        self.assertNotEqual(r.instance, Receipts().instance)

    def test_capacity_does_not_evict_running_and_completed_is_bounded(self):
        r = Receipts(capacity=1)
        key = r.begin()
        with self.assertRaises(RuntimeError):
            r.begin()
        r.finish(key, dict(state='failed', samples=0, sha256='0' * 64))
        second = r.begin()
        self.assertIsNone(r.get(key))
        self.assertEqual(len(r.records), 1)
        r.discard(second)
        self.assertIsNone(r.get(second))

    def test_expiry_and_malformed_id_fail_closed(self):
        r = Receipts(ttl_seconds=1)
        with patch('cosyvoice_completion_service.time.monotonic', return_value=10):
            key = r.begin()
        with patch('cosyvoice_completion_service.time.monotonic', return_value=11):
            self.assertIsNone(r.get(key))
        self.assertIsNone(r.get('../private'))

    def test_oversized_failed_generation_cannot_claim_completion(self):
        for samples in (480001,):
            r = Receipts()
            key = r.begin()
            r.finish(key, dict(state='completed', samples=samples, sha256='0' * 64))
            self.assertEqual(r.get(key)['state'], 'failed')


if __name__ == '__main__':
    unittest.main(verbosity=2)
