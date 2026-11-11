import os
import sys
import unittest

sys.path.insert(0, os.environ["WORKSPACE_DIR"])

from catalog import CatalogService, LookupCache, PriceReport, ProductStore  # noqa: E402


class StalenessTest(unittest.TestCase):
    def setUp(self):
        self.store = ProductStore()
        self.cache = LookupCache()
        self.service = CatalogService(self.store, self.cache)
        self.report = PriceReport(self.service)
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

    def reset_counters(self):
        self.cache.hits = 0
        self.cache.misses = 0

    def test_changing_a_returned_lookup_does_not_change_later_lookups(self):
        first = self.service.lookup("Blue Widget")
        first["price"] = 1
        first["name"] = "Hacked"
        second = self.service.lookup("blue  widget")
        self.assertEqual((second["price"], second["name"]), (500, "Blue Widget"))
        second["price"] = 2
        self.assertEqual(self.service.lookup("BLUE WIDGET")["price"], 500)
        self.assertEqual((self.cache.hits, self.cache.misses), (2, 1))

    def test_changing_returned_search_results_does_not_change_later_searches(self):
        first = self.service.search("")
        first[0]["price"] = 1
        first.append({"id": 99, "name": "Fake", "price": 1})
        first.reverse()
        second = self.service.search("")
        self.assertEqual([p["name"] for p in second], ["Blue Widget", "Green Gadget", "Red Widget", "Straße Lamp"])
        self.assertEqual([p["price"] for p in second], [500, 900, 450, 1200])
        second.clear()
        self.assertEqual(len(self.service.search(" ")), 4)
        self.assertEqual((self.cache.hits, self.cache.misses), (2, 1))

    def test_total_price_sums_matching_products_and_shares_entries_between_spellings(self):
        self.assertEqual(self.report.total_price(""), 3050)
        self.assertEqual(self.report.total_price("  "), 3050)
        self.assertEqual(self.report.total_price("BLUE"), 500)
        self.assertEqual(self.report.total_price("blue "), 500)
        self.assertEqual(self.report.total_price("nothing"), 0)
        self.assertEqual(self.report.total_price("Nothing"), 0)
        self.assertEqual((self.cache.hits, self.cache.misses), (3, 3))

    def test_update_price_refreshes_cached_totals_and_keeps_unrelated_ones(self):
        self.assertEqual(self.report.total_price("blue"), 500)
        self.assertEqual(self.report.total_price(""), 3050)
        self.assertEqual(self.report.total_price("green"), 900)
        self.reset_counters()
        self.service.update_price("BLUE   widget", 600)
        self.assertEqual(self.report.total_price("Blue"), 600)
        self.assertEqual(self.report.total_price(" "), 3150)
        self.assertEqual((self.cache.hits, self.cache.misses), (0, 2))
        self.assertEqual(self.report.total_price("green"), 900)
        self.assertEqual((self.cache.hits, self.cache.misses), (1, 2))

    def test_add_remove_and_rename_refresh_cached_totals(self):
        self.assertEqual(self.report.total_price("gizmo"), 0)
        self.assertEqual(self.report.total_price("red"), 450)
        self.assertEqual(self.report.total_price(""), 3050)
        self.service.add("Gizmo  Pro", 300)
        self.assertEqual(self.report.total_price("GIZMO"), 300)
        self.assertEqual(self.report.total_price(""), 3350)
        self.service.rename("gizmo pro", "Red Gizmo")
        self.assertEqual(self.report.total_price("gizmo"), 0)
        self.assertEqual(self.report.total_price("red"), 750)
        self.service.remove("RED  gizmo")
        self.assertEqual(self.report.total_price("red"), 450)
        self.assertEqual(self.report.total_price(""), 3050)

    def test_bulk_update_prices_refreshes_cached_totals(self):
        self.assertEqual(self.report.total_price("re"), 450)
        self.assertEqual(self.report.total_price("gr"), 900)
        self.service.bulk_update_prices({"RED  widget": 1, "green gadget": 2})
        self.assertEqual(self.report.total_price("re"), 1)
        self.assertEqual(self.report.total_price("gr"), 2)

    def test_bulk_update_prices_that_fails_in_the_middle_leaves_nothing_stale(self):
        spellings = ["Blue Widget", "blue  widget"]
        self.assertEqual(self.prices(spellings), [500, 500])
        self.assertEqual(self.service.lookup("Red Widget")["price"], 450)
        self.assertEqual(self.report.total_price("b"), 500)
        with self.assertRaises(KeyError):
            self.service.bulk_update_prices({"BLUE widget": 510, "Missing Thing": 1, "Red Widget": 460})
        self.assertEqual(self.prices(spellings), [510, 510])
        self.assertEqual(self.report.total_price("b"), 510)
        self.assertEqual(self.service.lookup("red widget")["price"], 450)

    def test_bulk_update_prices_with_two_spellings_of_one_product_keeps_the_last(self):
        self.assertEqual(self.service.lookup("Blue Widget")["price"], 500)
        self.service.bulk_update_prices({"Blue Widget": 1, "blue   WIDGET": 2})
        self.assertEqual(self.service.lookup("blue widget")["price"], 2)

    def test_import_rows_that_fails_in_the_middle_leaves_nothing_stale(self):
        self.assertIsNone(self.service.lookup("Gizmo One"))
        self.assertIsNone(self.service.lookup("Gizmo Two"))
        self.assertEqual(self.service.search("gizmo"), [])
        with self.assertRaises(ValueError):
            self.service.import_rows([("gizmo  one", 1), ("Gizmo Two", 2), ("BLUE WIDGET", 3)])
        self.assertEqual(self.service.lookup("Gizmo One")["price"], 1)
        self.assertEqual(self.service.lookup("gizmo two")["price"], 2)
        self.assertEqual([p["name"] for p in self.service.search("GIZMO")], ["gizmo one", "Gizmo Two"])

    def test_discount_changes_matching_prices_and_returns_the_count(self):
        self.assertEqual(self.service.discount("  RE", 10), 1)
        self.assertEqual(self.store.find("red widget")["price"], 405)
        self.assertEqual(self.service.discount("", 50), 4)
        self.assertEqual(self.prices(["Blue Widget", "red widget", "GREEN  gadget", "strasse lamp"]), [250, 202, 450, 600])
        self.assertEqual(self.service.discount("nothing", 10), 0)
        self.assertEqual(self.prices(["Blue Widget"]), [250])

    def test_discount_rounds_down_to_whole_cents(self):
        self.service.add("Penny", 1)
        self.service.add("Odd", 999)
        self.service.discount("penny", 1)
        self.service.discount("odd", 15)
        self.assertEqual(self.service.lookup("penny")["price"], 0)
        self.assertEqual(self.service.lookup("ODD")["price"], 849)
        self.assertIsInstance(self.service.lookup("odd")["price"], int)

    def test_discount_rejects_bad_percentages_without_changing_anything(self):
        self.assertEqual(self.service.lookup("Blue Widget")["price"], 500)
        self.assertEqual(self.report.total_price(""), 3050)
        self.reset_counters()
        for bad in (0, 100, 150, -5, 2.5, "10", None, True):
            with self.assertRaises(ValueError, msg=repr(bad)):
                self.service.discount("blue", bad)
        self.assertEqual(self.store.find("Blue Widget")["price"], 500)
        self.assertEqual(self.service.lookup("blue widget")["price"], 500)
        self.assertEqual(self.report.total_price(" "), 3050)
        self.assertEqual((self.cache.hits, self.cache.misses), (2, 0))

    def test_discount_refreshes_lookups_searches_and_totals_in_every_spelling(self):
        spellings = ["Red Widget", "RED  widget", " red\twidget"]
        self.assertEqual(self.prices(spellings), [450] * 3)
        self.assertEqual([p["price"] for p in self.service.search("r")], [450])
        self.assertEqual(self.report.total_price("red"), 450)
        self.assertEqual(self.report.total_price(""), 3050)
        self.service.discount("Re", 20)
        self.assertEqual(self.prices(spellings), [360] * 3)
        self.assertEqual([p["price"] for p in self.service.search("R")], [360])
        self.assertEqual(self.report.total_price("RED "), 360)
        self.assertEqual(self.report.total_price(""), 2960)

    def test_discount_keeps_unrelated_entries_cached(self):
        self.service.lookup("Green Gadget")
        self.service.lookup("Blue Widget")
        self.service.search("gad")
        self.service.search("blue")
        self.report.total_price("green")
        self.report.total_price("zzz")
        self.service.discount("red", 10)
        self.reset_counters()
        self.service.lookup("green  gadget")
        self.service.lookup("blue widget")
        self.service.search("GAD")
        self.service.search("Blue")
        self.report.total_price("GREEN")
        self.report.total_price("zzz")
        self.assertEqual((self.cache.hits, self.cache.misses), (6, 0))
        self.service.lookup("red widget")
        self.assertEqual((self.cache.hits, self.cache.misses), (6, 1))

    def test_discount_with_odd_prefix_spelling_matches_by_normalized_name(self):
        self.service.discount("  STRASSE  l", 50)
        self.assertEqual(self.service.lookup("Straße Lamp")["price"], 600)
        self.assertEqual(self.service.lookup("blue widget")["price"], 500)

    def test_names_module_behavior_is_unchanged(self):
        from catalog.names import display_name, normalize_name
        self.assertEqual(normalize_name("  Blue \t Widget\n"), "blue widget")
        self.assertEqual(normalize_name("STRASSE"), normalize_name("Straße"))
        self.assertEqual(display_name("  Blue \t Widget\n"), "Blue Widget")


if __name__ == "__main__":
    unittest.main(verbosity=2)
