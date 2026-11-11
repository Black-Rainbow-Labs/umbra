import os
import sys
import unittest

sys.path.insert(0, os.environ["WORKSPACE_DIR"])

from pricing import Cart, Coupon
from pricing.money import to_cents


class ToCentsTests(unittest.TestCase):
    def test_values_that_truncate_in_floating_point(self):
        for dollars, cents in [(0.29, 29), (1.15, 115), (4.35, 435), (19.99, 1999), (2.3, 230), (8.2, 820), (0, 0)]:
            self.assertEqual(to_cents(dollars), cents, dollars)


class CartTests(unittest.TestCase):
    def test_percent_coupon_taxes_discounted_amount(self):
        cart = Cart(tax_rate=8, coupon=Coupon("percent", 10))
        cart.add("mug", 19.99, 3)
        self.assertEqual(cart.summary(), {"subtotal": 5997, "discount": 600, "tax": 432, "total": 5829})

    def test_fixed_coupon_taxes_discounted_amount(self):
        cart = Cart(tax_rate=10, coupon=Coupon("fixed", 2.30))
        cart.add("pen", 10.00, 2)
        self.assertEqual(cart.summary(), {"subtotal": 2000, "discount": 230, "tax": 177, "total": 1947})

    def test_fixed_coupon_never_exceeds_subtotal(self):
        cart = Cart(tax_rate=10, coupon=Coupon("fixed", 50))
        cart.add("pen", 12.00)
        self.assertEqual(cart.summary(), {"subtotal": 1200, "discount": 1200, "tax": 0, "total": 0})

    def test_quarter_off_with_tax(self):
        cart = Cart(tax_rate=6, coupon=Coupon("percent", 25))
        cart.add("tea", 8.20, 3)
        self.assertEqual(cart.total_cents(), 1956)

    def test_no_coupon_keeps_working(self):
        cart = Cart(tax_rate=8)
        cart.add("pen", 10.00, 2)
        cart.add("pad", 4.50)
        self.assertEqual(cart.total_cents(), 2646)

    def test_bad_quantity_rejected(self):
        with self.assertRaises(ValueError):
            Cart().add("pen", 1.0, 0)


if __name__ == "__main__":
    unittest.main()
