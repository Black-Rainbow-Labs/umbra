import datetime
import unittest

from shiplib.dates import add_business_days, is_business_day, parse_date


class DatesTests(unittest.TestCase):
    def test_parse_date(self):
        self.assertEqual(parse_date("2024-03-01"), datetime.date(2024, 3, 1))

    def test_weekend_is_not_a_business_day(self):
        self.assertFalse(is_business_day(datetime.date(2024, 3, 2)))
        self.assertFalse(is_business_day(datetime.date(2024, 3, 3)))
        self.assertTrue(is_business_day(datetime.date(2024, 3, 4)))

    def test_zero_days_returns_start(self):
        self.assertEqual(add_business_days(datetime.date(2024, 3, 2), 0), datetime.date(2024, 3, 2))

    def test_midweek_addition(self):
        self.assertEqual(add_business_days(datetime.date(2024, 3, 5), 2), datetime.date(2024, 3, 7))

    def test_friday_plus_one_is_monday(self):
        self.assertEqual(add_business_days(datetime.date(2024, 3, 1), 1), datetime.date(2024, 3, 4))


if __name__ == "__main__":
    unittest.main()
