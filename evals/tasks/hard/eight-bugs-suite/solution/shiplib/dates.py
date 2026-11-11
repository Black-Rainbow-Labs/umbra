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
