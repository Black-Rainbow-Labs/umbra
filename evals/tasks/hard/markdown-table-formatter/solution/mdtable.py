"""Formats markdown tables so that every column lines up."""
import re
import unicodedata

DELIMITER_CELL = re.compile(r"^:?-+:?$")
MIN_WIDTH = 3


def scan_row(line):
    """Splits a row at unescaped pipes. Returns (raw cells, starts with separator, ends with separator)."""
    text = line.strip()
    cells = []
    current = []
    backslashes = 0
    last_was_separator = False
    for char in text:
        last_was_separator = False
        if char == "|" and backslashes % 2 == 0:
            cells.append("".join(current))
            current = []
            last_was_separator = True
        else:
            current.append(char)
        backslashes = backslashes + 1 if char == "\\" else 0
    cells.append("".join(current))
    return cells, text.startswith("|"), last_was_separator


def split_row(line):
    """Returns the trimmed cells of a row, without the optional outer pipes."""
    cells, leading, trailing = scan_row(line)
    if leading:
        cells = cells[1:]
    if trailing and cells:
        cells = cells[:-1]
    return [cell.strip() for cell in cells]


def has_unescaped_pipe(line):
    """Tells whether a line contains a pipe that is not escaped."""
    return len(scan_row(line)[0]) > 1


def char_width(char):
    """Returns the display width of one character: 0 for combining and format characters, 2 for wide ones."""
    if unicodedata.combining(char) or unicodedata.category(char) in ("Mn", "Me", "Cf"):
        return 0
    return 2 if unicodedata.east_asian_width(char) in ("W", "F") else 1


def display_width(text):
    """Returns the display width of a text."""
    return sum(char_width(char) for char in text)


def parse_alignments(cells):
    """Turns delimiter cells into alignments 'left', 'right', 'center' or 'none'. Raises ValueError for a bad cell."""
    result = []
    for cell in cells:
        if not DELIMITER_CELL.match(cell):
            raise ValueError("not a table: bad delimiter row")
        left, right = cell.startswith(":"), cell.endswith(":")
        result.append("center" if left and right and len(cell) > 1 else "left" if left else "right" if right else "none")
    if not result:
        raise ValueError("not a table: bad delimiter row")
    return result


def pad_cell(text, width, align):
    """Pads a cell with spaces to a display width. Centered text gets the extra space on the right."""
    extra = width - display_width(text)
    if align == "right":
        return " " * extra + text
    if align == "center":
        return " " * (extra // 2) + text + " " * (extra - extra // 2)
    return text + " " * extra


def delimiter_cell(width, align):
    """Builds a delimiter cell of the given width."""
    if align == "left":
        return ":" + "-" * (width - 1)
    if align == "right":
        return "-" * (width - 1) + ":"
    if align == "center":
        return ":" + "-" * (width - 2) + ":"
    return "-" * width


def fit_row(cells, count):
    """Pads a row with empty cells or drops extra cells so it has exactly count cells."""
    return (cells + [""] * count)[:count]


def format_table(text):
    """Formats one table given as text (header, delimiter row, body rows). Raises ValueError if it is not a table."""
    lines = [line for line in re.split(r"\r\n|\n|\r", text) if line.strip()]
    if len(lines) < 2:
        raise ValueError("not a table: needs a header and a delimiter row")
    header = split_row(lines[0])
    count = len(header)
    aligns = (parse_alignments(split_row(lines[1])) + ["none"] * count)[:count]
    rows = [header] + [fit_row(split_row(line), count) for line in lines[2:]]
    widths = [max([MIN_WIDTH] + [display_width(row[i]) for row in rows]) for i in range(count)]
    render = lambda row: "| " + " | ".join(pad_cell(row[i], widths[i], aligns[i]) for i in range(count)) + " |"
    delimiter = "| " + " | ".join(delimiter_cell(widths[i], aligns[i]) for i in range(count)) + " |"
    return "\n".join([render(rows[0]), delimiter] + [render(row) for row in rows[1:]])


def is_delimiter_line(line):
    """Tells whether a line is a valid delimiter row that contains at least one pipe."""
    if not has_unescaped_pipe(line):
        return False
    try:
        parse_alignments(split_row(line))
    except ValueError:
        return False
    return True


def fence_marker(line):
    """Returns the fence marker (three or more backticks or tildes) that starts a line, or ''."""
    match = re.match(r"^ {0,3}(`{3,}|~{3,})", line)
    return match.group(1) if match else ""


def table_length(lines, start):
    """Returns how many lines the table starting at lines[start] has, or 0 when none starts there."""
    if start + 1 >= len(lines) or lines[start].startswith(("    ", "\t")):
        return 0
    if not has_unescaped_pipe(lines[start]) or not is_delimiter_line(lines[start + 1]):
        return 0
    end = start + 2
    while end < len(lines) and lines[end].strip() and has_unescaped_pipe(lines[end]):
        end += 1
    return end - start


def format_document(text):
    """Formats every table in a markdown document. Text outside tables and tables inside code fences stay as they are."""
    eol = "\r\n" if "\r\n" in text else "\n"
    lines = text.split(eol)
    out = []
    fence = ""
    index = 0
    while index < len(lines):
        line = lines[index]
        marker = fence_marker(line)
        if fence:
            if marker and marker[0] == fence[0] and len(marker) >= len(fence) and not line.strip()[len(marker):].strip():
                fence = ""
            out.append(line)
            index += 1
            continue
        if marker:
            fence = marker
            out.append(line)
            index += 1
            continue
        length = table_length(lines, index)
        if length:
            out.extend(format_table(eol.join(lines[index:index + length])).split("\n"))
            index += length
        else:
            out.append(line)
            index += 1
    return eol.join(out)
