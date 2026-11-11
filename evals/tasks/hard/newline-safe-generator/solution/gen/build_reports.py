"""Builds the order reports from a CSV export.

Usage: python gen/build_reports.py [--input input/orders.csv] [--out out]
Writes summary.txt, orders.tsv, by_customer.json and report.md into the output folder.
All files are UTF-8 without a BOM, with LF line endings and one newline at the end.
"""

import argparse
import csv
import json
import os
from decimal import Decimal, ROUND_HALF_UP

CENT = Decimal("0.01")


def parse_amount(text):
    """Parse an amount with optional thousands separators and round it half up to two decimals."""
    return Decimal(text.strip().replace(",", "")).quantize(CENT, rounding=ROUND_HALF_UP)


def id_key(order_id):
    """Sort key for ids such as A-12: the number after the last hyphen, then the whole id."""
    number = order_id.rsplit("-", 1)[-1]
    return (int(number) if number.isdigit() else 0, order_id)


def read_orders(path):
    """Read the CSV export (CRLF or LF) and return a list of order dicts with cleaned fields."""
    with open(path, "r", encoding="utf-8", newline="") as handle:
        rows = list(csv.DictReader(handle))
    orders = []
    for row in rows:
        if not any((value or "").strip() for value in row.values()):
            continue
        orders.append({
            "id": row["id"].strip(),
            "customer": row["customer"].strip(),
            "amount": parse_amount(row["amount"]),
            "status": row["status"].strip().lower(),
        })
    return sorted(orders, key=lambda order: id_key(order["id"]))


def active_orders(orders):
    """Return the orders whose status is not cancelled."""
    return [order for order in orders if order["status"] != "cancelled"]


def write_text(path, text):
    """Write text as UTF-8 without a BOM, keeping LF line endings on every platform."""
    with open(path, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(text)


def summary_text(orders):
    """Build the text of summary.txt."""
    active = active_orders(orders)
    total = sum((order["amount"] for order in active), Decimal("0.00"))
    largest = max(active, key=lambda order: order["amount"])
    customers = len(set(order["customer"] for order in active))
    return "orders: %d\ncustomers: %d\ntotal: %s\nlargest: %s (%s)\n" % (len(active), customers, total, largest["id"], largest["amount"])


def tsv_text(orders):
    """Build the text of orders.tsv (every order, including cancelled ones)."""
    lines = ["id\tcustomer\tamount\tstatus"]
    for order in orders:
        lines.append("%s\t%s\t%s\t%s" % (order["id"], order["customer"], order["amount"], order["status"]))
    return "\n".join(lines) + "\n"


def json_text(orders):
    """Build the text of by_customer.json."""
    result = {}
    for order in active_orders(orders):
        entry = result.setdefault(order["customer"], {"orders": 0, "total": Decimal("0.00")})
        entry["orders"] += 1
        entry["total"] += order["amount"]
    for entry in result.values():
        entry["total"] = str(entry["total"])
    return json.dumps(result, indent=2, sort_keys=True, ensure_ascii=False) + "\n"


def report_text(orders):
    """Build the text of report.md."""
    active = active_orders(orders)
    lines = ["# Order report", "", "| id | customer | amount |", "|---|---|---|"]
    for order in active:
        lines.append("| %s | %s | %s |" % (order["id"], order["customer"], order["amount"]))
    total = sum((order["amount"] for order in active), Decimal("0.00"))
    return "\n".join(lines) + "\n\nTotal: %s\n" % total


def main():
    """Read the export and write the four report files."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", default="input/orders.csv")
    parser.add_argument("--out", default="out")
    args = parser.parse_args()
    orders = read_orders(args.input)
    os.makedirs(args.out, exist_ok=True)
    write_text(os.path.join(args.out, "summary.txt"), summary_text(orders))
    write_text(os.path.join(args.out, "orders.tsv"), tsv_text(orders))
    write_text(os.path.join(args.out, "by_customer.json"), json_text(orders))
    write_text(os.path.join(args.out, "report.md"), report_text(orders))
    print("wrote 4 files to %s" % args.out)


if __name__ == "__main__":
    main()
