import random
import re
import unicodedata
import unittest

from mdtable import format_document, format_table


def width(text):
    # Independent copy of the stated width rule.
    total = 0
    for char in text:
        if unicodedata.combining(char) or unicodedata.category(char) in ("Mn", "Me", "Cf"):
            continue
        total += 2 if unicodedata.east_asian_width(char) in ("W", "F") else 1
    return total


def lines_of(*rows):
    return "\n".join(rows)


class Rules(unittest.TestCase):
    def test_R1_split_cells_with_optional_outer_pipes(self):
        want = lines_of("| a   | b   |", "| --- | --- |", "| 1   | 2   |")
        self.assertEqual(format_table("| a | b |\n|---|---|\n| 1 | 2 |"), want)
        self.assertEqual(format_table("a | b\n---|---\n1 | 2"), want)
        self.assertEqual(format_table("| a | b\n--|--\n1 | 2 |"), want)
        self.assertEqual(format_table("  a  |  b \n--- | ---\n 1|2"), want)

    def test_R1_escaped_pipes_stay_inside_the_cell_and_count_two(self):
        got = format_table(r"a | b" + "\n" + r"---|---" + "\n" + r"x \| y | z")
        self.assertEqual(got, lines_of("| a      | b   |", "| ------ | --- |", r"| x \| y | z   |"))

    def test_R1_pipe_after_an_even_number_of_backslashes_splits(self):
        got = format_table("a | b\n---|---\n" + r"x \\| y")
        self.assertEqual(got, lines_of("| a    | b   |", "| ---- | --- |", "| x \\\\ | y   |"))
        got = format_table("a | b\n---|---\n" + r"x \\\| y | z")
        self.assertEqual(got, lines_of("| a        | b   |", "| -------- | --- |", "| x \\\\\\| y | z   |"))

    def test_R2_delimiter_row_sets_the_alignment_and_is_written_in_full_width(self):
        got = format_table("l | r | c | n\n:-- | --: | :-: | ---\n1|2|3|4")
        self.assertEqual(got, lines_of("| l   |   r |  c  | n   |", "| :-- | --: | :-: | --- |", "| 1   |   2 |  3  | 4   |"))
        got = format_table("a | b\n:---------: | ----------\nlonger text | x")
        self.assertEqual(got, lines_of("|      a      | b   |", "| :---------: | --- |", "| longer text | x   |"))

    def test_R2_the_delimiter_width_follows_the_content_with_a_minimum_of_three(self):
        got = format_table("a|b\n:-|-:\n1|2")
        self.assertEqual(got, lines_of("| a   |   b |", "| :-- | --: |", "| 1   |   2 |"))
        got = format_table("a|b\n-|-\n1|2")
        self.assertEqual(got, lines_of("| a   | b   |", "| --- | --- |", "| 1   | 2   |"))

    def test_R3_column_width_is_the_widest_cell_of_header_and_body(self):
        got = format_table("Name | Qty\n--- | ---\nApple | 3\nWatermelon | 12")
        self.assertEqual(got, lines_of("| Name       | Qty |", "| ---------- | --- |", "| Apple      | 3   |", "| Watermelon | 12  |"))
        got = format_table("A very long header | b\n-|-\nx | y")
        self.assertEqual(got.split("\n")[0], "| A very long header | b   |")

    def test_R4_wide_characters_count_two_and_combining_marks_count_zero(self):
        got = format_table("名前 | x\n--- | ---\nab | 日本語テキスト")
        self.assertEqual(got, lines_of("| 名前 | x              |", "| ---- | -------------- |", "| ab   | 日本語テキスト |"))
        got = format_table("é | b\n--- | ---\nxyz | y")
        self.assertEqual(got, lines_of("| é   | b   |", "| --- | --- |", "| xyz | y   |"))
        got = format_table("한글 | v\n--|--\nabcd | 1")
        self.assertEqual(got, lines_of("| 한글 | v   |", "| ---- | --- |", "| abcd | 1   |"))
        got = format_table("a\U0001F600 | b\n--|--\nxyz | y")
        self.assertEqual(got, lines_of("| a\U0001F600 | b   |", "| --- | --- |", "| xyz | y   |"))
        got = format_table("a​b | b\n--|--\nxyz | y")
        self.assertEqual(got, lines_of("| a​b  | b   |", "| --- | --- |", "| xyz | y   |"))

    def test_R5_cells_are_aligned_like_their_column_in_header_and_body(self):
        got = format_table("n\n--:\n5\n123")
        self.assertEqual(got, lines_of("|   n |", "| --: |", "|   5 |", "| 123 |"))
        got = format_table("h\n:-:\nabc\nxxxxxx")
        self.assertEqual(got, lines_of("| " + "  h   " + " |", "| " + ":----:" + " |", "| " + " abc  " + " |", "| " + "xxxxxx" + " |"))
        got = format_table("h\n:--\nabc\nxxxxxx")
        self.assertEqual(got.split("\n")[0], "| h      |")

    def test_R6_ragged_rows_are_padded_or_cut_to_the_header_length(self):
        got = format_table("a | b | c\n--- | ---\n1\n1 | 2 | 3 | 4 | 5")
        self.assertEqual(got, lines_of("| a   | b   | c   |", "| --- | --- | --- |", "| 1   |     |     |", "| 1   | 2   | 3   |"))
        got = format_table("a | b\n--- | --- | ---\n1 | 2")
        self.assertEqual(got, lines_of("| a   | b   |", "| --- | --- |", "| 1   | 2   |"))
        got = format_table("a | b | c\n:-: | --:\n1|2|3")
        self.assertEqual(got, lines_of("|  a  |   b | c   |", "| :-: | --: | --- |", "|  1  |   2 | 3   |"))

    def test_R7_empty_cells_stay_empty_and_are_padded_with_spaces(self):
        got = format_table("a | b\n--- | ---\n| | x |\ny |\n|  |  |")
        self.assertEqual(got, lines_of("| a   | b   |", "| --- | --- |", "|     | x   |", "| y   |     |", "|     |     |"))
        got = format_table("| | b |\n|---|---|\n| 1 | 2 |")
        self.assertEqual(got, lines_of("|     | b   |", "| --- | --- |", "| 1   | 2   |"))

    def test_R8_input_that_is_not_a_table_raises_ValueError(self):
        with self.assertRaisesRegex(ValueError, r"^not a table: needs a header and a delimiter row$"):
            format_table("a | b")
        with self.assertRaisesRegex(ValueError, r"^not a table: needs a header and a delimiter row$"):
            format_table("")
        for text in ("a | b\nc | d", "a|b\n--|x-\n1|2", "a|b\n:|--", "a|b\n---|- -", "a|b\n|\n1|2"):
            with self.assertRaisesRegex(ValueError, r"^not a table: bad delimiter row$"):
                format_table(text)
        self.assertEqual(format_table("\n\na | b\n--|--\n1 | 2\n\n"), lines_of("| a   | b   |", "| --- | --- |", "| 1   | 2   |"))

    def test_R9_formatting_twice_changes_nothing_and_every_line_has_the_same_width(self):
        rng = random.Random(7)
        cells = ["a", "bb", "ccc", "名前", "é", "x\\|y", "", "\U0001F600", "a  b", "\\", "long cell text", "日本語"]
        aligns = [":--", "--:", ":-:", "---", "-----", ":----:"]
        for _ in range(300):
            count = rng.randint(1, 5)
            def make_row(pool, size):
                return "| " + " | ".join(rng.choice(pool) for _ in range(size)) + " |"

            rows = [make_row(cells, count), make_row(aligns, rng.randint(1, 6))]
            for _ in range(rng.randint(0, 4)):
                rows.append(make_row(cells, rng.randint(1, 7)))
            text = "\n".join(rows)
            once = format_table(text)
            self.assertEqual(format_table(once), once, text)
            out = once.split("\n")
            self.assertEqual(len(out), len(rows), text)
            self.assertEqual({width(line) for line in out}, {width(out[0])}, once)
            self.assertTrue(all(line.startswith("| ") and line.endswith(" |") for line in out), once)
            document = "intro\n\n" + once + "\n\noutro\n"
            self.assertEqual(format_document(document), document)

    def test_R10_format_document_formats_only_real_tables_outside_code(self):
        doc = (
            "# Title\n\nSome text | with a pipe\n\n| a | b |\n|-|-|\n| 1 | 2 |\n\n"
            "```\n| x | y |\n|-|-|\n| 1 | 2 |\n```\n\n~~~md\n| p | q |\n|-|-|\n~~~\n\n"
            "    | c | d |\n    |-|-|\n\nHeading\n---\n\nend\n"
        )
        want = doc.replace("| a | b |\n|-|-|\n| 1 | 2 |", "| a   | b   |\n| --- | --- |\n| 1   | 2   |")
        self.assertEqual(format_document(doc), want)

    def test_R10_a_table_ends_at_a_blank_line_or_a_line_without_a_pipe(self):
        doc = "| a | b |\n|-|-|\n| 1 | 2 |\nnot a row\n\nx | y\n-|-\n3 | 4\n\n"
        want = "| a   | b   |\n| --- | --- |\n| 1   | 2   |\nnot a row\n\n| x   | y   |\n| --- | --- |\n| 3   | 4   |\n\n"
        self.assertEqual(format_document(doc), want)

    def test_R10_line_endings_final_newline_and_indentation(self):
        doc = "a\r\n\r\n  | a | b |\r\n  |-|-|\r\n  | 1 | 2 |\r\n\r\nz"
        out = format_document(doc)
        self.assertEqual(out, "a\r\n\r\n| a   | b   |\r\n| --- | --- |\r\n| 1   | 2   |\r\n\r\nz")
        self.assertEqual(format_document("| a | b |\n|-|-|\n| 1 | 2 |"), "| a   | b   |\n| --- | --- |\n| 1   | 2   |")
        self.assertEqual(format_document(""), "")

    def test_R10_fences_close_only_with_a_matching_marker_and_bad_tables_are_left_alone(self):
        doc = "```\n~~~\n| a | b |\n|-|-|\n```\n| a | b |\n|-|-|\n"
        self.assertEqual(format_document(doc), "```\n~~~\n| a | b |\n|-|-|\n```\n| a   | b   |\n| --- | --- |\n")
        bad = "a | b\n--- | x\n1 | 2\n"
        self.assertEqual(format_document(bad), bad)
        longer = "````\n```\n| a | b |\n|-|-|\n````\n"
        self.assertEqual(format_document(longer), longer)

    def test_R11_output_has_no_trailing_spaces_or_newline_and_keeps_inner_spacing(self):
        got = format_table("  a  b  |  c \r\n --- | --- \r\n  x   y | z  \r\n")
        self.assertEqual(got, lines_of("| a  b  | c   |", "| ----- | --- |", "| x   y | z   |"))
        self.assertFalse(got.endswith("\n"))
        self.assertTrue(all(line == line.rstrip() for line in got.split("\n")))
        self.assertNotIn("\r", got)


if __name__ == "__main__":
    unittest.main()
