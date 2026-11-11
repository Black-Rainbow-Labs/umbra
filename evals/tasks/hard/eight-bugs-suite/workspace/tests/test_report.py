import unittest

from shiplib.report import format_cents, render_report


class ReportTests(unittest.TestCase):
    def test_format_cents(self):
        self.assertEqual(format_cents(1234), "$12.34")
        self.assertEqual(format_cents(5), "$0.05")

    def test_single_item_report(self):
        self.assertEqual(render_report([("A", 250)]), "A: $2.50\nTOTAL: $2.50")

    def test_empty_report_has_only_a_total(self):
        self.assertEqual(render_report([]), "TOTAL: $0.00")


if __name__ == "__main__":
    unittest.main()
