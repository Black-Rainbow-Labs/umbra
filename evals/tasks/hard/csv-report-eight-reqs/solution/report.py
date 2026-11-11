"""Sales report tool. Reads a CSV of sales rows and writes revenue per category as CSV."""
import argparse
import csv
import io
import re
import sys
from decimal import Decimal, ROUND_HALF_UP

REQUIRED = ["category", "quantity", "price"]
QUANTITY_PATTERN = re.compile(r"^\d+$")
PRICE_PATTERN = re.compile(r"^\d+(\.\d+)?$")


def read_records(path):
    """Reads the file as UTF-8 (BOM allowed) and returns the non blank CSV records."""
    with open(path, "r", encoding="utf-8-sig", newline="") as handle:
        return [record for record in csv.reader(handle) if record]


def find_columns(header):
    """Maps each required column name to its index, or returns the missing names."""
    names = [name.strip().lower() for name in header]
    missing = [name for name in REQUIRED if name not in names]
    return {name: names.index(name) for name in REQUIRED if name in names}, missing


def check_row(row, columns, width):
    """Returns the reason a row is bad, or an empty string when it is good."""
    if len(row) != width:
        return "wrong number of fields"
    if row[columns["category"]].strip() == "":
        return "empty category"
    if not QUANTITY_PATTERN.match(row[columns["quantity"]].strip()):
        return "invalid quantity"
    if not PRICE_PATTERN.match(row[columns["price"]].strip()):
        return "invalid price"
    return ""


def aggregate(records, columns):
    """Totals the good rows per category and returns (totals, bad row count)."""
    totals = {}
    bad = 0
    for number, row in enumerate(records[1:], 1):
        reason = check_row(row, columns, len(records[0]))
        if reason:
            sys.stderr.write("row %d: %s\n" % (number, reason))
            bad += 1
            continue
        quantity = int(row[columns["quantity"]].strip())
        revenue = quantity * Decimal(row[columns["price"]].strip())
        entry = totals.setdefault(row[columns["category"]].strip(), [0, 0, Decimal(0)])
        entry[0] += 1
        entry[1] += quantity
        entry[2] += revenue
    return totals, bad


def money(value):
    """Rounds a Decimal to two places, halves going up."""
    return str(value.quantize(Decimal("0.01"), rounding=ROUND_HALF_UP))


def render(totals):
    """Builds the report text: sorted category rows and the TOTAL row."""
    out = io.StringIO()
    writer = csv.writer(out, lineterminator="\n")
    writer.writerow(["category", "items", "quantity", "revenue"])
    ordered = sorted(totals.items(), key=lambda pair: (-pair[1][2], pair[0].casefold(), pair[0]))
    for name, (items, quantity, revenue) in ordered:
        writer.writerow([name, items, quantity, money(revenue)])
    writer.writerow(["TOTAL", sum(t[0] for t in totals.values()), sum(t[1] for t in totals.values()), money(sum((t[2] for t in totals.values()), Decimal(0)))])
    return out.getvalue()


def main():
    """Parses arguments, builds the report and writes it. Returns the exit code."""
    parser = argparse.ArgumentParser()
    parser.add_argument("input")
    parser.add_argument("--out")
    args = parser.parse_args()
    try:
        records = read_records(args.input)
    except (OSError, UnicodeDecodeError):
        sys.stderr.write("error: cannot read %s\n" % args.input)
        return 2
    if not records:
        sys.stderr.write("error: input has no header row\n")
        return 2
    columns, missing = find_columns(records[0])
    if missing:
        for name in missing:
            sys.stderr.write("error: missing column: %s\n" % name)
        return 2
    totals, bad = aggregate(records, columns)
    data = render(totals).encode("utf-8")
    if args.out:
        with open(args.out, "wb") as handle:
            handle.write(data)
    else:
        sys.stdout.buffer.write(data)
        sys.stdout.buffer.flush()
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
