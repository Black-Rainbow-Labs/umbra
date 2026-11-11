import unittest

from catalog import CatalogService, LookupCache, ProductStore


class CatalogServiceTest(unittest.TestCase):
    def setUp(self):
        self.store = ProductStore()
        self.cache = LookupCache()
        self.service = CatalogService(self.store, self.cache)
        self.service.import_rows([("Blue Widget", 500), ("Red Widget", 450), ("Green Gadget", 900)])

    def test_lookup_returns_product(self):
        self.assertEqual(self.service.lookup("Blue Widget")["price"], 500)
        self.assertIsNone(self.service.lookup("Nothing"))

    def test_second_lookup_is_a_hit(self):
        self.service.lookup("Red Widget")
        self.service.lookup("Red Widget")
        self.assertEqual((self.cache.hits, self.cache.misses), (1, 1))

    def test_update_price_is_visible_to_the_same_spelling(self):
        self.service.lookup("Blue Widget")
        self.service.update_price("Blue Widget", 600)
        self.assertEqual(self.service.lookup("Blue Widget")["price"], 600)

    def test_search_orders_by_name(self):
        names = [p["name"] for p in self.service.search("")]
        self.assertEqual(names, ["Blue Widget", "Green Gadget", "Red Widget"])


if __name__ == "__main__":
    unittest.main()
