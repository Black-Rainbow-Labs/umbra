import unittest

from shiplib.customers import dedupe_emails, normalize_email


class CustomersTests(unittest.TestCase):
    def test_trim_and_lowercase(self):
        self.assertEqual(normalize_email("  Ann@Example.COM "), "ann@example.com")

    def test_gmail_ignores_dots_and_tags(self):
        self.assertEqual(normalize_email("A.n.n+shop@gmail.com"), "ann@gmail.com")

    def test_other_domains_keep_dots_and_tags(self):
        self.assertEqual(normalize_email("Ann.Lee+news@example.com"), "ann.lee+news@example.com")

    def test_dedupe_keeps_first_written_form(self):
        self.assertEqual(dedupe_emails(["Bo@x.com", "cy@x.com", "bo@X.com"]), ["Bo@x.com", "cy@x.com"])


if __name__ == "__main__":
    unittest.main()
