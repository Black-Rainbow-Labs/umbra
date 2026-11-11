"""First draft of the sales report tool. Reads a CSV and prints revenue per category."""
import sys


def main():
    path = sys.argv[1]
    totals = {}
    with open(path) as handle:
        lines = handle.read().split("\n")
    header = lines[0].split(",")
    for line in lines[1:]:
        if not line:
            continue
        fields = line.split(",")
        row = dict(zip(header, fields))
        revenue = float(row["quantity"]) * float(row["price"])
        entry = totals.setdefault(row["category"], [0, 0, 0.0])
        entry[0] += 1
        entry[1] += int(row["quantity"])
        entry[2] += revenue
    print("category,items,quantity,revenue")
    grand = 0.0
    for category in sorted(totals):
        items, quantity, revenue = totals[category]
        print("%s,%d,%d,%.2f" % (category, items, quantity, round(revenue, 2)))
        grand += round(revenue, 2)
    print("TOTAL,%d,%d,%.2f" % (sum(t[0] for t in totals.values()), sum(t[1] for t in totals.values()), grand))


if __name__ == "__main__":
    main()
