import os
import sys
import unittest

sys.path.insert(0, os.environ["WORKSPACE_DIR"])

from tokenizer import tokenize


def summary(source):
    return [(t.kind, t.text, t.line, t.column) for t in tokenize(source)]


class PositionTests(unittest.TestCase):
    def test_column_restarts_after_each_newline(self):
        self.assertEqual(summary("a = 1\nbb = 22\n  c"), [
            ("name", "a", 1, 1), ("op", "=", 1, 3), ("number", "1", 1, 5),
            ("name", "bb", 2, 1), ("op", "=", 2, 4), ("number", "22", 2, 6),
            ("name", "c", 3, 3),
        ])

    def test_blank_lines(self):
        self.assertEqual(summary("a\n\n\nb"), [("name", "a", 1, 1), ("name", "b", 4, 1)])

    def test_crlf_line_endings(self):
        self.assertEqual(summary("a\r\nb"), [("name", "a", 1, 1), ("name", "b", 2, 1)])

    def test_token_after_multiline_string(self):
        got = summary('"a\nb" + x')
        self.assertEqual(got, [("string", '"a\nb"', 1, 1), ("op", "+", 2, 4), ("name", "x", 2, 6)])


class StringTests(unittest.TestCase):
    def test_escaped_backslash_before_closing_quote(self):
        self.assertEqual(summary('"C:\\\\" + x'), [
            ("string", '"C:\\\\"', 1, 1), ("op", "+", 1, 8), ("name", "x", 1, 10),
        ])

    def test_escaped_quote_inside(self):
        got = summary('f("a \\"b\\" c", 2)')
        self.assertEqual([t[1] for t in got], ["f", "(", '"a \\"b\\" c"', ",", "2", ")"])

    def test_escaped_backslash_then_escaped_quote(self):
        got = summary('"x\\\\\\"y" z')
        self.assertEqual([t[1] for t in got], ['"x\\\\\\"y"', "z"])

    def test_single_quotes(self):
        self.assertEqual([t[1] for t in summary("'it\\'s' + 'ok'")], ["'it\\'s'", "+", "'ok'"])

    def test_other_quote_inside_string(self):
        self.assertEqual([t[1] for t in summary('"it\'s" x')], ['"it\'s"', "x"])

    def test_unterminated_string_still_raises(self):
        with self.assertRaises(ValueError):
            tokenize('"abc\\"')
        with self.assertRaises(ValueError):
            tokenize('x = "abc')

    def test_bad_character_raises(self):
        with self.assertRaises(ValueError):
            tokenize("a $ b")


class BasicTests(unittest.TestCase):
    def test_numbers_and_operators(self):
        self.assertEqual([t[1] for t in summary("1.5*2>=3 == x")], ["1.5", "*", "2", ">=", "3", "==", "x"])

    def test_empty_source(self):
        self.assertEqual(tokenize(""), [])


if __name__ == "__main__":
    unittest.main()
