from pathlib import Path
import sys
import unittest
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'scripts'))
from flashhead_stream_worker import validate_budgets

class LimitTests(unittest.TestCase):
    def test_maintenance_does_not_raise_business_cap(self):
        config=dict(timeout_ms=45000,maintenance_timeout_ms=240000)
        validate_budgets(config)
        for field in config:
            invalid=dict(config);invalid[field]+=1
            with self.assertRaises(ValueError):validate_budgets(invalid)

    def test_missing_non_integer_and_zero_limits_fail_closed(self):
        with self.assertRaises(KeyError):validate_budgets({})
        for value in [0,-1,True,45000.0]:
            with self.assertRaises(ValueError):validate_budgets(dict(timeout_ms=value,maintenance_timeout_ms=240000))

if __name__=='__main__':unittest.main()
