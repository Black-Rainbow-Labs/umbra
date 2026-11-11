"""Sales report tool. Reads a CSV of sales rows and writes revenue per category as CSV."""
import argparse
import csv
import io
import re
import sys
from decimal import Decimal, ROUND_HALF_UP, localcontext

REQUIRED = ["category", "quantity", "price"]
QUANTITY_PATTERN = re.compile(r"^\d+$")
PRICE_PATTERN = re.compile(r"^\d+(\.\d+)?$")


def read_text(path):
    """Reads the input (a file, or stdin when the path is a dash) as UTF-8 and drops a BOM."""
    if path == "-":
        data = sys.stdin.buffer.read()
    else:
        with open(path, "rb") as handle:
            data = handle.read()
    return data.decode("utf-8-sig")


def read_records(text, delimiter):
    """Splits the text into CSV records with the given delimiter and drops blank lines."""
    return [record for record in csv.reader(io.StringIO(text, newline=""), delimiter=delimiter) if record]


def parse_delimiter(value):
    """Turns the --delimiter value into one character, or returns None when it is not allowed."""
    if value == "tab":
        return "\t"
    if len(value) == 1 and value not in '"\r\n':
        return value
    return None


def find_columns(header):
    """Maps each column name to the index of its first occurrence and lists missing required names."""
    names = [name.strip().lower() for name in header]
    columns = {}
    for index, name in enumerate(names):
        columns.setdefault(name, index)
    missing = [name for name in REQUIRED if name not in columns]
    return columns, missing


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
    if "discount" in columns:
        text = row[columns["discount"]].strip()
        if text != "" and (not PRICE_PATTERN.match(text) or Decimal(text) > 100):
            return "invalid discount"
    return ""


def row_revenue(row, columns, quantity):
    """Computes quantity times price, reduced by the discount percentage when there is one."""
    revenue = quantity * Decimal(row[columns["price"]].strip())
    if "discount" in columns:
        text = row[columns["discount"]].strip()
        if text != "":
            revenue = revenue * (Decimal(100) - Decimal(text)) / Decimal(100)
    return revenue


def aggregate(records, columns):
    """Totals the good rows per category and returns (totals, list of (row number, reason))."""
    totals = {}
    bad = []
    with localcontext() as context:
        context.prec = 200
        for number, row in enumerate(records[1:], 1):
            reason = check_row(row, columns, len(records[0]))
            if reason:
                bad.append((number, reason))
                continue
            quantity = int(row[columns["quantity"]].strip())
            entry = totals.setdefault(row[columns["category"]].strip(), [0, 0, Decimal(0)])
            entry[0] += 1
            entry[1] += quantity
            entry[2] += row_revenue(row, columns, quantity)
    return totals, bad


def money(value):
    """Rounds a Decimal to two places, halves going up."""
    with localcontext() as context:
        context.prec = 200
        return str(value.quantize(Decimal("0.01"), rounding=ROUND_HALF_UP))


def order_categories(totals, mode):
    """Sorts (name, [items, quantity, revenue]) pairs by the chosen mode with the stated tie breaks."""
    pairs = list(totals.items())
    if mode == "name":
        return sorted(pairs, key=lambda pair: (pair[0].casefold(), pair[0]))
    if mode == "items":
        return sorted(pairs, key=lambda pair: (-pair[1][0], -pair[1][2], pair[0].casefold(), pair[0]))
    return sorted(pairs, key=lambda pair: (-pair[1][2], pair[0].casefold(), pair[0]))


def render(totals, mode, top):
    """Builds the report text: sorted category rows, an optional Other row and the TOTAL row."""
    out = io.StringIO()
    writer = csv.writer(out, lineterminator="\n")
    writer.writerow(["category", "items", "quantity", "revenue"])
    ordered = order_categories(totals, mode)
    shown, rest = (ordered, []) if top is None else (ordered[:top], ordered[top:])
    for name, (items, quantity, revenue) in shown:
        writer.writerow([name, items, quantity, money(revenue)])
    if rest:
        writer.writerow(["Other", sum(t[0] for _, t in rest), sum(t[1] for _, t in rest), money(sum((t[2] for _, t in rest), Decimal(0)))])
    writer.writerow(["TOTAL", sum(t[0] for t in totals.values()), sum(t[1] for t in totals.values()), money(sum((t[2] for t in totals.values()), Decimal(0)))])
    return out.getvalue()


def render_bad_rows(bad):
    """Builds the --bad-rows file text: a header and one row,reason line per skipped record."""
    out = io.StringIO()
    writer = csv.writer(out, lineterminator="\n")
    writer.writerow(["row", "reason"])
    for number, reason in bad:
        writer.writerow([number, reason])
    return out.getvalue()


def build_parser():
    """Creates the argument parser for the command line."""
    parser = argparse.ArgumentParser()
    parser.add_argument("input")
    parser.add_argument("--out")
    parser.add_argument("--delimiter", default=",")
    parser.add_argument("--sort", choices=["revenue", "items", "name"], default="revenue")
    parser.add_argument("--top")
    parser.add_argument("--bad-rows", dest="bad_rows")
    return parser


def main():
    """Parses arguments, builds the report and writes it. Returns the exit code."""
    args = build_parser().parse_args()
    delimiter = parse_delimiter(args.delimiter)
    if delimiter is None:
        sys.stderr.write("error: invalid --delimiter\n")
        return 2
    top = None
    if args.top is not None:
        if not QUANTITY_PATTERN.match(args.top) or int(args.top) < 1:
            sys.stderr.write("error: invalid --top\n")
            return 2
        top = int(args.top)
    try:
        records = read_records(read_text(args.input), delimiter)
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
    for number, reason in bad:
        sys.stderr.write("row %d: %s\n" % (number, reason))
    data = render(totals, args.sort, top).encode("utf-8")
    if args.bad_rows:
        with open(args.bad_rows, "wb") as handle:
            handle.write(render_bad_rows(bad).encode("utf-8"))
    if args.out:
        with open(args.out, "wb") as handle:
            handle.write(data)
    else:
        sys.stdout.buffer.write(data)
        sys.stdout.buffer.flush()
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
