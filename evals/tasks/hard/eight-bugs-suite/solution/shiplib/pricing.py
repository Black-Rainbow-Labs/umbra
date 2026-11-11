"""Money helpers. Every amount is an integer number of cents."""

BULK_TIERS = ((50, 20), (10, 10))


def apply_discount(cents, percent):
    """Return cents reduced by percent, rounded half up to a whole cent."""
    return (cents * (100 - percent) + 50) // 100


def bulk_unit_price(unit_cents, quantity):
    """Return the unit price after the bulk discount.

    Ordering 10 or more units takes 10 percent off the unit price.
    Ordering 50 or more units takes 20 percent off the unit price.
    """
    for minimum, percent in BULK_TIERS:
        if quantity >= minimum:
            return apply_discount(unit_cents, percent)
    return unit_cents


def line_total(unit_cents, quantity):
    """Return the total for one order line, after the bulk discount."""
    return bulk_unit_price(unit_cents, quantity) * quantity


def add_tax(cents, rate_percent):
    """Return cents plus tax at rate_percent, with the tax rounded half up to a whole cent."""
    return cents + (cents * rate_percent + 50) // 100
