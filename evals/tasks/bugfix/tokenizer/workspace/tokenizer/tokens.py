"""The token type produced by the scanner."""
from dataclasses import dataclass


@dataclass(frozen=True)
class Token:
    """One piece of source text. line and column both start at 1."""

    kind: str
    text: str
    line: int
    column: int
