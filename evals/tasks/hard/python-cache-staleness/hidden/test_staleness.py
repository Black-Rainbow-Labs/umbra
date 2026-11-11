import os
import sys
import unittest

sys.path.insert(0, os.environ["WORKSPACE_DIR"])

from catalog import CatalogService, LookupCache, ProductStore  # noqa: E402


class StalenessTest(unittest.TestCase):
    def setUp(self):
        self.store = ProductStore()
        self.cache = LookupCache()
        self.service = CatalogService(self.store, self.cache)
        self.service.import_rows([("Blue Widget", 500), ("Red Widget", 450), ("Green Gadget", 900), ("Straße Lamp", 1200)])
        self.cache.hits = 0
        self.cache.misses = 0
        self.cache.clear()

    def prices(self, spellings):
        return [self.service.lookup(s)["price"] for s in spellings]

    def test_whitespace_variants_share_one_entry(self):
        for spelling in ["Blue Widget", "blue  widget", " BLUE\twidget ", "Blue\nWidget"]:
            self.assertEqual(self.service.lookup(spelling)["id"], 1)
        self.assertEqual((self.cache.hits, self.cache.misses), (3, 1))
        self.assertEqual(len(self.cache.keys()), 1)

    def test_case_folding_shares_one_entry(self):
        self.assertEqual(self.service.lookup("Straße Lamp")["price"], 1200)
        self.assertEqual(self.service.lookup("STRASSE LAMP")["price"], 1200)
        self.assertEqual((self.cache.hits, self.cache.misses), (1, 1))

    def test_unicode_whitespace_shares_one_entry(self):
        self.service.lookup("Red Widget")
        self.service.lookup("Red Widget")
        self.service.lookup("Red  Widget")
        self.assertEqual((self.cache.hits, self.cache.misses), (2, 1))

    def test_missing_names_share_one_entry(self):
        self.assertIsNone(self.service.lookup("No  Such"))
        self.assertIsNone(self.service.lookup("no such "))
        self.assertEqual((self.cache.hits, self.cache.misses), (1, 1))

    def test_update_price_through_another_spelling_is_visible_to_all(self):
        spellings = ["Blue Widget", "blue  widget", " BLUE WIDGET", "Blue\tWidget"]
        self.assertEqual(self.prices(spellings), [500] * 4)
        self.service.update_price("BLUE   widget ", 999)
        self.assertEqual(self.prices(spellings), [999] * 4)

    def test_bulk_update_prices_invalidates_every_spelling(self):
        spellings = ["Red Widget", "red  widget", "RED WIDGET "]
        self.assertEqual(self.prices(spellings), [450] * 3)
        self.assertEqual(self.service.bulk_update_prices({"RED   widget": 460, "Blue Widget": 510}), 2)
        self.assertEqual(self.prices(spellings), [460] * 3)
        self.assertEqual(self.service.lookup("blue widget")["price"], 510)

    def test_bulk_update_prices_does_not_clear_unrelated_entries(self):
        self.service.lookup("Green Gadget")
        self.service.lookup("Red Widget")
        self.service.search("green")
        self.cache.hits = 0
        self.cache.misses = 0
        self.service.bulk_update_prices({"Red Widget": 460})
        self.service.lookup("Green Gadget")
        self.service.search("Green")
        self.assertEqual((self.cache.hits, self.cache.misses), (2, 0))
        self.service.lookup("red widget")
        self.assertEqual((self.cache.hits, self.cache.misses), (2, 1))

    def test_bulk_update_prices_refreshes_cached_searches(self):
        self.assertEqual([p["price"] for p in self.service.search("blue")], [500])
        self.assertEqual(len(self.service.search("")), 4)
        self.service.bulk_update_prices({"blue  WIDGET": 777})
        self.assertEqual([p["price"] for p in self.service.search("blue")], [777])
        self.assertEqual([p["price"] for p in self.service.search("")], [p["price"] for p in self.store.search("")])
        self.assertIn(777, [p["price"] for p in self.service.search("")])

    def test_search_keys_are_normalized(self):
        self.service.search("Blue ")
        self.service.search("blue")
        self.service.search("  BLUE")
        self.assertEqual((self.cache.hits, self.cache.misses), (2, 1))
        self.service.search("blue w")
        self.service.search("Blue   W")
        self.assertEqual((self.cache.hits, self.cache.misses), (3, 2))

    def test_update_price_refreshes_searches_with_odd_spelled_prefixes(self):
        self.service.search("  BLUE  w")
        self.service.search("b")
        self.service.update_price("blue widget", 1)
        self.assertEqual(self.service.search("blue   w")[0]["price"], 1)
        self.assertEqual(self.service.search("B")[0]["price"], 1)

    def test_rename_refreshes_old_and_new_names_and_searches(self):
        self.assertIsNotNone(self.service.lookup("Blue Widget"))
        self.assertIsNone(self.service.lookup("Azure  Widget"))
        self.assertEqual([p["name"] for p in self.service.search("azure")], [])
        self.assertEqual([p["name"] for p in self.service.search("blue")], ["Blue Widget"])
        self.service.rename("blue   widget", "Azure  Widget")
        self.assertIsNone(self.service.lookup("Blue Widget"))
        self.assertEqual(self.service.lookup("azure widget")["name"], "Azure Widget")
        self.assertEqual([p["name"] for p in self.service.search("azure")], ["Azure Widget"])
        self.assertEqual([p["name"] for p in self.service.search("blue")], [])

    def test_add_refreshes_cached_misses_and_searches(self):
        self.assertIsNone(self.service.lookup("Gizmo Pro"))
        self.assertEqual(self.service.search("giz"), [])
        self.service.add("  GIZMO   PRO ", 300)
        self.assertEqual(self.service.lookup("gizmo pro")["price"], 300)
        self.assertEqual([p["name"] for p in self.service.search("GIZ")], ["GIZMO PRO"])

    def test_remove_refreshes_lookups_and_searches(self):
        self.assertIsNotNone(self.service.lookup("Green Gadget"))
        self.assertEqual(len(self.service.search("g")), 1)
        self.service.remove("GREEN  gadget")
        self.assertIsNone(self.service.lookup("green gadget"))
        self.assertIsNone(self.service.lookup("Green Gadget"))
        self.assertEqual(self.service.search("g"), [])

    def test_unrelated_entries_stay_cached_after_single_updates(self):
        self.service.lookup("Green Gadget")
        self.service.search("gad")
        self.service.search("zzz")
        self.cache.hits = 0
        self.cache.misses = 0
        self.service.update_price("Blue Widget", 1)
        self.service.lookup("green  gadget")
        self.service.search("GAD")
        self.service.search("zzz")
        self.assertEqual((self.cache.hits, self.cache.misses), (3, 0))

    def test_names_module_behavior_is_unchanged(self):
        from catalog.names import display_name, normalize_name
        self.assertEqual(normalize_name("  Blue \t Widget\n"), "blue widget")
        self.assertEqual(normalize_name("STRASSE"), normalize_name("Straße"))
        self.assertEqual(display_name("  Blue \t Widget\n"), "Blue Widget")


if __name__ == "__main__":
    unittest.main(verbosity=2)
