// Project files for the tabs-makefile-tsv task: the Makefile, the TSV data, the Python tools, and the expected results.

export const COLUMNS_BEFORE = ["sku", "name", "category", "price_cents", "stock", "note"];
export const COLUMNS_AFTER = ["sku", "name", "category", "price", "stock", "note", "status"];

/**
 * Small deterministic random number generator (mulberry32).
 * @param seed - integer seed
 */
function makeRandom(seed) {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x6d2b79f5) >>> 0;
    let t = state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/**
 * Builds the product rows (arrays of six strings) with fixed edge cases among generated ones.
 * @param seed - integer seed
 */
export function productRows(seed) {
  const random = makeRandom(seed);
  const names = ["Garden hose", "Claw hammer", "Desk lamp", "Notebook A5", "Crème brûlée torch", "12\" ruler", "Tea kettle", "Jigsaw puzzle", "Paper clips", "Wall clock", "Plant pot", "Screw set", "Yoga mat", "Lunch box"];
  const categories = ["garden", "tools", "office", "kitchen", "toys", "home"];
  const notes = ["", "", "", "seasonal", "discontinued soon", "", "supplier: Müller & Co", "fragile"];
  const rows = [];
  for (let index = 1; index <= 40; index += 1) {
    const name = names[Math.floor(random() * names.length)] + (index % 3 === 0 ? ` ${index}` : "");
    const category = categories[Math.floor(random() * categories.length)];
    const price = String(5 + Math.floor(random() * 250000));
    const stock = String(Math.floor(random() * 60));
    rows.push([`P${String(index).padStart(4, "0")}`, name, category, price, stock, notes[Math.floor(random() * notes.length)]]);
  }
  rows[0][3] = "5";
  rows[1][3] = "100000";
  rows[2][3] = "100";
  rows[3][3] = "";
  rows[4][4] = "0";
  rows[5][4] = "9";
  rows[6][4] = "10";
  rows[7][4] = "1";
  rows[8][5] = "";
  rows[39][5] = "";
  rows[39][4] = "3";
  return rows;
}

/**
 * Returns the TSV file text before the change (LF line endings and a final newline).
 * @param seed - integer seed
 */
export function productsBefore(seed) {
  const lines = [COLUMNS_BEFORE.join("\t"), ...productRows(seed).map((row) => row.join("\t"))];
  return lines.join("\n") + "\n";
}

/**
 * Formats a whole number of cents as dollars with two decimals.
 * @param cents - digits only
 */
export function centsToDollars(cents) {
  const value = Number(cents);
  return `${Math.floor(value / 100)}.${String(value % 100).padStart(2, "0")}`;
}

/**
 * Returns the stock status word for a stock count.
 * @param stock - digits only
 */
export function statusFor(stock) {
  const count = Number(stock);
  return count === 0 ? "out" : count < 10 ? "low" : "ok";
}

/**
 * Returns the rows after the change: seven fields, price in dollars, status last.
 * @param seed - integer seed
 */
export function expectedRows(seed) {
  return productRows(seed).map((row) => [row[0], row[1], row[2], row[3] === "" ? "" : centsToDollars(row[3]), row[4], row[5], statusFor(row[4])]);
}

/**
 * Returns the TSV file text the task asks for.
 * @param seed - integer seed
 */
export function productsAfter(seed) {
  const lines = [COLUMNS_AFTER.join("\t"), ...expectedRows(seed).map((row) => row.join("\t"))];
  return lines.join("\n") + "\n";
}

/**
 * Returns the Makefile as it is before the change, as an array of lines (recipe lines start with a tab).
 */
export function makefileLines() {
  return [
    "# Product data reports.",
    '# Run "make all" to build the statistics and check the data file.',
    "",
    "PYTHON = python",
    "DATA = data/products.tsv",
    "OUT = build",
    "DIST = dist",
    "COLUMNS = 6",
    "",
    ".PHONY: all stats check clean",
    "",
    "all: stats check",
    "",
    "$(OUT):",
    "\tmkdir -p $(OUT)",
    "",
    "# Statistics for the price column.",
    "stats: $(OUT) $(DATA)",
    "\t$(PYTHON) tools/tsvstat.py $(DATA) price_cents > $(OUT)/stats.txt",
    "",
    "# The data file must have the expected number of columns on every line.",
    "check: $(DATA)",
    "\t$(PYTHON) tools/tsvcheck.py $(DATA) $(COLUMNS)",
    "",
    "clean:",
    "\trm -rf $(OUT)",
  ];
}

/**
 * Returns the Makefile text of the reference solution.
 */
export function solvedMakefileLines() {
  return [
    "# Product data reports.",
    '# Run "make all" to build the statistics and check the data file.',
    "",
    "PYTHON = python",
    "DATA = data/products.tsv",
    "OUT = build",
    "DIST = dist",
    "COLUMNS = 7",
    "",
    ".PHONY: all stats check clean dist",
    "",
    "all: stats check",
    "",
    "$(OUT):",
    "\tmkdir -p $(OUT)",
    "",
    "# Statistics for the price column.",
    "stats: $(OUT) $(DATA)",
    "\t$(PYTHON) tools/tsvstat.py $(DATA) price > $(OUT)/stats.txt",
    "",
    "# The data file must have the expected number of columns on every line.",
    "check: $(DATA)",
    "\t$(PYTHON) tools/tsvcheck.py $(DATA) $(COLUMNS)",
    "",
    "$(OUT)/lowstock.tsv: $(DATA) $(OUT)",
    "\t$(PYTHON) tools/tsvfilter.py $(DATA) status low out > $(OUT)/lowstock.tsv",
    "",
    "$(OUT)/categories.txt: $(DATA) $(OUT)",
    "\t$(PYTHON) tools/tsvcolumn.py $(DATA) category | sort -u > $(OUT)/categories.txt",
    "",
    "dist: stats $(OUT)/lowstock.tsv $(OUT)/categories.txt",
    "\tmkdir -p $(DIST)",
    "\t$(PYTHON) tools/bundle.py $(DIST)/bundle.txt $(OUT)/stats.txt $(OUT)/lowstock.tsv $(OUT)/categories.txt",
    "",
    "clean:",
    "\trm -rf $(OUT) $(DIST)",
  ];
}

/**
 * Returns the helper Python scripts keyed by project relative path.
 */
export function toolFiles() {
  return {
    "tools/tsvcommon.py": [
      '"""Shared helpers for the TSV tools."""',
      "",
      "import sys",
      "",
      "",
      "def read_rows(path):",
      '    """Return the lines of a TSV file (without line endings) as lists of fields."""',
      '    with open(path, "rb") as handle:',
      '        text = handle.read().decode("utf-8")',
      '    return [line.split("\\t") for line in text.split("\\n")[:-1]]',
      "",
      "",
      "def emit(lines):",
      '    """Write lines to standard output with LF endings on every platform."""',
      '    sys.stdout.buffer.write(("\\n".join(lines) + "\\n").encode("utf-8"))',
      "",
      "",
      "def column_index(header, name):",
      '    """Return the position of a column in the header or exit with an error."""',
      "    if name not in header:",
      '        sys.exit("no column named %s in %s" % (name, header))',
      "    return header.index(name)",
      "",
    ].join("\n"),
    "tools/tsvstat.py": [
      '"""Usage: tsvstat.py FILE COLUMN. Prints the count, sum, min and max of a numeric column; empty cells are skipped."""',
      "",
      "import sys",
      "from decimal import Decimal",
      "",
      "from tsvcommon import column_index, emit, read_rows",
      "",
      "rows = read_rows(sys.argv[1])",
      "index = column_index(rows[0], sys.argv[2])",
      'values = [Decimal(row[index]) for row in rows[1:] if row[index] != ""]',
      "emit([",
      '    "column: %s" % sys.argv[2],',
      '    "count: %d" % len(values),',
      '    "sum: %s" % sum(values),',
      '    "min: %s" % min(values),',
      '    "max: %s" % max(values),',
      "])",
      "",
    ].join("\n"),
    "tools/tsvcheck.py": [
      '"""Usage: tsvcheck.py FILE COLUMNS. Exits with 1 unless every line has exactly COLUMNS tab separated fields and the file ends with one newline."""',
      "",
      "import sys",
      "",
      'with open(sys.argv[1], "rb") as handle:',
      "    data = handle.read()",
      "expected = int(sys.argv[2])",
      'if not data.endswith(b"\\n") or b"\\r" in data:',
      '    sys.exit("file must use LF line endings and end with a newline")',
      "count = 0",
      'for number, line in enumerate(data.split(b"\\n")[:-1], 1):',
      '    fields = line.count(b"\\t") + 1',
      "    if fields != expected:",
      '        sys.exit("line %d has %d columns, expected %d" % (number, fields, expected))',
      "    count += 1",
      'print("ok: %d lines, %d columns" % (count, expected))',
      "",
    ].join("\n"),
    "tools/tsvfilter.py": [
      '"""Usage: tsvfilter.py FILE COLUMN VALUE... Prints the header and every row whose COLUMN equals one of the VALUEs, unchanged."""',
      "",
      "import sys",
      "",
      "from tsvcommon import column_index",
      "",
      'with open(sys.argv[1], "rb") as handle:',
      '    lines = handle.read().decode("utf-8").split("\\n")[:-1]',
      'index = column_index(lines[0].split("\\t"), sys.argv[2])',
      "wanted = set(sys.argv[3:])",
      'out = [lines[0]] + [line for line in lines[1:] if line.split("\\t")[index] in wanted]',
      'sys.stdout.buffer.write(("\\n".join(out) + "\\n").encode("utf-8"))',
      "",
    ].join("\n"),
    "tools/tsvcolumn.py": [
      '"""Usage: tsvcolumn.py FILE COLUMN. Prints the non empty values of a column, one per line, in file order."""',
      "",
      "import sys",
      "",
      "from tsvcommon import column_index, emit, read_rows",
      "",
      "rows = read_rows(sys.argv[1])",
      "index = column_index(rows[0], sys.argv[2])",
      'emit([row[index] for row in rows[1:] if row[index] != ""])',
      "",
    ].join("\n"),
    "tools/bundle.py": [
      '"""Usage: bundle.py OUTFILE INPUT... Writes each INPUT after a line "== <file name> ==", adding a final newline when one is missing."""',
      "",
      "import os",
      "import sys",
      "",
      'with open(sys.argv[1], "wb") as out:',
      "    for path in sys.argv[2:]:",
      '        with open(path, "rb") as handle:',
      "            data = handle.read()",
      '        if not data.endswith(b"\\n"):',
      '            data += b"\\n"',
      '        out.write(("== %s ==\\n" % os.path.basename(path)).encode("utf-8") + data)',
      "",
    ].join("\n"),
  };
}

/**
 * Returns the expected content of build/stats.txt for the changed data.
 * @param seed - integer seed
 */
export function expectedStats(seed) {
  const cents = productRows(seed).map((row) => row[3]).filter((value) => value !== "").map(Number);
  const total = cents.reduce((sum, value) => sum + value, 0);
  return ["column: price", `count: ${cents.length}`, `sum: ${centsToDollars(total)}`, `min: ${centsToDollars(Math.min(...cents))}`, `max: ${centsToDollars(Math.max(...cents))}`, ""].join("\n");
}

/**
 * Returns the expected content of build/lowstock.tsv.
 * @param seed - integer seed
 */
export function expectedLowStock(seed) {
  const rows = expectedRows(seed).filter((row) => row[6] === "low" || row[6] === "out");
  return [COLUMNS_AFTER.join("\t"), ...rows.map((row) => row.join("\t"))].join("\n") + "\n";
}

/**
 * Returns the expected content of build/categories.txt.
 * @param seed - integer seed
 */
export function expectedCategories(seed) {
  return [...new Set(productRows(seed).map((row) => row[2]))].sort().join("\n") + "\n";
}

/**
 * Returns the expected content of dist/bundle.txt.
 * @param seed - integer seed
 */
export function expectedBundle(seed) {
  const parts = [["stats.txt", expectedStats(seed)], ["lowstock.tsv", expectedLowStock(seed)], ["categories.txt", expectedCategories(seed)]];
  return parts.map(([name, text]) => `== ${name} ==\n${text}`).join("");
}
