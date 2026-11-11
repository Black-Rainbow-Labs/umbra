"""Character level rules used by the scanner."""

TWO_CHAR_OPERATORS = ("==", "!=", "<=", ">=")
ONE_CHAR_OPERATORS = "+-*/=<>(),"


def is_name_start(ch):
    """True for characters that can begin a name."""
    return ch.isalpha() or ch == "_"


def is_name_part(ch):
    """True for characters that can continue a name."""
    return ch.isalnum() or ch == "_"


def read_name(source, start):
    """Return the name that starts at the given position."""
    end = start
    while end < len(source) and is_name_part(source[end]):
        end += 1
    return source[start:end]


def read_number(source, start):
    """Return the number that starts at the position, with an optional fraction."""
    end = start
    while end < len(source) and source[end].isdigit():
        end += 1
    if end + 1 < len(source) and source[end] == "." and source[end + 1].isdigit():
        end += 1
        while end < len(source) and source[end].isdigit():
            end += 1
    return source[start:end]


def read_operator(source, start):
    """Return the operator at the position, or None when the character is not an operator."""
    if source[start:start + 2] in TWO_CHAR_OPERATORS:
        return source[start:start + 2]
    if source[start] in ONE_CHAR_OPERATORS:
        return source[start]
    return None


def read_string(source, start):
    """Return the quoted string that starts at the position, quotes included.

    A backslash escapes the character after it, so \" does not end the string and \\ is
    a single backslash. Raises ValueError when the closing quote is missing.
    """
    quote = source[start]
    i = start + 1
    while i < len(source):
        ch = source[i]
        if ch == quote and source[i - 1] != "\\":
            return source[start:i + 1]
        i += 1
    raise ValueError("unterminated string starting at offset %d" % start)
