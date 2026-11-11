"""The product store: the source of truth that the cache sits in front of."""

from .names import display_name, normalize_name


class ProductStore:
    """Holds products as {"id", "name", "price"} dicts, with prices in cents."""

    def __init__(self):
        self._products = {}
        self._next_id = 1

    def _find_id(self, name):
        wanted = normalize_name(name)
        for product_id, product in self._products.items():
            if normalize_name(product["name"]) == wanted:
                return product_id
        return None

    def add(self, name, price):
        """Add a product and return its id. Raises ValueError when the name already exists."""
        if self._find_id(name) is not None:
            raise ValueError(f"duplicate product: {name}")
        product_id = self._next_id
        self._next_id += 1
        self._products[product_id] = {"id": product_id, "name": display_name(name), "price": price}
        return product_id

    def find(self, name):
        """Return a copy of the product with this name, or None."""
        product_id = self._find_id(name)
        return dict(self._products[product_id]) if product_id is not None else None

    def set_price(self, name, price):
        """Set a product's price. Raises KeyError when the product does not exist."""
        product_id = self._find_id(name)
        if product_id is None:
            raise KeyError(name)
        self._products[product_id]["price"] = price

    def rename(self, old_name, new_name):
        """Rename a product. Raises KeyError when it is missing and ValueError when the new name is taken."""
        product_id = self._find_id(old_name)
        if product_id is None:
            raise KeyError(old_name)
        existing = self._find_id(new_name)
        if existing is not None and existing != product_id:
            raise ValueError(f"duplicate product: {new_name}")
        self._products[product_id]["name"] = display_name(new_name)

    def remove(self, name):
        """Delete a product. Raises KeyError when it does not exist."""
        product_id = self._find_id(name)
        if product_id is None:
            raise KeyError(name)
        del self._products[product_id]

    def search(self, prefix):
        """Return copies of the products whose name starts with prefix, ordered by normalized name."""
        wanted = normalize_name(prefix)
        found = [p for p in self._products.values() if normalize_name(p["name"]).startswith(wanted)]
        return [dict(p) for p in sorted(found, key=lambda p: normalize_name(p["name"]))]
