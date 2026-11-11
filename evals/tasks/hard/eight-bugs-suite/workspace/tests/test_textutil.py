import unittest

from shiplib.textutil import slugify, wrap_text


class TextUtilTests(unittest.TestCase):
    def test_slugify(self):
        self.assertEqual(slugify("  Hello, World!  "), "hello-world")

    def test_wrap_short_text_is_one_line(self):
        self.assertEqual(wrap_text("one two", 20), ["one two"])

    def test_wrap_breaks_between_words(self):
        self.assertEqual(wrap_text("aaa bbb ccc ddd", 7), ["aaa bbb", "ccc ddd"])

    def test_wrap_does_not_exceed_width(self):
        self.assertEqual(wrap_text("aaa bbbb", 7), ["aaa", "bbbb"])

    def test_wrap_long_word_gets_its_own_line(self):
        self.assertEqual(wrap_text("a bcdefghij k", 5), ["a", "bcdefghij", "k"])


if __name__ == "__main__":
    unittest.main()
