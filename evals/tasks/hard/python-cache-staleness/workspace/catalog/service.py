"""The catalog service: reads go through the cache, writes go to the store and invalidate."""

from .keys import lookup_key, search_key
from .names import normalize_name


class CatalogService:
    """Product lookups and searches with caching in front of a ProductStore."""

    def __init__(self, store, cache):
        self.store = store
        self.cache = cache

    def lookup(self, name):
        """Return the product with this name (a dict) or None. Misses are cached too."""
        return self.cache.get(lookup_key(name), lambda: self.store.find(name))

    def search(self, prefix):
        """Return the products whose name starts with prefix, ordered by normalized name."""
        return self.cache.get(search_key(prefix), lambda: self.store.search(prefix))

    def _invalidate(self, name):
        """Drop the cached lookup for name and every cached search whose prefix matches it."""
        normalized = normalize_name(name)
        self.cache.invalidate(lookup_key(name))
        self.cache.invalidate_where(lambda key: key[0] == "search" and normalized.startswith(key[1]))

    def add(self, name, price):
        """Add a product and return its id."""
        product_id = self.store.add(name, price)
        self._invalidate(name)
        return product_id

    def update_price(self, name, price):
        """Change one product's price."""
        self.store.set_price(name, price)
        self._invalidate(name)

    def rename(self, old_name, new_name):
        """Rename a product."""
        self.store.rename(old_name, new_name)
        self._invalidate(old_name)
        self._invalidate(new_name)

    def remove(self, name):
        """Delete a product."""
        self.store.remove(name)
        self._invalidate(name)

    def bulk_update_prices(self, updates):
        """Apply a mapping of product name to new price, in order. Returns how many were applied."""
        for name, price in updates.items():
            self.store.set_price(name, price)
        return len(updates)

    def import_rows(self, rows):
        """Add products from (name, price) pairs and return their ids."""
        return [self.add(name, price) for name, price in rows]
