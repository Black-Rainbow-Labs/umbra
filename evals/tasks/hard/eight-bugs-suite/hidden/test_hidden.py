"""Hidden tests. One test class per bug (Bug1 to Bug14), two classes for the invoice function (Part2) and one class of behavior that must not change."""

import datetime
import unittest

from shiplib.customers import dedupe_emails, normalize_email
from shiplib.dates import add_business_days, business_days_between, is_business_day, parse_date
from shiplib.inventory import Inventory, OutOfStock
from shiplib.orders import invoice, order_summary, place_order
from shiplib.parser import parse_order_line, parse_order_lines, parse_price
from shiplib.pricing import add_tax, apply_discount, bulk_unit_price, line_total
from shiplib.report import format_cents, render_report
from shiplib.shipping import base_shipping_cents, free_shipping, shipping_cents
from shiplib.textutil import slugify, truncate, wrap_text


class Bug1DiscountRounding(unittest.TestCase):
    def test_half_cents_round_up(self):
        self.assertEqual(apply_discount(1005, 50), 503)
        self.assertEqual(apply_discount(1, 50), 1)
        self.assertEqual(apply_discount(3, 50), 2)
        self.assertEqual(apply_discount(5, 10), 5)

    def test_other_values(self):
        self.assertEqual(apply_discount(999, 33), 669)
        self.assertEqual(apply_discount(2999, 15), 2549)
        self.assertEqual(apply_discount(0, 50), 0)
        self.assertEqual(apply_discount(100, 0), 100)
        self.assertEqual(apply_discount(100, 100), 0)

    def test_result_is_an_int(self):
        self.assertIsInstance(apply_discount(1005, 50), int)


class Bug2BulkBoundaries(unittest.TestCase):
    def test_ten_units(self):
        self.assertEqual(bulk_unit_price(1000, 10), 900)

    def test_nine_and_eleven_units(self):
        self.assertEqual(bulk_unit_price(1000, 9), 1000)
        self.assertEqual(bulk_unit_price(1000, 11), 900)

    def test_fifty_units(self):
        self.assertEqual(bulk_unit_price(1000, 50), 800)
        self.assertEqual(bulk_unit_price(1000, 49), 900)
        self.assertEqual(bulk_unit_price(1000, 51), 800)

    def test_line_totals(self):
        self.assertEqual(line_total(1000, 10), 9000)
        self.assertEqual(line_total(1000, 50), 40000)
        self.assertEqual(line_total(1000, 1), 1000)


class Bug3BusinessDays(unittest.TestCase):
    def test_friday_plus_one(self):
        self.assertEqual(add_business_days(datetime.date(2024, 3, 1), 1), datetime.date(2024, 3, 4))

    def test_friday_plus_five(self):
        self.assertEqual(add_business_days(datetime.date(2024, 3, 1), 5), datetime.date(2024, 3, 8))

    def test_thursday_plus_two(self):
        self.assertEqual(add_business_days(datetime.date(2024, 2, 29), 2), datetime.date(2024, 3, 4))

    def test_across_a_leap_day(self):
        self.assertEqual(add_business_days(datetime.date(2024, 2, 28), 2), datetime.date(2024, 3, 1))

    def test_ten_business_days_is_two_weeks(self):
        self.assertEqual(add_business_days(datetime.date(2024, 3, 4), 10), datetime.date(2024, 3, 18))

    def test_result_is_never_a_weekend_when_count_is_positive(self):
        start = datetime.date(2024, 3, 1)
        for count in range(1, 30):
            self.assertTrue(is_business_day(add_business_days(start, count)), count)


class Bug4PriceParsing(unittest.TestCase):
    def test_prices_that_floats_get_wrong(self):
        self.assertEqual(parse_price("19.99"), 1999)
        self.assertEqual(parse_price("1.15"), 115)
        self.assertEqual(parse_price("4.35"), 435)
        self.assertEqual(parse_price("0.29"), 29)
        self.assertEqual(parse_price("$8.20"), 820)
        self.assertEqual(parse_price(" 2.30 "), 230)

    def test_simple_prices(self):
        self.assertEqual(parse_price("12"), 1200)
        self.assertEqual(parse_price("5.5"), 550)
        self.assertEqual(parse_price("0.07"), 7)

    def test_in_an_order_line(self):
        self.assertEqual(parse_order_line("SKU-9, 2, $8.20"), ("SKU-9", 2, 820))
        self.assertEqual(parse_order_lines("A, 1, 1.15\nB, 3, 4.35\n"), [("A", 1, 115), ("B", 3, 435)])

    def test_price_is_an_int(self):
        self.assertIsInstance(parse_price("19.99"), int)


class Bug5ReserveExactStock(unittest.TestCase):
    def test_reserve_exactly_what_is_available(self):
        inventory = Inventory({"A": 3})
        inventory.reserve("A", 3)
        self.assertEqual(inventory.available("A"), 0)

    def test_reserve_one_unit_of_one(self):
        inventory = Inventory({"A": 1})
        inventory.reserve("A", 1)
        self.assertEqual(inventory.available("A"), 0)

    def test_more_than_available_still_raises_and_changes_nothing(self):
        inventory = Inventory({"A": 3}, audit=[])
        with self.assertRaises(OutOfStock):
            inventory.reserve("A", 4)
        self.assertEqual(inventory.available("A"), 3)
        self.assertEqual(len(inventory.audit), 0)

    def test_unknown_sku_raises(self):
        with self.assertRaises(OutOfStock):
            Inventory({}).reserve("Z", 1)

    def test_through_an_order(self):
        inventory = Inventory({"A": 12, "B": 1})
        items = place_order("A, 12, 1.00\nB, 1, 2.50\n", inventory)
        self.assertEqual(items, [("A", 1080), ("B", 250)])
        self.assertEqual((inventory.available("A"), inventory.available("B")), (0, 0))


class Bug6AuditList(unittest.TestCase):
    def test_each_inventory_has_its_own_audit(self):
        first = Inventory({"A": 5})
        first.reserve("A", 1)
        second = Inventory({"A": 5})
        self.assertEqual(second.audit, [])
        second.restock("A", 2)
        self.assertEqual(first.audit, [("reserve", "A", 1)])
        self.assertEqual(second.audit, [("restock", "A", 2)])

    def test_a_passed_list_is_the_one_used(self):
        mine = []
        inventory = Inventory({"A": 5}, audit=mine)
        inventory.reserve("A", 2)
        self.assertEqual(mine, [("reserve", "A", 2)])
        self.assertIs(inventory.audit, mine)

    def test_an_empty_passed_list_is_still_used(self):
        mine = []
        inventory = Inventory(audit=mine)
        inventory.restock("A", 1)
        self.assertEqual(len(mine), 1)

    def test_the_default_list_is_not_shared_between_three_inventories(self):
        inventories = [Inventory({"A": 1}) for _ in range(3)]
        inventories[0].restock("A", 1)
        self.assertEqual([len(i.audit) for i in inventories], [1, 0, 0])


class Bug7ReportOrder(unittest.TestCase):
    def test_numeric_order(self):
        text = render_report([("a", 900), ("b", 12000), ("c", 100), ("d", 25)])
        self.assertEqual(text, "b: $120.00\na: $9.00\nc: $1.00\nd: $0.25\nTOTAL: $130.25")

    def test_ties_sort_by_name(self):
        text = render_report([("pear", 500), ("apple", 500), ("fig", 500)])
        self.assertEqual(text, "apple: $5.00\nfig: $5.00\npear: $5.00\nTOTAL: $15.00")

    def test_ties_below_a_larger_total(self):
        text = render_report([("z", 300), ("m", 300), ("a", 1000)])
        self.assertEqual(text.splitlines()[:3], ["a: $10.00", "m: $3.00", "z: $3.00"])

    def test_through_an_order(self):
        inventory = Inventory({"A": 5, "B": 5, "C": 5})
        text = order_summary("A, 1, 9.00\nB, 1, 12.00\nC, 1, 100.00\n", inventory)
        self.assertEqual(text, "C: $100.00\nB: $12.00\nA: $9.00\nTOTAL: $121.00")


class Bug8Wrapping(unittest.TestCase):
    def test_line_of_exactly_width_is_allowed(self):
        self.assertEqual(wrap_text("aaa bbb", 7), ["aaa bbb"])

    def test_line_one_over_width_is_split(self):
        self.assertEqual(wrap_text("aaa bbbb", 7), ["aaa", "bbbb"])

    def test_paragraph(self):
        text = "the quick brown fox jumps over the lazy dog"
        lines = wrap_text(text, 12)
        self.assertEqual(lines, ["the quick", "brown fox", "jumps over", "the lazy dog"])
        self.assertTrue(all(len(line) <= 12 for line in lines))

    def test_every_line_fits_for_many_widths(self):
        text = "alpha beta gamma delta epsilon zeta eta theta iota kappa"
        for width in range(10, 40):
            lines = wrap_text(text, width)
            self.assertTrue(all(len(line) <= width for line in lines), width)
            self.assertEqual(" ".join(lines), text)


class BehaviorThatMustNotChange(unittest.TestCase):
    def test_tax(self):
        self.assertEqual(add_tax(1000, 8), 1080)
        self.assertEqual(add_tax(1050, 5), 1103)
        self.assertEqual(add_tax(0, 8), 0)

    def test_slugify(self):
        self.assertEqual(slugify("  Hello, World!  "), "hello-world")
        self.assertEqual(slugify("A--B  C"), "a-b-c")

    def test_wrap_edge_cases(self):
        self.assertEqual(wrap_text("", 10), [])
        self.assertEqual(wrap_text("a bcdefghij k", 5), ["a", "bcdefghij", "k"])
        self.assertEqual(wrap_text("one   two", 20), ["one two"])

    def test_dates(self):
        self.assertEqual(parse_date("2024-03-01"), datetime.date(2024, 3, 1))
        self.assertFalse(is_business_day(datetime.date(2024, 3, 3)))
        self.assertEqual(add_business_days(datetime.date(2024, 3, 2), 0), datetime.date(2024, 3, 2))
        self.assertEqual(add_business_days(datetime.date(2024, 3, 2), 1), datetime.date(2024, 3, 4))

    def test_parse_skips_comments_and_blanks(self):
        self.assertEqual(parse_order_lines("# c\n\n A , 2 , 3.00 \n"), [("A", 2, 300)])

    def test_format_cents(self):
        self.assertEqual(format_cents(0), "$0.00")
        self.assertEqual(format_cents(100005), "$1000.05")

    def test_out_of_stock_order_changes_nothing(self):
        inventory = Inventory({"A": 10, "B": 1}, audit=[])
        with self.assertRaises(OutOfStock):
            place_order("A, 2, 3.00\nB, 2, 4.00\n", inventory)
        self.assertEqual((inventory.available("A"), inventory.available("B")), (10, 1))
        self.assertEqual(inventory.audit, [])

    def test_order_with_a_bulk_line(self):
        inventory = Inventory({"A": 100})
        items = place_order("A, 20, 10.00\n", inventory)
        self.assertEqual(items, [("A", 18000)])

    def test_restock(self):
        inventory = Inventory()
        inventory.restock("A", 4)
        self.assertEqual(inventory.available("A"), 4)

    def test_shipping_edges(self):
        self.assertEqual(base_shipping_cents(0), 0)
        self.assertEqual(base_shipping_cents(-20), 0)
        self.assertEqual(shipping_cents(500, "local"), 500)
        self.assertEqual(shipping_cents(0, "national"), 0)
        with self.assertRaises(ValueError):
            shipping_cents(500, "moon")

    def test_free_shipping_threshold(self):
        self.assertTrue(free_shipping(7500))
        self.assertTrue(free_shipping(9000))
        self.assertFalse(free_shipping(7499))
        self.assertFalse(free_shipping(0))

    def test_gmail_rule_and_simple_dedupe(self):
        self.assertEqual(normalize_email("  Ann@Example.COM "), "ann@example.com")
        self.assertEqual(normalize_email("A.n.n+shop@gmail.com"), "ann@gmail.com")
        self.assertEqual(dedupe_emails(["Bo@x.com", "cy@x.com", "bo@X.com"]), ["Bo@x.com", "cy@x.com"])
        self.assertEqual(dedupe_emails([]), [])

    def test_empty_order(self):
        inventory = Inventory({"A": 1}, audit=[])
        self.assertEqual(place_order("", inventory), [])
        self.assertEqual(place_order("# nothing\n\n", inventory), [])
        self.assertEqual(inventory.audit, [])

    def test_truncate_of_fitting_text(self):
        self.assertEqual(truncate("hello", 5), "hello")
        self.assertEqual(truncate("hello", 9), "hello")

    def test_business_days_between_midweek(self):
        self.assertEqual(business_days_between(datetime.date(2024, 3, 5), datetime.date(2024, 3, 8)), 3)


class Bug9ShippingWeightSteps(unittest.TestCase):
    def test_up_to_the_base_weight(self):
        for grams in (1, 100, 499, 500):
            self.assertEqual(base_shipping_cents(grams), 500, grams)

    def test_a_started_step_is_charged_in_full(self):
        self.assertEqual(base_shipping_cents(501), 625)
        self.assertEqual(base_shipping_cents(751), 750)
        self.assertEqual(base_shipping_cents(1001), 875)

    def test_whole_steps(self):
        self.assertEqual(base_shipping_cents(750), 625)
        self.assertEqual(base_shipping_cents(1000), 750)
        self.assertEqual(base_shipping_cents(2000), 1250)

    def test_local_zone_uses_the_steps(self):
        self.assertEqual(shipping_cents(501, "local"), 625)
        self.assertEqual(shipping_cents(3000, "local"), 1750)

    def test_result_is_an_int(self):
        self.assertIsInstance(base_shipping_cents(501), int)


class Bug10ShippingZoneRounding(unittest.TestCase):
    def test_half_cents_round_up(self):
        self.assertEqual(shipping_cents(800, "national"), 863)
        self.assertEqual(shipping_cents(3000, "national"), 2013)

    def test_other_national_values(self):
        self.assertEqual(shipping_cents(500, "national"), 575)
        self.assertEqual(shipping_cents(501, "national"), 719)
        self.assertEqual(shipping_cents(1001, "national"), 1006)
        self.assertEqual(shipping_cents(2000, "national"), 1438)

    def test_international_values(self):
        self.assertEqual(shipping_cents(500, "international"), 1500)
        self.assertEqual(shipping_cents(1001, "international"), 2625)
        self.assertEqual(shipping_cents(0, "international"), 0)

    def test_result_is_an_int(self):
        self.assertIsInstance(shipping_cents(800, "national"), int)


class Bug11EmailNormalization(unittest.TestCase):
    def test_gmail_dots_and_tags_are_dropped(self):
        self.assertEqual(normalize_email("A.B+c@GMAIL.com "), "ab@gmail.com")
        self.assertEqual(normalize_email("x.y.z@gmail.com"), "xyz@gmail.com")

    def test_other_domains_keep_dots(self):
        self.assertEqual(normalize_email("john.doe@example.com"), "john.doe@example.com")
        self.assertEqual(normalize_email("x.y@googlemail.com"), "x.y@googlemail.com")

    def test_other_domains_keep_tags(self):
        self.assertEqual(normalize_email("A+B@Example.com"), "a+b@example.com")
        self.assertEqual(normalize_email("sales+2024@mail.gmail.com"), "sales+2024@mail.gmail.com")

    def test_dedupe_uses_the_domain_rule(self):
        addresses = ["a.b@gmail.com", "ab@gmail.com", "a.b@example.com", "ab@example.com", "A.B@EXAMPLE.com"]
        self.assertEqual(dedupe_emails(addresses), ["a.b@gmail.com", "a.b@example.com", "ab@example.com"])


class Bug12DuplicateSkuStock(unittest.TestCase):
    def test_repeated_lines_are_checked_together(self):
        inventory = Inventory({"A": 10, "B": 5}, audit=[])
        with self.assertRaises(OutOfStock):
            place_order("A, 6, 1.00\nB, 1, 1.00\nA, 5, 1.00\n", inventory)
        self.assertEqual((inventory.available("A"), inventory.available("B")), (10, 5))
        self.assertEqual(inventory.audit, [])

    def test_repeated_lines_that_fit_exactly(self):
        inventory = Inventory({"A": 10})
        items = place_order("A, 6, 1.00\nA, 4, 2.00\n", inventory)
        self.assertEqual(items, [("A", 600), ("A", 800)])
        self.assertEqual(inventory.available("A"), 0)

    def test_each_line_is_priced_on_its_own(self):
        inventory = Inventory({"A": 60})
        items = place_order("A, 30, 10.00\nA, 30, 10.00\n", inventory)
        self.assertEqual(items, [("A", 27000), ("A", 27000)])

    def test_sku_names_are_case_sensitive(self):
        inventory = Inventory({"A": 1, "a": 1})
        self.assertEqual(place_order("A, 1, 1.00\na, 1, 1.00\n", inventory), [("A", 100), ("a", 100)])

    def test_unknown_sku_raises(self):
        with self.assertRaises(OutOfStock):
            place_order("Z, 1, 1.00", Inventory({}))


class Bug13Truncate(unittest.TestCase):
    def test_cut_text_is_exactly_width_long(self):
        self.assertEqual(truncate("hello world", 8), "hello...")
        self.assertEqual(truncate("hello world", 10), "hello w...")
        self.assertEqual(truncate("abcdef", 4), "a...")
        self.assertEqual(truncate("abcdef", 3), "...")

    def test_every_width(self):
        text = "the quick brown fox"
        for width in range(3, len(text)):
            self.assertEqual(len(truncate(text, width)), width, width)

    def test_unicode(self):
        self.assertEqual(truncate("héllo wörld", 8), "héllo...")

    def test_text_that_fits_is_unchanged(self):
        self.assertEqual(truncate("hello world", 11), "hello world")
        self.assertEqual(truncate("hello world", 50), "hello world")
        self.assertEqual(truncate("ab", 2), "ab")
        self.assertEqual(truncate("", 0), "")

    def test_too_narrow_to_cut(self):
        with self.assertRaises(ValueError):
            truncate("abc", 2)
        with self.assertRaises(ValueError):
            truncate("abcdef", 0)


class Bug14BusinessDaysBetween(unittest.TestCase):
    def test_across_a_weekend(self):
        self.assertEqual(business_days_between(datetime.date(2024, 3, 1), datetime.date(2024, 3, 4)), 1)
        self.assertEqual(business_days_between(datetime.date(2024, 3, 2), datetime.date(2024, 3, 4)), 1)

    def test_a_weekend_end_adds_nothing(self):
        self.assertEqual(business_days_between(datetime.date(2024, 3, 1), datetime.date(2024, 3, 2)), 0)
        self.assertEqual(business_days_between(datetime.date(2024, 3, 1), datetime.date(2024, 3, 3)), 0)

    def test_whole_weeks(self):
        self.assertEqual(business_days_between(datetime.date(2024, 3, 3), datetime.date(2024, 3, 10)), 5)
        self.assertEqual(business_days_between(datetime.date(2024, 3, 4), datetime.date(2024, 3, 8)), 4)
        self.assertEqual(business_days_between(datetime.date(2024, 3, 4), datetime.date(2024, 3, 18)), 10)

    def test_end_not_after_start(self):
        self.assertEqual(business_days_between(datetime.date(2024, 3, 5), datetime.date(2024, 3, 5)), 0)
        self.assertEqual(business_days_between(datetime.date(2024, 3, 8), datetime.date(2024, 3, 4)), 0)

    def test_matches_add_business_days(self):
        for start in (datetime.date(2024, 3, 1), datetime.date(2024, 3, 2), datetime.date(2024, 3, 6)):
            for count in range(0, 21):
                self.assertEqual(business_days_between(start, add_business_days(start, count)), count, (start, count))


class Part2InvoiceTotals(unittest.TestCase):
    def test_shipping_and_tax_are_added(self):
        inventory = Inventory({"A": 100, "B": 10})
        result = invoice("A, 2, 10.00\nB, 1, 5.00\n", inventory, "local", 500, 8)
        self.assertEqual(result, {"items": [("A", 2000), ("B", 500)], "subtotal": 2500, "shipping": 500, "tax": 240, "total": 3240})
        self.assertEqual(inventory.available("A"), 98)

    def test_free_shipping_at_the_threshold(self):
        result = invoice("A, 3, 25.00", Inventory({"A": 5}), "local", 500, 10)
        self.assertEqual((result["subtotal"], result["shipping"], result["tax"], result["total"]), (7500, 0, 750, 8250))

    def test_shipping_just_below_the_threshold(self):
        result = invoice("A, 1, 74.99", Inventory({"A": 5}), "local", 500, 10)
        self.assertEqual((result["subtotal"], result["shipping"], result["tax"], result["total"]), (7499, 500, 800, 8799))

    def test_national_zone_rounding_and_tax_rounding(self):
        result = invoice("A, 1, 10.00", Inventory({"A": 5}), "national", 800, 5)
        self.assertEqual((result["shipping"], result["tax"], result["total"]), (863, 93, 1956))

    def test_tax_rounds_half_up(self):
        result = invoice("A, 1, 5.50", Inventory({"A": 5}), "local", 500, 5)
        self.assertEqual((result["subtotal"], result["shipping"], result["tax"], result["total"]), (550, 500, 53, 1103))

    def test_international_heavy_parcel_with_no_tax(self):
        result = invoice("A, 1, 10.00", Inventory({"A": 5}), "international", 1001, 0)
        self.assertEqual((result["shipping"], result["tax"], result["total"]), (2625, 0, 3625))

    def test_bulk_line_ships_free(self):
        result = invoice("A, 10, 10.00", Inventory({"A": 10}), "international", 100000, 0)
        self.assertEqual((result["items"], result["subtotal"], result["shipping"], result["total"]), ([("A", 9000)], 9000, 0, 9000))

    def test_keys(self):
        result = invoice("A, 1, 1.00", Inventory({"A": 1}), "local", 1, 0)
        self.assertEqual(set(result), {"items", "subtotal", "shipping", "tax", "total"})
        self.assertTrue(all(isinstance(result[key], int) for key in ("subtotal", "shipping", "tax", "total")))


class Part2InvoiceEdges(unittest.TestCase):
    def test_unknown_zone_reserves_nothing_even_when_free(self):
        inventory = Inventory({"A": 5}, audit=[])
        with self.assertRaises(ValueError):
            invoice("A, 1, 100.00", inventory, "moon", 500, 8)
        self.assertEqual(inventory.available("A"), 5)
        self.assertEqual(inventory.audit, [])

    def test_out_of_stock_reserves_nothing(self):
        inventory = Inventory({"A": 5, "B": 1}, audit=[])
        with self.assertRaises(OutOfStock):
            invoice("A, 2, 1.00\nB, 2, 1.00\n", inventory, "local", 500, 8)
        self.assertEqual((inventory.available("A"), inventory.available("B")), (5, 1))
        self.assertEqual(inventory.audit, [])

    def test_repeated_lines_are_checked_together(self):
        inventory = Inventory({"A": 10}, audit=[])
        with self.assertRaises(OutOfStock):
            invoice("A, 6, 1.00\nA, 6, 1.00\n", inventory, "local", 500, 8)
        self.assertEqual(inventory.available("A"), 10)

    def test_repeated_lines_are_summed(self):
        result = invoice("A, 1, 1.00\nA, 2, 1.00\n", Inventory({"A": 3}), "local", 500, 0)
        self.assertEqual((result["items"], result["subtotal"], result["total"]), ([("A", 100), ("A", 200)], 300, 800))

    def test_comments_and_blank_lines_are_skipped(self):
        result = invoice("# order\n\nA, 1, 2.00\n", Inventory({"A": 1}), "local", 0, 0)
        self.assertEqual((result["subtotal"], result["shipping"], result["total"]), (200, 0, 200))

    def test_two_invoices_use_the_same_stock(self):
        inventory = Inventory({"A": 3})
        invoice("A, 2, 1.00", inventory, "local", 500, 0)
        with self.assertRaises(OutOfStock):
            invoice("A, 2, 1.00", inventory, "local", 500, 0)
        self.assertEqual(inventory.available("A"), 1)
