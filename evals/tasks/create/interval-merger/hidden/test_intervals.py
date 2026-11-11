import io
import json
import os
import sys
import unittest

sys.path.insert(0, os.environ["WORKSPACE"])


def report(total, bad):
    print(json.dumps({"total": total, "bad": bad}))
    sys.exit(0)


try:
    import intervals
except Exception as error:
    report(1, ["import intervals failed: %s" % error])


class MergeTests(unittest.TestCase):
    def test_empty(self):
        self.assertEqual(intervals.merge_intervals([]), [])

    def test_basic(self):
        self.assertEqual(intervals.merge_intervals([(1, 3), (2, 6), (8, 10), (15, 18)]), [(1, 6), (8, 10), (15, 18)])

    def test_touching(self):
        self.assertEqual(intervals.merge_intervals([(1, 2), (2, 3)]), [(1, 3)])

    def test_unsorted(self):
        self.assertEqual(intervals.merge_intervals([(5, 6), (1, 2), (2, 4)]), [(1, 4), (5, 6)])

    def test_contained(self):
        self.assertEqual(intervals.merge_intervals([(1, 10), (2, 3), (4, 5)]), [(1, 10)])

    def test_zero_length(self):
        self.assertEqual(intervals.merge_intervals([(3, 3)]), [(3, 3)])
        self.assertEqual(intervals.merge_intervals([(1, 5), (5, 5)]), [(1, 5)])
        self.assertEqual(intervals.merge_intervals([(1, 2), (4, 4)]), [(1, 2), (4, 4)])

    def test_lists_in_tuples_out(self):
        result = intervals.merge_intervals([[1, 2], [2, 5], [7, 8]])
        self.assertEqual(result, [(1, 5), (7, 8)])
        self.assertTrue(all(type(item) is tuple for item in result))

    def test_floats_and_negatives(self):
        self.assertEqual(intervals.merge_intervals([(-2.5, -1), (-1.5, 0.5), (2, 3)]), [(-2.5, 0.5), (2, 3)])

    def test_invalid(self):
        with self.assertRaises(ValueError):
            intervals.merge_intervals([(1, 2), (5, 3)])

    def test_not_mutated(self):
        data = [(5, 6), (1, 2), (2, 4)]
        copy = list(data)
        intervals.merge_intervals(data)
        self.assertEqual(data, copy)

    def test_same_start(self):
        self.assertEqual(intervals.merge_intervals([(1, 4), (1, 2), (1, 9)]), [(1, 9)])


class TotalLengthTests(unittest.TestCase):
    def test_overlap_counted_once(self):
        self.assertEqual(intervals.total_length([(1, 3), (2, 6), (8, 10)]), 7)

    def test_empty(self):
        self.assertEqual(intervals.total_length([]), 0)

    def test_touching_and_float(self):
        self.assertAlmostEqual(intervals.total_length([(0, 1.5), (1.5, 2)]), 2)

    def test_invalid(self):
        with self.assertRaises(ValueError):
            intervals.total_length([(2, 1)])


class GapsTests(unittest.TestCase):
    def test_basic(self):
        self.assertEqual(intervals.gaps([(2, 4), (6, 8)], 0, 10), [(0, 2), (4, 6), (8, 10)])

    def test_no_intervals(self):
        self.assertEqual(intervals.gaps([], 3, 9), [(3, 9)])

    def test_clipped(self):
        self.assertEqual(intervals.gaps([(-5, 3), (9, 20)], 0, 10), [(3, 9)])

    def test_fully_covered(self):
        self.assertEqual(intervals.gaps([(-1, 5), (4, 12)], 0, 10), [])

    def test_touching_has_no_zero_gap(self):
        self.assertEqual(intervals.gaps([(0, 3), (3, 6)], 0, 6), [])

    def test_outside_only(self):
        self.assertEqual(intervals.gaps([(20, 30), (-9, -1)], 0, 10), [(0, 10)])

    def test_unsorted_overlapping(self):
        self.assertEqual(intervals.gaps([(6, 9), (1, 4), (3, 7)], 0, 10), [(0, 1), (9, 10)])

    def test_lo_equals_hi(self):
        self.assertEqual(intervals.gaps([(1, 2)], 5, 5), [])

    def test_lo_greater_than_hi(self):
        with self.assertRaises(ValueError):
            intervals.gaps([], 5, 1)

    def test_returns_tuples(self):
        result = intervals.gaps([[2, 3]], 0, 5)
        self.assertEqual(result, [(0, 2), (3, 5)])
        self.assertTrue(all(type(item) is tuple for item in result))


suite = unittest.defaultTestLoader.loadTestsFromModule(sys.modules[__name__])
result = unittest.TextTestRunner(stream=io.StringIO()).run(suite)
bad = ["%s: %s" % (test.id().split(".")[-1], text.strip().splitlines()[-1]) for test, text in result.failures + result.errors]
report(result.testsRun, bad)
