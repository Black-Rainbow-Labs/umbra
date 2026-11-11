import unittest

from tokenizer import tokenize, Token


def summary(source):
    return [(t.kind, t.text, t.line, t.column) for t in tokenize(source)]


class ScannerTests(unittest.TestCase):
    def test_simple_expression(self):
        self.assertEqual(summary("x = 12 + y"), [
            ("name", "x", 1, 1), ("op", "=", 1, 3), ("number", "12", 1, 5),
            ("op", "+", 1, 8), ("name", "y", 1, 10),
        ])

    def test_two_character_operators(self):
        self.assertEqual([t.text for t in tokenize("a <= b != c")], ["a", "<=", "b", "!=", "c"])

    def test_decimal_number(self):
        self.assertEqual(summary("3.25"), [("number", "3.25", 1, 1)])

    def test_position_on_second_line(self):
        self.assertEqual(summary("a\nb"), [("name", "a", 1, 1), ("name", "b", 2, 1)])

    def test_string_with_escaped_quote(self):
        self.assertEqual(summary('say("a \\"b\\" c")')[2], ("string", '"a \\"b\\" c"', 1, 5))

    def test_string_ending_in_escaped_backslash(self):
        self.assertEqual(summary('"C:\\\\" + x')[0], ("string", '"C:\\\\"', 1, 1))

    def test_unterminated_string(self):
        with self.assertRaises(ValueError):
            tokenize('"abc')


if __name__ == "__main__":
    unittest.main()
