"""Cache keys used by the catalog service."""


def lookup_key(name):
    """Return the cache key for looking up one product by name."""
    return ("lookup", name.strip().lower())


def search_key(prefix):
    """Return the cache key for a prefix search."""
    return ("search", prefix.strip().lower())


def total_key(prefix):
    """Return the cache key for the total price of the products under a prefix."""
    return ("total", prefix.strip().lower())
