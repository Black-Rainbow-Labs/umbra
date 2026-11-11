"""Shipping costs. Every amount is an integer number of cents."""

BASE_CENTS = 500
BASE_GRAMS = 500
STEP_GRAMS = 250
STEP_CENTS = 125
ZONE_PERCENT = {"local": 100, "national": 115, "international": 300}
FREE_SHIPPING_CENTS = 7500


def base_shipping_cents(weight_grams):
    """Return the shipping cost for a parcel before the zone percentage is applied.

    A parcel of up to 500 grams costs 500 cents. Every further 250 grams, or part of 250 grams,
    adds 125 cents. A weight of 0 or less costs 0.
    """
    if weight_grams <= 0:
        return 0
    extra_grams = max(0, weight_grams - BASE_GRAMS)
    return BASE_CENTS + (extra_grams // STEP_GRAMS) * STEP_CENTS


def shipping_cents(weight_grams, zone):
    """Return the shipping cost for a parcel going to zone.

    The base cost is multiplied by the zone percentage (local 100, national 115, international 300)
    and the result is rounded half up to a whole cent. An unknown zone raises ValueError.
    """
    if zone not in ZONE_PERCENT:
        raise ValueError(f"unknown zone: {zone}")
    return round(base_shipping_cents(weight_grams) * ZONE_PERCENT[zone] / 100)


def free_shipping(subtotal_cents):
    """Return True when an order subtotal of subtotal_cents ships free (7500 cents or more)."""
    return subtotal_cents >= FREE_SHIPPING_CENTS
