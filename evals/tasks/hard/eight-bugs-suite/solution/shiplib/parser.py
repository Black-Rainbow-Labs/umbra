"""Parsing of order text."""

from decimal import ROUND_HALF_UP, Decimal


def parse_price(text):
    """Convert a price such as '19.99' or '$19.99' to whole cents."""
    cents = Decimal(text.strip().lstrip("$")) * 100
    return int(cents.to_integral_value(rounding=ROUND_HALF_UP))


def parse_order_line(line):
    """Parse 'SKU, quantity, unit price' into (sku, quantity, unit_cents).

    Blank lines and lines that start with # give None.
    """
    stripped = line.strip()
    if not stripped or stripped.startswith("#"):
        return None
    sku, quantity, price = [part.strip() for part in stripped.split(",")]
    return sku, int(quantity), parse_price(price)


def parse_order_lines(text):
    """Parse a whole order, one line per item, skipping blank and comment lines."""
    parsed = (parse_order_line(line) for line in text.splitlines())
    return [line for line in parsed if line is not None]
