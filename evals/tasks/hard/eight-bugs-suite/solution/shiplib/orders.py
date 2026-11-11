"""Placing orders: parse, check stock, reserve, price and report."""

from collections import Counter

from . import parser, pricing, report, shipping
from .inventory import OutOfStock


def place_order(text, inventory):
    """Reserve stock for every line of an order and return [(sku, line_total_cents), ...] in order.

    Lines for the same SKU are added together when checking stock, and each line is still priced on its own.
    When the lines ask for more than is in stock, OutOfStock is raised and nothing is reserved.
    """
    lines = parser.parse_order_lines(text)
    wanted = Counter()
    for sku, quantity, _ in lines:
        wanted[sku] += quantity
    for sku, quantity in wanted.items():
        if inventory.available(sku) < quantity:
            raise OutOfStock(sku)
    items = []
    for sku, quantity, unit_cents in lines:
        inventory.reserve(sku, quantity)
        items.append((sku, pricing.line_total(unit_cents, quantity)))
    return items


def order_summary(text, inventory):
    """Place an order and return its text report."""
    return report.render_report(place_order(text, inventory))


def invoice(text, inventory, zone, weight_grams, tax_percent):
    """Place an order and return its totals as a dict of integer cents.

    The keys are items (the list place_order returns), subtotal (the sum of the line totals), shipping,
    tax and total. shipping is 0 when the subtotal ships free (shipping.free_shipping), otherwise
    shipping.shipping_cents(weight_grams, zone). tax is pricing.add_tax(subtotal + shipping, tax_percent)
    minus subtotal + shipping, so shipping is taxed too. total is subtotal + shipping + tax.
    An unknown zone raises ValueError and an order that is out of stock raises OutOfStock. In both cases
    nothing is reserved, and the zone is checked even when the order would ship free.
    """
    zone_cost = shipping.shipping_cents(weight_grams, zone)
    items = place_order(text, inventory)
    subtotal = sum(total for _, total in items)
    shipping_total = 0 if shipping.free_shipping(subtotal) else zone_cost
    taxable = subtotal + shipping_total
    tax = pricing.add_tax(taxable, tax_percent) - taxable
    return {"items": items, "subtotal": subtotal, "shipping": shipping_total, "tax": tax, "total": taxable + tax}
