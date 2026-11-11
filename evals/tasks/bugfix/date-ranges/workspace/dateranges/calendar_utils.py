"""Calendar questions built on merged ranges."""
from datetime import date

from .model import DateRange, ONE_DAY
from .merge import merge_ranges


def total_covered_days(ranges):
    """Count the distinct days covered by the ranges. Overlapping days count once."""
    return sum(item.days() for item in merge_ranges(ranges))


def find_gaps(ranges, window):
    """Return the free stretches of the window that no range covers, ordered by start."""
    gaps = []
    cursor = window.start
    for item in merge_ranges(ranges):
        if item.end < cursor:
            continue
        if item.start > window.end:
            break
        if item.start > cursor:
            gaps.append(DateRange(cursor, item.start - ONE_DAY))
        cursor = max(cursor, item.end + ONE_DAY)
    if cursor <= window.end:
        gaps.append(DateRange(cursor, window.end))
    return gaps


def last_day_of_month(year, month):
    """Return the last date of the given month."""
    if month == 12:
        return date(year, 12, 31)
    return date(year, month + 1, 1) - ONE_DAY


def split_by_month(item):
    """Cut a range at month boundaries and return one range per month it touches."""
    pieces = []
    start = item.start
    while start <= item.end:
        month_end = last_day_of_month(start.year, start.month)
        pieces.append(DateRange(start, min(month_end, item.end)))
        start = month_end + ONE_DAY
    return pieces
