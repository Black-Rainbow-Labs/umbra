"""Money helpers. All amounts inside the package are integer cents."""


def to_cents(amount):
    """Convert a dollar amount such as 19.99 into whole cents."""
    return int(amount * 100)


def from_cents(cents):
    """Convert whole cents back into a dollar amount."""
    return cents / 100


def format_cents(cents):
    """Format cents as a dollar string such as $1,234.50."""
    return "${:,.2f}".format(cents / 100)


def percent_of(cents, percent):
    """Return the given percent of an amount in cents, rounded to the nearest cent."""
    return int(round(cents * percent / 100))
