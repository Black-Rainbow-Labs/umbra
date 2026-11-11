"""Small text helpers."""

import re


def slugify(text):
    """Lowercase text, replace each run of characters other than letters and digits with one hyphen,
    and strip hyphens from both ends."""
    return re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")


def wrap_text(text, width):
    """Split text into lines of at most width characters, breaking only between words.

    Words are separated by single spaces in the output. A word longer than width gets a line to itself.
    Empty text gives an empty list.
    """
    lines = []
    current = ""
    for word in text.split():
        if current and len(current) + len(word) > width:
            lines.append(current)
            current = word
        elif current:
            current += " " + word
        else:
            current = word
    if current:
        lines.append(current)
    return lines


def truncate(text, width):
    """Return text cut to at most width characters.

    Text that already fits in width characters is returned unchanged. Otherwise the result is the start of
    the text followed by '...' and is exactly width characters long. Cutting needs a width of at least 3,
    so a smaller width raises ValueError when the text does not fit.
    """
    if len(text) <= width:
        return text
    if width < 3:
        raise ValueError("width must be at least 3")
    return text[:width] + "..."
