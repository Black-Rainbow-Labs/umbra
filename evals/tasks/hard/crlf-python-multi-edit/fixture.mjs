// Builds the CRLF Python module (stockbook.py), its visible test runner, and the reference edits.

export const MODULE_NAME = "stockbook.py";
export const RUNNER_NAME = "run_tests.py";
export const TARGET_NAMES = ["format_sku", "parse_quantity", "round_price", "low_stock_items", "apply_discount", "summarize_warehouse"];

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

const VERBS = ["group", "collect", "count", "scan", "merge", "split", "rank", "filter", "pack", "trim", "index", "tally"];
const NOUNS = ["bins", "pallets", "crates", "shelves", "lots", "batches", "aisles", "orders", "labels", "routes", "docks", "tags"];
const KEYS = ["zone", "owner", "kind", "batch", "supplier", "shelf", "carrier", "lane"];

/**
 * Returns the original and the changed source (LF lines) of the six target functions.
 */
export function targetSources() {
  const original = {};
  const solved = {};
  original.format_sku = [
    "def format_sku(prefix, number):",
    '    """Build a SKU label from a prefix and a number."""',
    '    return "%s-%s" % (prefix, number)',
  ];
  solved.format_sku = [
    "def format_sku(prefix, number):",
    '    """Build a SKU label: upper case prefix, a hyphen, and the number padded to six digits."""',
    '    return "%s-%06d" % (str(prefix).strip().upper(), int(number))',
  ];
  original.parse_quantity = [
    "def parse_quantity(text):",
    '    """Turn text into a whole number of units."""',
    "    value = int(text)",
    "    return value",
  ];
  solved.parse_quantity = [
    "def parse_quantity(text):",
    '    """Turn text into a whole number of units; commas are thousands separators, blank means 0."""',
    '    cleaned = text.strip().replace(",", "")',
    '    if cleaned == "":',
    "        return 0",
    "    value = int(cleaned)",
    "    if value < 0:",
    '        raise ValueError("quantity cannot be negative: %r" % text)',
    "    return value",
  ];
  original.round_price = [
    "def round_price(value):",
    '    """Round a price to two decimals."""',
    "    return round(value, 2)",
  ];
  solved.round_price = [
    "def round_price(value):",
    '    """Round a price to two decimals, halves away from zero, and return a float."""',
    '    cents = Decimal(str(value)).quantize(Decimal("0.01"), rounding=ROUND_HALF_UP)',
    "    return float(cents)",
  ];
  original.low_stock_items = [
    "def low_stock_items(items, threshold=DEFAULT_THRESHOLD):",
    '    """Return the items whose quantity is below the threshold."""',
    "    result = []",
    "    for item in items:",
    '        if item["quantity"] < threshold:',
    "            result.append(item)",
    "    return result",
  ];
  solved.low_stock_items = [
    "def low_stock_items(items, threshold=DEFAULT_THRESHOLD):",
    '    """Return the items at or below the threshold, lowest quantity first, ties by sku."""',
    "    result = []",
    "    for item in items:",
    '        if item["quantity"] <= threshold:',
    "            result.append(item)",
    '    return sorted(result, key=lambda item: (item["quantity"], item["sku"]))',
  ];
  original.apply_discount = [
    "def apply_discount(price, percent):",
    '    """Return the price after taking a percentage off."""',
    "    return price * (1 - percent / 100)",
  ];
  solved.apply_discount = [
    "def apply_discount(price, percent):",
    '    """Return the price after taking a percentage (limited to 0..100) off, rounded as a price."""',
    "    percent = max(0, min(100, percent))",
    "    return round_price(price * (100 - percent) / 100)",
  ];
  original.summarize_warehouse = [
    "def summarize_warehouse(items):",
    '    """Count the units held in the warehouse."""',
    "    total = 0",
    "    for item in items:",
    '        total += item["quantity"]',
    '    return {"total_units": total}',
  ];
  solved.summarize_warehouse = [
    "def summarize_warehouse(items):",
    '    """Count the units, the distinct SKUs and the stock value held in the warehouse."""',
    "    total = 0",
    "    value = 0.0",
    "    skus = set()",
    "    for item in items:",
    '        total += item["quantity"]',
    '        value += item["quantity"] * item["price"]',
    '        skus.add(item["sku"])',
    '    return {"total_units": total, "distinct_skus": len(skus), "value": round_price(value)}',
  ];
  return { original, solved };
}

/**
 * Near duplicates of the target functions. They must stay exactly as they are.
 */
function decoySources() {
  return {
    format_sku_legacy: [
      "def format_sku_legacy(prefix, number):",
      '    """Build a SKU label the way the 2019 importer expects it."""',
      '    return "%s-%s" % (prefix, number)',
    ],
    parse_quantity_strict: [
      "def parse_quantity_strict(text):",
      '    """Turn text into a whole number of units; no cleanup at all."""',
      "    value = int(text)",
      "    return value",
    ],
    round_price_display: [
      "def round_price_display(value):",
      '    """Round a price to two decimals for display only."""',
      "    return round(value, 2)",
    ],
    low_stock_items_report: [
      "def low_stock_items_report(items, threshold=DEFAULT_THRESHOLD):",
      '    """Return the items whose quantity is below the threshold, for the weekly report."""',
      "    result = []",
      "    for item in items:",
      '        if item["quantity"] < threshold:',
      "            result.append(item)",
      "    return result",
    ],
    apply_discount_bulk: [
      "def apply_discount_bulk(price, percent):",
      '    """Return the price after taking a percentage off, bulk orders."""',
      "    return price * (1 - percent / 100)",
    ],
    summarize_warehouse_brief: [
      "def summarize_warehouse_brief(items):",
      '    """Count the units held in the warehouse, short form."""',
      "    total = 0",
      "    for item in items:",
      '        total += item["quantity"]',
      '    return {"total_units": total}',
    ],
  };
}

/**
 * Builds one generated helper function (about 20 lines) from one of five templates.
 * @param template - template number 0..4
 * @param name - function name
 * @param key - record key the helper works on
 */
function fillerFunction(template, name, key) {
  const templates = [
    [
      `def ${name}(records, key="${key}"):`,
      `    """Group the records by \`key\` and return the groups, biggest first.`,
      "",
      "    Records without the key are collected under the empty string.",
      '    """',
      "    groups = {}",
      "    for record in records:",
      '        value = record.get(key, "")',
      "        if value not in groups:",
      "            groups[value] = []",
      "        groups[value].append(record)",
      "    ordered = sorted(groups.items(), key=lambda pair: (-len(pair[1]), str(pair[0])))",
      "    result = []",
      "    for value, members in ordered:",
      '        result.append({"value": value, "count": len(members)})',
      "    return result",
    ],
    [
      `def ${name}(values, size=${3 + (name.length % 4)}):`,
      '    """Split a list into chunks of at most `size` values.',
      "",
      "    The last chunk may be shorter. An empty list gives an empty list.",
      '    """',
      "    chunks = []",
      "    current = []",
      "    for value in values:",
      "        current.append(value)",
      "        if len(current) == size:",
      "            chunks.append(current)",
      "            current = []",
      "    if current:",
      "        chunks.append(current)",
      "    return chunks",
    ],
    [
      `def ${name}(rows, field="${key}"):`,
      '    """Return the distinct values of `field` in first seen order.',
      "",
      "    Rows that do not have the field are skipped.",
      '    """',
      "    seen = set()",
      "    ordered = []",
      "    for row in rows:",
      "        if field not in row:",
      "            continue",
      "        value = row[field]",
      "        if value in seen:",
      "            continue",
      "        seen.add(value)",
      "        ordered.append(value)",
      "    return ordered",
    ],
    [
      `def ${name}(text, width=${40 + (name.length % 5) * 4}):`,
      '    """Wrap text at `width` characters on word boundaries.',
      "",
      "    Words longer than the width stay on a line of their own.",
      '    """',
      "    lines = []",
      '    line = ""',
      "    for word in text.split():",
      "        if line and len(line) + 1 + len(word) > width:",
      "            lines.append(line)",
      '            line = ""',
      "        line = (line + \" \" + word) if line else word",
      "    if line:",
      "        lines.append(line)",
      "    return lines",
    ],
    [
      `def ${name}(entries, limit=${2 + (name.length % 5)}):`,
      '    """Return the `limit` entries with the highest "weight" value.',
      "",
      "    Ties keep their original order. Entries without a weight count as zero.",
      '    """',
      "    indexed = list(enumerate(entries))",
      '    indexed.sort(key=lambda pair: (-pair[1].get("weight", 0), pair[0]))',
      "    best = []",
      "    for _, entry in indexed[:limit]:",
      "        best.append(entry)",
      "    return best",
    ],
  ];
  return templates[template];
}

/**
 * Returns the first two hand written helpers, which the visible tests cover.
 */
function handWrittenHelpers() {
  return [
    [
      "def normalize_name(name):",
      '    """Collapse runs of spaces and trim a product name."""',
      '    return " ".join(name.split())',
    ],
    [
      "def chunk_list(values, size):",
      '    """Split a list into chunks of at most `size` values."""',
      "    return [values[index:index + size] for index in range(0, len(values), size)]",
    ],
  ];
}

/**
 * Builds the whole module as LF text. With `solved` true the six target functions are the changed ones.
 * @param seed - integer seed for the generated helpers
 * @param solved - true to use the reference versions of the six functions
 */
export function buildModuleText(seed, solved) {
  const random = makeRandom(seed);
  const { original, solved: changed } = targetSources();
  const decoys = decoySources();
  const targets = solved ? changed : original;
  const header = [
    '"""Stock book helpers for the warehouse tools."""',
    "",
    "from decimal import Decimal, ROUND_HALF_UP",
    "",
    "DEFAULT_THRESHOLD = 5",
    "CENT = Decimal(\"0.01\")",
    'LEGACY_EXAMPLE = "format_sku(\'ab\', 42) -> \'ab-42\'"',
  ];
  const blocks = [header, ...handWrittenHelpers()];
  const specials = [
    [8, targets.format_sku, decoys.format_sku_legacy],
    [17, decoys.parse_quantity_strict, targets.parse_quantity],
    [26, targets.round_price, decoys.round_price_display],
    [35, decoys.low_stock_items_report, targets.low_stock_items],
    [44, targets.apply_discount, decoys.apply_discount_bulk],
    [53, decoys.summarize_warehouse_brief, targets.summarize_warehouse],
  ];
  const usedNames = new Set();
  for (let index = 0; index < 84; index += 1) {
    const special = specials.find((entry) => entry[0] === index);
    if (special) blocks.push(special[1], special[2]);
    let name;
    do {
      name = `${VERBS[Math.floor(random() * VERBS.length)]}_${NOUNS[Math.floor(random() * NOUNS.length)]}`;
    } while (usedNames.has(name));
    usedNames.add(name);
    blocks.push(fillerFunction(Math.floor(random() * 5), name, KEYS[Math.floor(random() * KEYS.length)]));
  }
  return blocks.map((block) => block.join("\n")).join("\n\n\n") + "\n";
}

/**
 * Converts LF text to CRLF bytes.
 * @param text - LF text
 */
export function toCrlf(text) {
  return Buffer.from(text.replace(/\n/g, "\r\n"), "utf8");
}

const BASE_TESTS = [
  ["test_parse_quantity_plain", ['assert parse_quantity("12") == 12']],
  ["test_round_price_simple", ["assert round_price(1.5) == 1.5"]],
  ["test_low_stock_below_threshold", ['items = [{"sku": "A", "quantity": 2}, {"sku": "B", "quantity": 9}]', 'assert [item["sku"] for item in low_stock_items(items, 5)] == ["A"]']],
  ["test_apply_discount_zero_percent", ["assert apply_discount(100, 0) == 100"]],
  ["test_summarize_total_units", ['items = [{"sku": "A", "quantity": 2, "price": 1.0}, {"sku": "B", "quantity": 3, "price": 2.0}]', 'assert summarize_warehouse(items)["total_units"] == 5']],
  ["test_normalize_name_collapses_spaces", ['assert normalize_name("  big   box ") == "big box"']],
  ["test_chunk_list_sizes", ["assert chunk_list([1, 2, 3, 4, 5], 2) == [[1, 2], [3, 4], [5]]"]],
];

const ADDED_TESTS = [
  ["test_format_sku_pads_and_uppercases", ['assert format_sku("ab", 42) == "AB-000042"']],
  ["test_parse_quantity_separators", ['assert parse_quantity(" 1,200 ") == 1200', 'assert parse_quantity("") == 0']],
  ["test_round_price_half_up", ["assert round_price(2.675) == 2.68"]],
  ["test_low_stock_items_includes_threshold", ['items = [{"sku": "B", "quantity": 5}, {"sku": "A", "quantity": 5}]', 'assert [item["sku"] for item in low_stock_items(items, 5)] == ["A", "B"]']],
  ["test_apply_discount_clamps_percent", ["assert apply_discount(50, 150) == 0.0"]],
  ["test_summarize_warehouse_value", ['items = [{"sku": "A", "quantity": 3, "price": 19.99}, {"sku": "A", "quantity": 2, "price": 5.5}]', 'result = summarize_warehouse(items)', 'assert result["distinct_skus"] == 1 and result["value"] == 70.97']],
];

/**
 * Builds the test runner as LF text, optionally with the six tests the task asks for.
 * @param withAdded - true to include the added tests
 */
export function buildRunnerText(withAdded) {
  const lines = [
    '"""Tiny test runner: runs every test_* function in this file and prints a summary."""',
    "",
    "import sys",
    "import traceback",
    "",
    "from stockbook import (",
    "    apply_discount,",
    "    chunk_list,",
    "    format_sku,",
    "    low_stock_items,",
    "    normalize_name,",
    "    parse_quantity,",
    "    round_price,",
    "    summarize_warehouse,",
    ")",
    "",
  ];
  const tests = withAdded ? [...BASE_TESTS, ...ADDED_TESTS] : BASE_TESTS;
  for (const [name, body] of tests) {
    lines.push("", `def ${name}():`, ...body.map((line) => `    ${line}`), "");
  }
  lines.push(
    "",
    "def run_all():",
    '    """Run every test_* function and return the process exit code."""',
    '    names = sorted(name for name in globals() if name.startswith("test_"))',
    "    failed = 0",
    "    for name in names:",
    "        try:",
    "            globals()[name]()",
    "        except Exception:",
    "            failed += 1",
    '            print("FAIL", name)',
    "            traceback.print_exc()",
    '    print("%d passed, %d failed" % (len(names) - failed, failed))',
    "    return 1 if failed else 0",
    "",
    "",
    'if __name__ == "__main__":',
    "    sys.exit(run_all())",
    ""
  );
  return lines.join("\n");
}
