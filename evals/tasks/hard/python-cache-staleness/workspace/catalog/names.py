"""Product name handling shared by the store."""


def normalize_name(name):
    """Return the canonical form of a product name.

    Names are compared ignoring case (full Unicode case folding) and ignoring whitespace
    differences: leading and trailing whitespace is dropped and every run of whitespace
    inside the name counts as one space.
    """
    return " ".join(name.split()).casefold()


def display_name(name):
    """Return a name as it is shown: whitespace collapsed, case kept."""
    return " ".join(name.split())
