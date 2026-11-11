"""The DateRange value type. Both ends are inclusive."""
from dataclasses import dataclass
from datetime import date, timedelta

ONE_DAY = timedelta(days=1)


@dataclass(frozen=True)
class DateRange:
    """A span of whole days from start to end, both included."""

    start: date
    end: date

    def __post_init__(self):
        if self.end < self.start:
            raise ValueError("end %s is before start %s" % (self.end, self.start))

    def days(self):
        """Number of calendar days in the range, counting both ends."""
        return (self.end - self.start).days + 1

    def contains(self, day):
        """True when the day falls inside the range."""
        return self.start <= day <= self.end

    def overlaps_or_touches(self, other):
        """True when the ranges share a day or one starts the day after the other ends."""
        return self.start <= other.end + ONE_DAY and other.start <= self.end + ONE_DAY
