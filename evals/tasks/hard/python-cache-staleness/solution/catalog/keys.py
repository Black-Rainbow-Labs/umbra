"""Cache keys used by the catalog service."""

from .names import normalize_name


def lookup_key(name):
    """Return the cache key for looking up one product by name."""
    return ("lookup", normalize_name(name))


def search_key(prefix):
    """Return the cache key for a prefix search."""
    return ("search", normalize_name(prefix))


def total_key(prefix):
    """Return the cache key for the total price of the products under a prefix."""
    return ("total", normalize_name(prefix))
