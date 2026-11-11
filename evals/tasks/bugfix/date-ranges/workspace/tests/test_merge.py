import unittest
from datetime import date

from dateranges import DateRange, merge_ranges, total_covered_days


def r(start_day, end_day):
    return DateRange(date(2025, 1, start_day), date(2025, 1, end_day))


class MergeTests(unittest.TestCase):
    def test_overlapping_sorted_ranges(self):
        self.assertEqual(merge_ranges([r(1, 5), r(4, 8)]), [r(1, 8)])

    def test_touching_ranges_merge(self):
        self.assertEqual(merge_ranges([r(1, 5), r(6, 10)]), [r(1, 10)])

    def test_disjoint_ranges_stay_separate(self):
        self.assertEqual(merge_ranges([r(1, 3), r(10, 12)]), [r(1, 3), r(10, 12)])

    def test_unsorted_input(self):
        self.assertEqual(merge_ranges([r(10, 15), r(1, 5), r(4, 8)]), [r(1, 8), r(10, 15)])

    def test_covered_days(self):
        self.assertEqual(total_covered_days([r(1, 5), r(4, 8)]), 8)


if __name__ == "__main__":
    unittest.main()
