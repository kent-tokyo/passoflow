import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))

from run_range import resolve_run_range


class RunRangeTests(unittest.TestCase):
    def test_resolves_one_indexed_inclusive_bounds(self):
        self.assertEqual(resolve_run_range(5, None, None), (0, 5))
        self.assertEqual(resolve_run_range(5, 2, 4), (1, 4))
        self.assertEqual(resolve_run_range(5, 3, None), (2, 5))
        self.assertEqual(resolve_run_range(0, None, None), (0, 0))

    def test_rejects_invalid_or_out_of_bounds_ranges(self):
        for start, end in ((0, None), (-1, None), (None, 0), (4, 2), (6, None), (None, 6)):
            with self.assertRaises(ValueError):
                resolve_run_range(5, start, end)
        with self.assertRaises(ValueError):
            resolve_run_range(0, 1, None)


if __name__ == "__main__":
    unittest.main()
