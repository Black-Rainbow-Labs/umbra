import unittest

from shiplib.inventory import Inventory, OutOfStock
from shiplib.orders import order_summary, place_order


class OrdersTests(unittest.TestCase):
    def test_place_order_prices_each_line(self):
        inventory = Inventory({"A": 10, "B": 10})
        items = place_order("A, 2, 3.00\nB, 1, 4.00\n", inventory)
        self.assertEqual(items, [("A", 600), ("B", 400)])
        self.assertEqual(inventory.available("A"), 8)

    def test_out_of_stock_reserves_nothing(self):
        inventory = Inventory({"A": 10, "B": 1})
        with self.assertRaises(OutOfStock):
            place_order("A, 2, 3.00\nB, 2, 4.00\n", inventory)
        self.assertEqual(inventory.available("A"), 10)

    def test_order_takes_all_remaining_stock(self):
        inventory = Inventory({"A": 2})
        items = place_order("A, 2, 5.00", inventory)
        self.assertEqual(items, [("A", 1000)])
        self.assertEqual(inventory.available("A"), 0)

    def test_summary_lists_largest_total_first(self):
        inventory = Inventory({"A": 5, "B": 5})
        text = order_summary("A, 1, 9.00\nB, 1, 12.00\n", inventory)
        self.assertEqual(text, "B: $12.00\nA: $9.00\nTOTAL: $21.00")


if __name__ == "__main__":
    unittest.main()
