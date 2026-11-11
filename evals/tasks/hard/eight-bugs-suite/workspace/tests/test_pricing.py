import unittest

from shiplib.pricing import add_tax, apply_discount, bulk_unit_price, line_total


class PricingTests(unittest.TestCase):
    def test_discount_of_zero_percent_changes_nothing(self):
        self.assertEqual(apply_discount(1999, 0), 1999)

    def test_discount_of_ten_percent(self):
        self.assertEqual(apply_discount(2000, 10), 1800)

    def test_discount_rounds_half_up(self):
        self.assertEqual(apply_discount(1005, 50), 503)

    def test_no_bulk_discount_below_ten_units(self):
        self.assertEqual(bulk_unit_price(1000, 9), 1000)

    def test_bulk_discount_starts_at_ten_units(self):
        self.assertEqual(bulk_unit_price(1000, 10), 900)

    def test_bigger_bulk_discount_for_sixty_units(self):
        self.assertEqual(bulk_unit_price(1000, 60), 800)

    def test_line_total_multiplies_the_discounted_price(self):
        self.assertEqual(line_total(1000, 20), 18000)

    def test_tax_is_added(self):
        self.assertEqual(add_tax(1000, 8), 1080)

    def test_tax_rounds_half_up(self):
        self.assertEqual(add_tax(1050, 5), 1103)


if __name__ == "__main__":
    unittest.main()
