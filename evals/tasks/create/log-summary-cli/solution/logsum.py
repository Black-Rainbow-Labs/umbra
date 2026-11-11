"""Print a summary of a log file."""
import re
import sys
from collections import Counter

LINE = re.compile(r"^(\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d) (ERROR|WARN|INFO|DEBUG) ([a-z0-9_-]+): (.*)$")
LEVELS = ["ERROR", "WARN", "INFO", "DEBUG"]


def read_lines(path):
    """Return the lines of the file without line endings."""
    with open(path, "r", encoding="utf-8", newline="") as handle:
        text = handle.read()
    return [line.rstrip("\r") for line in text.split("\n")]


def scan(lines):
    """Return (entries, skipped) where entries is a list of (timestamp, level, service)."""
    entries = []
    skipped = 0
    for line in lines:
        if not line.strip():
            continue
        match = LINE.match(line)
        if match:
            entries.append((match.group(1), match.group(2), match.group(3)))
        else:
            skipped += 1
    return entries, skipped


def render(entries, skipped):
    """Return the summary text lines."""
    out = ["total: %d" % len(entries)]
    if not entries:
        if skipped:
            out.append("skipped: %d" % skipped)
        return out
    levels = Counter(level for _, level, _ in entries)
    out.extend("%s: %d" % (level, levels[level]) for level in LEVELS)
    if skipped:
        out.append("skipped: %d" % skipped)
    stamps = [stamp for stamp, _, _ in entries]
    out.append("first: %s" % min(stamps))
    out.append("last: %s" % max(stamps))
    out.append("top services:")
    services = Counter(service for _, _, service in entries)
    ranked = sorted(services.items(), key=lambda item: (-item[1], item[0]))[:3]
    out.extend("  %s: %d" % item for item in ranked)
    return out


def main(argv):
    """Entry point: print the summary for the file named in argv."""
    if len(argv) < 2:
        sys.stderr.write("usage: python logsum.py <logfile>\n")
        return 2
    try:
        lines = read_lines(argv[1])
    except OSError:
        sys.stderr.write("error: cannot read %s\n" % argv[1])
        return 2
    entries, skipped = scan(lines)
    sys.stdout.write("\n".join(render(entries, skipped)) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
