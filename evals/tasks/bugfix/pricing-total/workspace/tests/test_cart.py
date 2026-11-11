import unittest

from pricing import Cart, Coupon


class CartTotalTests(unittest.TestCase):
    def test_total_without_coupon(self):
        cart = Cart(tax_rate=8)
        cart.add("pen", 10.00, 2)
        cart.add("pad", 4.50)
        self.assertEqual(cart.total_cents(), 2646)

    def test_percent_coupon_with_tax(self):
        cart = Cart(tax_rate=8, coupon=Coupon("percent", 10))
        cart.add("mug", 19.99, 3)
        self.assertEqual(cart.total_cents(), 5829)

    def test_empty_cart(self):
        self.assertEqual(Cart(tax_rate=8).total_cents(), 0)


if __name__ == "__main__":
    unittest.main()
