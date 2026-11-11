"""Date helpers for shipping estimates."""

import datetime


def parse_date(text):
    """Parse a date written as YYYY-MM-DD."""
    return datetime.date.fromisoformat(text)


def is_business_day(day):
    """Return True for Monday to Friday."""
    return day.weekday() < 5


def add_business_days(start, count):
    """Return the date that is count business days (Monday to Friday) after start.

    A count of 0 returns start unchanged, even when start is on a weekend.
    """
    day = start
    remaining = count
    while remaining > 0:
        day += datetime.timedelta(days=1)
        if is_business_day(day):
            remaining -= 1
    return day


def business_days_between(start, end):
    """Return how many business days (Monday to Friday) there are after start, up to and including end.

    So Friday to the next Monday is 1, Saturday to Monday is 1 and Friday to Saturday is 0.
    Returns 0 when end is not after start.
    """
    count = 0
    day = start + datetime.timedelta(days=1)
    while day <= end:
        if is_business_day(day):
            count += 1
        day += datetime.timedelta(days=1)
    return count
