"""Builds the order reports from a CSV export.

Usage: python gen/build_reports.py [--input input/orders.csv] [--out out]
Writes summary.txt, orders.tsv, by_customer.json and report.md into the output folder.
"""

import argparse
import csv
import json
import os


def read_orders(path):
    """Read the CSV export and return a list of order dicts."""
    with open(path, "rb") as handle:
        text = handle.read().decode("utf-8")
    orders = []
    for row in csv.DictReader(text.split("\n")):
        orders.append({
            "id": row["id"],
            "customer": row["customer"],
            "amount": float(row["amount"]),
            "status": row["status"],
        })
    return orders


def active_orders(orders):
    """Return the orders that are not cancelled."""
    return [order for order in orders if order["status"] != "cancelled"]


def write_summary(orders, out_dir):
    """Write summary.txt with the order count, customer count, total and the largest order."""
    active = active_orders(orders)
    total = sum(order["amount"] for order in active)
    largest = max(active, key=lambda order: order["amount"])
    with open(os.path.join(out_dir, "summary.txt"), "w") as handle:
        handle.write("orders: %d\n" % len(active))
        handle.write("customers: %d\n" % len(set(order["customer"] for order in active)))
        handle.write("total: %s\n" % total)
        handle.write("largest: %s (%s)\n" % (largest["id"], largest["amount"]))
        handle.write("\n")


def write_tsv(orders, out_dir):
    """Write orders.tsv with every order, including cancelled ones, sorted by id."""
    with open(os.path.join(out_dir, "orders.tsv"), "w") as handle:
        writer = csv.writer(handle, delimiter="\t")
        writer.writerow(["id", "customer", "amount", "status"])
        for order in sorted(orders, key=lambda order: order["id"]):
            writer.writerow([order["id"], order["customer"], order["amount"], order["status"]])


def write_json(orders, out_dir):
    """Write by_customer.json with the order count and total for each customer."""
    result = {}
    for order in active_orders(orders):
        entry = result.setdefault(order["customer"], {"orders": 0, "total": 0.0})
        entry["orders"] += 1
        entry["total"] += order["amount"]
    with open(os.path.join(out_dir, "by_customer.json"), "w") as handle:
        json.dump(result, handle, indent=4)


def write_report(orders, out_dir):
    """Write report.md, a Markdown table of the orders that count."""
    active = active_orders(orders)
    with open(os.path.join(out_dir, "report.md"), "w", encoding="utf-8-sig") as handle:
        handle.write("# Order report\n\n")
        handle.write("| id | customer | amount |\n|---|---|---|\n")
        for order in active:
            handle.write("| %s | %s | %s |\n" % (order["id"], order["customer"], order["amount"]))
        handle.write("\nTotal: %s\n" % sum(order["amount"] for order in active))


def main():
    """Read the export and write the four report files."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", default="input/orders.csv")
    parser.add_argument("--out", default="out")
    args = parser.parse_args()
    orders = read_orders(args.input)
    os.makedirs(args.out)
    write_summary(orders, args.out)
    write_tsv(orders, args.out)
    write_json(orders, args.out)
    write_report(orders, args.out)
    print("wrote 4 files to %s" % args.out)


if __name__ == "__main__":
    main()
