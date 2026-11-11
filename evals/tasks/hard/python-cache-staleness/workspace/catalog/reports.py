"""Reports built on the catalog service. Results are cached in the service's cache."""

from .keys import total_key


class PriceReport:
    """Totals over the products of a CatalogService."""

    def __init__(self, service):
        self.service = service

    def total_price(self, prefix):
        """Return the sum of the prices (in cents) of the products whose name starts with prefix. Cached."""
        return self.service.cache.get(total_key(prefix), lambda: sum(p["price"] for p in self.service.store.search(prefix)))
