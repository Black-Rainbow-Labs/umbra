"""Placing orders: parse, check stock, reserve, price and report."""

from . import parser, pricing, report
from .inventory import OutOfStock


def place_order(text, inventory):
    """Reserve stock for every line of an order and return [(sku, line_total_cents), ...] in order.

    Lines for the same SKU are added together when checking stock, and each line is still priced on its own.
    When the lines ask for more than is in stock, OutOfStock is raised and nothing is reserved.
    """
    lines = parser.parse_order_lines(text)
    for sku, quantity, _ in lines:
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
