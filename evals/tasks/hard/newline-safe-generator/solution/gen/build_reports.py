"""Builds the order reports from a CSV export.

Usage: python gen/build_reports.py [--input input/orders.csv] [--out out]
Writes summary.txt, orders.tsv, by_customer.json, report.md, customers.csv and by_month.csv into the output folder.
All files are UTF-8 without a BOM, with LF line endings and one newline at the end.
"""

import argparse
import csv
import io
import json
import os
import re
import sys
from decimal import Decimal, ROUND_HALF_UP

CENT = Decimal("0.01")
AMOUNT_PATTERN = re.compile(r"^[0-9]+(\.[0-9]+)?$")
MONTH_PATTERN = re.compile(r"^([0-9]{4}-(0[1-9]|1[0-2]))-[0-9]{2}$")
SPACES = re.compile(r"[ \t\r\n]+")
FILE_COUNT = 6


def parse_amount(text):
    """Parse an amount with optional thousands separators and round it half up to two decimals, or return None."""
    cleaned = text.strip(" \t\r\n").replace(",", "")
    if not AMOUNT_PATTERN.match(cleaned):
        return None
    return Decimal(cleaned).quantize(CENT, rounding=ROUND_HALF_UP)


def id_key(order_id):
    """Sort key for ids such as A-12: the number after the last hyphen (0 when there is none), then the whole id."""
    _, hyphen, tail = order_id.rpartition("-")
    number = int(tail) if hyphen and re.fullmatch("[0-9]+", tail) else 0
    return (number, order_id)


def month_of(date_text):
    """Return YYYY-MM for a date written as YYYY-MM-DD with a month from 01 to 12, otherwise 'unknown'."""
    match = MONTH_PATTERN.match(date_text.strip(" \t\r\n"))
    return match.group(1) if match else "unknown"


def read_records(path):
    """Read the file as UTF-8 (a BOM is allowed) and return its non blank CSV records."""
    with open(path, "rb") as handle:
        text = handle.read().decode("utf-8-sig")
    return [record for record in csv.reader(io.StringIO(text, newline="")) if record]


def clean_order(row, columns):
    """Return a cleaned order dict for a record, or the reason it is skipped as a string."""
    def field(name):
        index = columns.get(name)
        return row[index] if index is not None and index < len(row) else ""
    order_id = field("id").strip(" \t\r\n")
    if not order_id:
        return "missing id"
    amount = parse_amount(field("amount"))
    if amount is None:
        return "invalid amount"
    return {
        "id": order_id,
        "customer": SPACES.sub(" ", field("customer")).strip(" "),
        "amount": amount,
        "status": field("status").strip(" \t\r\n").lower(),
        "month": month_of(field("date")),
    }


def read_orders(path):
    """Read the CSV export and return the cleaned orders sorted by id. Skipped rows are reported on stderr."""
    records = read_records(path)
    if not records:
        raise ValueError("no header")
    columns = {}
    for index, name in enumerate(records[0]):
        columns.setdefault(name.strip(" \t\r\n"), index)
    for name in ("id", "customer", "amount", "status"):
        if name not in columns:
            raise ValueError("missing column: %s" % name)
    orders = []
    for number, row in enumerate(records[1:], 1):
        order = clean_order(row, columns)
        if isinstance(order, str):
            sys.stderr.write("skipped row %d: %s\n" % (number, order))
        else:
            orders.append(order)
    return sorted(orders, key=lambda order: id_key(order["id"]))


def counted_orders(orders):
    """Return the orders whose status is not cancelled."""
    return [order for order in orders if order["status"] != "cancelled"]


def signed(order):
    """Return the amount of an order as it counts in totals: negative for a refunded order."""
    return -order["amount"] if order["status"] == "refunded" and order["amount"] else order["amount"]


def total_of(orders):
    """Return the sum of the signed amounts of the orders."""
    return sum((signed(order) for order in orders), Decimal("0.00"))


def write_text(path, text):
    """Write text as UTF-8 without a BOM, keeping LF line endings on every platform."""
    with open(path, "w", encoding="utf-8", newline="\n") as handle:
        handle.write(text)


def summary_text(orders):
    """Build the text of summary.txt."""
    counted = counted_orders(orders)
    candidates = [order for order in counted if order["status"] != "refunded"]
    largest = max(candidates, key=lambda order: order["amount"]) if candidates else None
    customers = len(set(order["customer"] for order in counted))
    largest_text = "%s (%s)" % (largest["id"], largest["amount"]) if largest else "none"
    return "orders: %d\ncustomers: %d\ntotal: %s\nlargest: %s\n" % (len(counted), customers, total_of(counted), largest_text)


def tsv_cell(value):
    """Return a value for a TSV cell: tabs and line breaks cannot occur there, so they become spaces."""
    return SPACES.sub(" ", str(value))


def tsv_text(orders):
    """Build the text of orders.tsv (every order, including cancelled ones)."""
    lines = ["id\tcustomer\tamount\tstatus"]
    for order in orders:
        lines.append("\t".join(tsv_cell(order[key]) for key in ("id", "customer", "amount", "status")))
    return "\n".join(lines) + "\n"


def customer_totals(orders):
    """Return {customer: {"orders": n, "total": Decimal}} for the orders that count."""
    result = {}
    for order in counted_orders(orders):
        entry = result.setdefault(order["customer"], {"orders": 0, "total": Decimal("0.00")})
        entry["orders"] += 1
        entry["total"] += signed(order)
    return result


def json_text(orders):
    """Build the text of by_customer.json."""
    result = customer_totals(orders)
    for entry in result.values():
        entry["total"] = str(entry["total"])
    return json.dumps(result, indent=2, sort_keys=True, ensure_ascii=False) + "\n"


def report_text(orders):
    """Build the text of report.md."""
    counted = counted_orders(orders)
    lines = ["# Order report", "", "| id | customer | amount |", "|---|---|---|"]
    for order in counted:
        lines.append("| %s | %s | %s |" % (order["id"], order["customer"], signed(order)))
    return "\n".join(lines) + "\n\nTotal: %s\n" % total_of(counted)


def csv_text(header, rows):
    """Build CSV text with LF line endings, quoting only the fields that need it."""
    out = io.StringIO()
    writer = csv.writer(out, lineterminator="\n")
    writer.writerow(header)
    writer.writerows(rows)
    return out.getvalue()


def customers_csv_text(orders):
    """Build the text of customers.csv: customers by total (largest first), then by name."""
    totals = customer_totals(orders)
    ordered = sorted(totals.items(), key=lambda pair: (-pair[1]["total"], pair[0]))
    return csv_text(["customer", "orders", "total"], [[name, entry["orders"], entry["total"]] for name, entry in ordered])


def by_month_csv_text(orders):
    """Build the text of by_month.csv: one row per month in order, then the orders without a usable date."""
    months = {}
    for order in counted_orders(orders):
        entry = months.setdefault(order["month"], {"orders": 0, "total": Decimal("0.00")})
        entry["orders"] += 1
        entry["total"] += signed(order)
    names = sorted(name for name in months if name != "unknown") + (["unknown"] if "unknown" in months else [])
    return csv_text(["month", "orders", "total"], [[name, months[name]["orders"], months[name]["total"]] for name in names])


def write_reports(orders, out_dir):
    """Write the six report files into the output folder, creating it when needed."""
    os.makedirs(out_dir, exist_ok=True)
    write_text(os.path.join(out_dir, "summary.txt"), summary_text(orders))
    write_text(os.path.join(out_dir, "orders.tsv"), tsv_text(orders))
    write_text(os.path.join(out_dir, "by_customer.json"), json_text(orders))
    write_text(os.path.join(out_dir, "report.md"), report_text(orders))
    write_text(os.path.join(out_dir, "customers.csv"), customers_csv_text(orders))
    write_text(os.path.join(out_dir, "by_month.csv"), by_month_csv_text(orders))


def main():
    """Read the export and write the report files. Returns the exit code."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--input", default="input/orders.csv")
    parser.add_argument("--out", default="out")
    args = parser.parse_args()
    try:
        orders = read_orders(args.input)
    except (OSError, UnicodeDecodeError, ValueError):
        sys.stderr.write("error: cannot read %s\n" % args.input)
        return 2
    write_reports(orders, args.out)
    sys.stdout.buffer.write(("wrote %d files to %s\n" % (FILE_COUNT, args.out)).encode("utf-8"))
    sys.stdout.buffer.flush()
    return 0


if __name__ == "__main__":
    sys.exit(main())
