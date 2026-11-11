"""First draft of the markdown table formatter."""


def format_table(text):
    """Formats one table given as text."""
    rows = []
    for line in text.split("\n"):
        rows.append([cell.strip() for cell in line.strip().strip("|").split("|")])
    widths = [max(len(row[i]) for row in rows if i < len(row)) for i in range(len(rows[0]))]
    lines = []
    for row in rows:
        lines.append("| " + " | ".join(cell.ljust(widths[i]) for i, cell in enumerate(row)) + " |")
    return "\n".join(lines)


def format_document(text):
    """Formats every table in a markdown document."""
    return text
