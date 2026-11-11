"""Interval helpers: merge, total length and gaps."""


def merge_intervals(intervals):
    """Return a sorted list of tuples with overlapping or touching intervals combined."""
    items = [(a, b) for a, b in intervals]
    for start, end in items:
        if start > end:
            raise ValueError("start greater than end: %r" % ((start, end),))
    items.sort()
    merged = []
    for start, end in items:
        if merged and start <= merged[-1][1]:
            if end > merged[-1][1]:
                merged[-1] = (merged[-1][0], end)
        else:
            merged.append((start, end))
    return merged


def total_length(intervals):
    """Return the length covered by the union of the intervals."""
    return sum(end - start for start, end in merge_intervals(intervals))


def gaps(intervals, lo, hi):
    """Return the uncovered parts of [lo, hi] as a sorted list of tuples."""
    if lo > hi:
        raise ValueError("lo greater than hi")
    result = []
    cursor = lo
    for start, end in merge_intervals(intervals):
        if end <= cursor:
            continue
        if start >= hi:
            break
        if start > cursor:
            result.append((cursor, start))
        cursor = max(cursor, end)
    if cursor < hi:
        result.append((cursor, hi))
    return result
