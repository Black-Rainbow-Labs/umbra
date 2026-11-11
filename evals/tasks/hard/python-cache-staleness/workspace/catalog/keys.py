"""Cache keys used by the catalog service."""


def lookup_key(name):
    """Return the cache key for looking up one product by name."""
    return ("lookup", name.strip().lower())


def search_key(prefix):
    """Return the cache key for a prefix search."""
    return ("search", prefix.strip().lower())
