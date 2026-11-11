import os
import sys
import unittest
from datetime import date

sys.path.insert(0, os.environ["WORKSPACE_DIR"])

from dateranges import DateRange, merge_ranges, total_covered_days, find_gaps, split_by_month


def r(start_day, end_day, month=1):
    return DateRange(date(2025, month, start_day), date(2025, month, end_day))


class MergeTests(unittest.TestCase):
    def test_contained_range_keeps_the_longer_end(self):
        self.assertEqual(merge_ranges([r(1, 31), r(5, 10)]), [r(1, 31)])

    def test_contained_range_given_first(self):
        self.assertEqual(merge_ranges([r(5, 10), r(1, 31)]), [r(1, 31)])

    def test_unsorted_with_containment(self):
        got = merge_ranges([r(20, 22), r(1, 12), r(3, 5), r(11, 15), r(30, 31)])
        self.assertEqual(got, [r(1, 15), r(20, 22), r(30, 31)])

    def test_touching_in_reverse_order(self):
        self.assertEqual(merge_ranges([r(6, 10), r(1, 5)]), [r(1, 10)])

    def test_one_day_gap_stays_separate(self):
        self.assertEqual(merge_ranges([r(1, 4), r(6, 9)]), [r(1, 4), r(6, 9)])

    def test_empty_and_single(self):
        self.assertEqual(merge_ranges([]), [])
        self.assertEqual(merge_ranges([r(3, 4)]), [r(3, 4)])

    def test_input_is_not_changed(self):
        original = [r(10, 12), r(1, 3)]
        snapshot = list(original)
        merge_ranges(original)
        self.assertEqual(original, snapshot)

    def test_across_months(self):
        got = merge_ranges([r(20, 31), DateRange(date(2025, 2, 1), date(2025, 2, 3))])
        self.assertEqual(got, [DateRange(date(2025, 1, 20), date(2025, 2, 3))])


class CalendarTests(unittest.TestCase):
    def test_covered_days_with_containment(self):
        self.assertEqual(total_covered_days([r(5, 10), r(1, 31), r(2, 3)]), 31)

    def test_covered_days_overlap_counts_once(self):
        self.assertEqual(total_covered_days([r(1, 10), r(5, 12)]), 12)

    def test_gaps(self):
        got = find_gaps([r(20, 25), r(5, 10)], r(1, 31))
        self.assertEqual(got, [r(1, 4), r(11, 19), r(26, 31)])

    def test_gaps_ignore_ranges_outside_window(self):
        got = find_gaps([DateRange(date(2024, 12, 1), date(2024, 12, 31)), r(10, 12)], r(1, 15))
        self.assertEqual(got, [r(1, 9), r(13, 15)])

    def test_gaps_when_fully_covered(self):
        self.assertEqual(find_gaps([r(1, 31)], r(5, 10)), [])

    def test_split_by_month(self):
        item = DateRange(date(2024, 12, 20), date(2025, 2, 10))
        self.assertEqual(split_by_month(item), [
            DateRange(date(2024, 12, 20), date(2024, 12, 31)),
            DateRange(date(2025, 1, 1), date(2025, 1, 31)),
            DateRange(date(2025, 2, 1), date(2025, 2, 10)),
        ])


if __name__ == "__main__":
    unittest.main()
