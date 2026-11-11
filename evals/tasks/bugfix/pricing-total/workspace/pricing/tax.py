"""Sales tax."""
from .money import percent_of


def tax_cents(taxable_cents, rate_percent):
    """Return the tax in cents for a taxable amount at the given percent rate."""
    return percent_of(taxable_cents, rate_percent)
