import unittest

from mdtable import format_table


class BasicTests(unittest.TestCase):
    def test_simple_table(self):
        text = "a | bb\n--- | ---\n1 | 2"
        self.assertEqual(format_table(text), "| a   | bb  |\n| --- | --- |\n| 1   | 2   |")


if __name__ == "__main__":
    unittest.main()
