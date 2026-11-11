import unittest

from shiplib.shipping import base_shipping_cents, free_shipping, shipping_cents


class ShippingTests(unittest.TestCase):
    def test_light_parcel_costs_the_base(self):
        self.assertEqual(base_shipping_cents(500), 500)
        self.assertEqual(base_shipping_cents(100), 500)

    def test_zero_weight_is_free(self):
        self.assertEqual(base_shipping_cents(0), 0)

    def test_whole_steps_over_the_base(self):
        self.assertEqual(base_shipping_cents(750), 625)
        self.assertEqual(base_shipping_cents(1000), 750)

    def test_a_started_step_counts_as_a_whole_step(self):
        self.assertEqual(base_shipping_cents(501), 625)

    def test_local_zone_is_the_base_cost(self):
        self.assertEqual(shipping_cents(500, "local"), 500)

    def test_zone_cost_rounds_half_up(self):
        self.assertEqual(shipping_cents(800, "national"), 863)

    def test_international_zone(self):
        self.assertEqual(shipping_cents(500, "international"), 1500)

    def test_unknown_zone_raises(self):
        with self.assertRaises(ValueError):
            shipping_cents(500, "moon")

    def test_free_shipping_threshold(self):
        self.assertTrue(free_shipping(7500))
        self.assertFalse(free_shipping(7499))


if __name__ == "__main__":
    unittest.main()
