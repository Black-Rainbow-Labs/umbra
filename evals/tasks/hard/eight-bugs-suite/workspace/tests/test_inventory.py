import unittest

from shiplib.inventory import Inventory, OutOfStock


class InventoryTests(unittest.TestCase):
    def test_available_for_unknown_sku_is_zero(self):
        self.assertEqual(Inventory().available("nope"), 0)

    def test_reserve_reduces_stock(self):
        inventory = Inventory({"A": 5})
        inventory.reserve("A", 2)
        self.assertEqual(inventory.available("A"), 3)

    def test_reserve_more_than_available_raises(self):
        inventory = Inventory({"A": 2})
        with self.assertRaises(OutOfStock):
            inventory.reserve("A", 3)

    def test_restock_adds_units_and_records_it(self):
        inventory = Inventory({"A": 1})
        inventory.restock("A", 4)
        self.assertEqual(inventory.available("A"), 5)
        self.assertEqual(inventory.audit[-1], ("restock", "A", 4))

    def test_two_inventories_do_not_share_audit(self):
        first = Inventory({"A": 5})
        first.restock("A", 1)
        second = Inventory({"A": 5})
        self.assertEqual(second.audit, [])


if __name__ == "__main__":
    unittest.main()
