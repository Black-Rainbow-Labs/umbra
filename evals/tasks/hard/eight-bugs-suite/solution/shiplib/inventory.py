"""Stock levels with an audit trail."""


class OutOfStock(Exception):
    """Raised when an order asks for more units than are in stock."""


class Inventory:
    """Units in stock per SKU.

    audit is an optional list that receives one tuple per change. When it is omitted,
    each Inventory gets its own new list. When a list is passed, that same list is used.
    """

    def __init__(self, stock=None, audit=None):
        self.stock = dict(stock or {})
        self.audit = audit if audit is not None else []

    def available(self, sku):
        """Return the units in stock for sku (0 for an unknown SKU)."""
        return self.stock.get(sku, 0)

    def reserve(self, sku, quantity):
        """Take quantity units out of stock and record ('reserve', sku, quantity).

        Raises OutOfStock when fewer than quantity units are available.
        Reserving exactly the units that are available is allowed.
        """
        if quantity > self.available(sku):
            raise OutOfStock(sku)
        self.stock[sku] = self.available(sku) - quantity
        self.audit.append(("reserve", sku, quantity))

    def restock(self, sku, quantity):
        """Add quantity units to stock and record ('restock', sku, quantity)."""
        self.stock[sku] = self.available(sku) + quantity
        self.audit.append(("restock", sku, quantity))
