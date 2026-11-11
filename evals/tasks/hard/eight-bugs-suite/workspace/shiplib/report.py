"""Plain text reports."""


def format_cents(cents):
    """Format whole cents as dollars, for example 1234 becomes '$12.34'."""
    return f"${cents // 100}.{cents % 100:02d}"


def render_report(items):
    """Render order items, given as (name, total_cents) pairs, one per line as 'name: $12.34'.

    Items are sorted by total, largest first. Items with equal totals are sorted by name, A to Z.
    The last line is 'TOTAL: $...' with the sum of all item totals.
    """
    ordered = sorted(items, key=lambda item: (str(item[1]), item[0]), reverse=True)
    lines = [f"{name}: {format_cents(total)}" for name, total in ordered]
    lines.append(f"TOTAL: {format_cents(sum(total for _, total in items))}")
    return "\n".join(lines)
