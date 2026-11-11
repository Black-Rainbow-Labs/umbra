"""Hidden behavior checks for stockbook.py. Usage: python checks.py <folder with stockbook.py>. Prints JSON."""

import json
import sys

sys.dont_write_bytecode = True
sys.path.insert(0, sys.argv[1])

results = {}


def check(name, function):
    """Run one check and record whether it raised."""
    try:
        function()
        results[name] = ""
    except Exception as error:
        results[name] = "%s: %s" % (type(error).__name__, error)


def check_format_sku():
    from stockbook import format_sku
    assert format_sku(" ab ", 42) == "AB-000042", format_sku(" ab ", 42)
    assert format_sku("Xy", 7) == "XY-000007"
    assert format_sku("q", 1234567) == "Q-1234567"


def check_parse_quantity():
    from stockbook import parse_quantity
    assert parse_quantity(" 1,200 ") == 1200
    assert parse_quantity("12") == 12
    assert parse_quantity("") == 0
    assert parse_quantity("   ") == 0
    assert parse_quantity("2,000,000") == 2000000
    for bad in ["-5", "1.5", "abc", "-0,5"]:
        try:
            parse_quantity(bad)
        except ValueError:
            continue
        raise AssertionError("no ValueError for %r" % bad)


def check_round_price():
    from stockbook import round_price
    assert round_price(2.675) == 2.68, round_price(2.675)
    assert round_price(-2.675) == -2.68, round_price(-2.675)
    assert round_price(1.005) == 1.01
    assert round_price(0.125) == 0.13
    assert round_price(3) == 3.0 and isinstance(round_price(3), float)
    assert round_price(19.994) == 19.99


def check_low_stock_items():
    from stockbook import low_stock_items
    items = [
        {"sku": "C", "quantity": 5}, {"sku": "A", "quantity": 5}, {"sku": "B", "quantity": 1},
        {"sku": "D", "quantity": 6}, {"sku": "E", "quantity": 0},
    ]
    assert [item["sku"] for item in low_stock_items(items, 5)] == ["E", "B", "A", "C"]
    assert [item["sku"] for item in low_stock_items(items)] == ["E", "B", "A", "C"]
    assert low_stock_items([], 5) == []
    assert [item["sku"] for item in low_stock_items(items, 0)] == ["E"]


def check_apply_discount():
    from stockbook import apply_discount
    assert apply_discount(80.0, 12.5) == 70.0
    assert apply_discount(19.99, 10) == 17.99, apply_discount(19.99, 10)
    assert apply_discount(50, 150) == 0.0
    assert apply_discount(50, -20) == 50.0
    assert apply_discount(100, 0) == 100.0
    assert apply_discount(0.99, 50) == 0.5, apply_discount(0.99, 50)


def check_summarize_warehouse():
    from stockbook import summarize_warehouse
    items = [
        {"sku": "A", "quantity": 3, "price": 19.99},
        {"sku": "B", "quantity": 2, "price": 5.5},
        {"sku": "A", "quantity": 1, "price": 19.99},
    ]
    result = summarize_warehouse(items)
    assert result == {"total_units": 6, "distinct_skus": 2, "value": 90.96}, result
    assert summarize_warehouse([]) == {"total_units": 0, "distinct_skus": 0, "value": 0.0}


check("format_sku", check_format_sku)
check("parse_quantity", check_parse_quantity)
check("round_price", check_round_price)
check("low_stock_items", check_low_stock_items)
check("apply_discount", check_apply_discount)
check("summarize_warehouse", check_summarize_warehouse)
print(json.dumps(results))
