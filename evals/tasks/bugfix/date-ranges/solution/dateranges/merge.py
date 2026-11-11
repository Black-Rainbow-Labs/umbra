"""Merging booked date ranges into the smallest list of separate ranges."""
from .model import DateRange


def merge_ranges(ranges):
    """Combine ranges that overlap or touch and return them ordered by start date.

    Ranges that share a day, or where one starts the day after the other ends, become
    one range. The input list is not changed.
    """
    merged = []
    for current in sorted(ranges, key=lambda item: (item.start, item.end)):
        if merged and merged[-1].overlaps_or_touches(current):
            last = merged.pop()
            merged.append(DateRange(last.start, max(last.end, current.end)))
        else:
            merged.append(current)
    return merged
