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
