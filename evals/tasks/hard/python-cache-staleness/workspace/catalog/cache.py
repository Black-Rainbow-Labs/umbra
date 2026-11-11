"""A small in memory cache that remembers loader results, including None."""


class LookupCache:
    """Caches values by key and counts hits and misses."""

    def __init__(self):
        self._data = {}
        self.hits = 0
        self.misses = 0

    def get(self, key, loader):
        """Return the cached value for key, calling loader() and storing its result on a miss."""
        if key in self._data:
            self.hits += 1
            return self._data[key]
        self.misses += 1
        value = loader()
        self._data[key] = value
        return value

    def invalidate(self, key):
        """Drop one entry if it exists."""
        self._data.pop(key, None)

    def invalidate_where(self, predicate):
        """Drop every entry whose key satisfies predicate(key)."""
        for key in [key for key in self._data if predicate(key)]:
            del self._data[key]

    def clear(self):
        """Drop every entry."""
        self._data.clear()

    def keys(self):
        """Return the keys currently cached."""
        return list(self._data)
