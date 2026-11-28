from pathlib import Path
import sys
import threading
from types import SimpleNamespace as NS
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'scripts'))
from avatar_delivery import Delivery, intersection_ns
from avatar_worker_core import Job, Cancelled


class DeliveryTests(unittest.TestCase):
    def test_one_block_backpressure_and_ordered_drain(self):
        entered, release = threading.Event(), threading.Event()
        seen = []
        def send(image):
            entered.set()
            if not release.wait(2):
                raise RuntimeError('TEST_TIMEOUT')
            seen.append(image)
        job = Job(NS(epoch=1), 3)
        delivery = Delivery(job, NS(send_rgb=send))
        try:
            delivery.submit([1, 2])
            self.assertTrue(entered.wait(1))
            # Producer can execute here while delivery is blocked; no second slot.
            with self.assertRaisesRegex(RuntimeError, 'DELIVERY_OCCUPIED'):
                delivery.submit([3])
            release.set()
            delivery.wait()
            delivery.submit([3])
            delivery.wait()
            self.assertEqual(seen, [1, 2, 3])
        finally:
            release.set()
            delivery.close()
        self.assertFalse(job.cancelled.is_set())

    def test_failure_cancels_parent_and_is_observed(self):
        def fail(_):
            raise ValueError('RELEASE_INVALID')
        job = Job(NS(epoch=1), 2)
        delivery = Delivery(job, NS(send_rgb=fail))
        try:
            delivery.submit([1])
            with self.assertRaisesRegex(ValueError, 'RELEASE_INVALID'):
                delivery.wait()
            self.assertTrue(job.cancelled.is_set())
        finally:
            delivery.close()

    def test_close_cancels_and_joins_before_resource_cleanup(self):
        entered, exited = threading.Event(), threading.Event()
        job = Job(NS(epoch=1), 2)
        def send(_):
            entered.set()
            try:
                while not job.cancelled.wait(.01):
                    job.check()
                job.check()
            finally:
                exited.set()
        delivery = Delivery(job, NS(send_rgb=send))
        delivery.submit([1])
        self.assertTrue(entered.wait(1))
        delivery.close()
        self.assertTrue(exited.is_set())
        with self.assertRaises(Cancelled):
            job.check()

    def test_overlap_is_interval_intersection_not_total_delivery(self):
        self.assertEqual(intersection_ns([(10, 30), (40, 60)], [(0, 15), (25, 50)]), 20)
        self.assertEqual(intersection_ns([(10, 20)], [(20, 30)]), 0)


if __name__ == '__main__':
    unittest.main()
