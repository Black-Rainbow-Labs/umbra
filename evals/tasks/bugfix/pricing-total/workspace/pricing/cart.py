"""A shopping cart that prices its lines, applies a coupon and adds tax."""
from .money import to_cents
from .discounts import discount_cents
from .tax import tax_cents


class CartLine:
    """One product in the cart. unit_price is in dollars."""

    def __init__(self, name, unit_price, quantity):
        if quantity <= 0:
            raise ValueError("quantity must be positive")
        self.name = name
        self.unit_price = unit_price
        self.quantity = quantity


class Cart:
    """Holds lines, an optional coupon and a tax rate in percent."""

    def __init__(self, tax_rate=0.0, coupon=None):
        self.tax_rate = tax_rate
        self.coupon = coupon
        self.lines = []

    def add(self, name, unit_price, quantity=1):
        """Add a line to the cart."""
        self.lines.append(CartLine(name, unit_price, quantity))

    def subtotal_cents(self):
        """Sum of unit price times quantity for every line, in cents."""
        return sum(to_cents(line.unit_price) * line.quantity for line in self.lines)

    def summary(self):
        """Return subtotal, discount, tax and total, all in cents."""
        subtotal = self.subtotal_cents()
        discount = discount_cents(self.coupon, subtotal)
        tax = tax_cents(subtotal, self.tax_rate)
        return {
            "subtotal": subtotal,
            "discount": discount,
            "tax": tax,
            "total": subtotal - discount + tax,
        }

    def total_cents(self):
        """The amount the customer pays, in cents."""
        return self.summary()["total"]
