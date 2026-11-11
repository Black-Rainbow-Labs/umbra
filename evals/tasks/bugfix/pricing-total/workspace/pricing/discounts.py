"""Coupons and the discount they give on a subtotal."""
from .money import to_cents, percent_of


class Coupon:
    """A coupon is either a percentage off ("percent") or a dollar amount off ("fixed")."""

    def __init__(self, kind, value):
        if kind not in ("percent", "fixed"):
            raise ValueError("unknown coupon kind: %s" % kind)
        self.kind = kind
        self.value = value


def discount_cents(coupon, subtotal_cents):
    """Return how many cents the coupon takes off the subtotal. Never more than the subtotal."""
    if coupon is None:
        return 0
    if coupon.kind == "percent":
        amount = percent_of(subtotal_cents, coupon.value)
    else:
        amount = to_cents(coupon.value)
    return min(amount, subtotal_cents)
