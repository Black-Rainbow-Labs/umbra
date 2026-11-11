"""Turns source text into a list of tokens with line and column positions."""
from .tokens import Token
from .rules import is_name_start, read_name, read_number, read_operator, read_string


def advance(text, line, column):
    """Return the line and column reached after reading the text from the given position."""
    for ch in text:
        if ch == "\n":
            line += 1
        else:
            column += 1
    return line, column


def read_token(source, pos, line, column):
    """Read one token at pos, or return None for a character the scanner does not know."""
    ch = source[pos]
    if ch in "\"'":
        return Token("string", read_string(source, pos), line, column)
    if ch.isdigit():
        return Token("number", read_number(source, pos), line, column)
    if is_name_start(ch):
        return Token("name", read_name(source, pos), line, column)
    operator = read_operator(source, pos)
    if operator is not None:
        return Token("op", operator, line, column)
    return None


def tokenize(source):
    """Split source into tokens. Whitespace is skipped. Raises ValueError on a bad character."""
    tokens = []
    pos, line, column = 0, 1, 1
    while pos < len(source):
        ch = source[pos]
        if ch.isspace():
            line, column = advance(ch, line, column)
            pos += 1
            continue
        token = read_token(source, pos, line, column)
        if token is None:
            raise ValueError("unexpected character %r at line %d column %d" % (ch, line, column))
        tokens.append(token)
        line, column = advance(token.text, line, column)
        pos += len(token.text)
    return tokens
