import unittest

from shiplib.parser import parse_order_line, parse_order_lines, parse_price


class ParserTests(unittest.TestCase):
    def test_whole_dollar_price(self):
        self.assertEqual(parse_price("12"), 1200)

    def test_price_with_dollar_sign(self):
        self.assertEqual(parse_price("$5.50"), 550)

    def test_price_with_cents_is_exact(self):
        self.assertEqual(parse_price("19.99"), 1999)

    def test_order_line(self):
        self.assertEqual(parse_order_line("SKU-1, 3, 4.00"), ("SKU-1", 3, 400))

    def test_blank_and_comment_lines_are_skipped(self):
        text = "# order 7\n\nSKU-1, 1, 2.00\n  \nSKU-2, 2, 3.00\n"
        self.assertEqual(parse_order_lines(text), [("SKU-1", 1, 200), ("SKU-2", 2, 300)])


if __name__ == "__main__":
    unittest.main()
