// Builds the generated Python package for the config default task, in its original or solved form.

import { mulberry32, randomInt, pick, shuffled } from "./gen-utils.mjs";

export const TARGET_FILE = "shopkit/export/scheduler.py";
export const TARGET_LINE_ORIGINAL = '    return get_setting("export.max_retries", 3)';
export const TARGET_LINE_SOLVED = '    return get_setting("export.max_retries", 5)';

const PACKAGES = ["export", "billing", "orders", "catalog", "shipping", "inventory", "customers", "returns", "loyalty", "reports", "vendors", "support"];
const ADJECTIVES = ["bulk", "daily", "fast", "late", "open", "prime", "quick", "regional", "safe", "tiered", "unit", "weekly"];
const NOUNS = ["lookup", "ledger", "queue", "window", "ticket", "rollup", "digest", "cursor", "bucket", "filter", "router", "mapper"];
const SETTING_KINDS = [
  { suffix: "limit", values: [10, 25, 50, 100, 250] },
  { suffix: "timeout_seconds", values: [5, 10, 30, 60] },
  { suffix: "batch_size", values: [20, 40, 80, 200] },
  { suffix: "enabled", values: [true, false] },
  { suffix: "max_retries", values: [2, 3, 4] },
  { suffix: "interval_seconds", values: [15, 60, 300] },
];

/**
 * Formats a JavaScript value as a Python literal.
 * @param value - a number or boolean
 */
function pythonLiteral(value) {
  if (value === true) return "True";
  if (value === false) return "False";
  return String(value);
}

/**
 * Builds the text of one generated settings module with two or three settings readers.
 * @param pkg - the package name
 * @param name - the module name
 * @param rng - seeded generator
 * @param usedKeys - Set of keys already used, to keep generated keys unique
 */
function buildSettingsModule(pkg, name, rng, usedKeys) {
  const lines = [`"""${pkg} ${name.replace("_", " ")}: tuning values for the ${name} step."""`, "from shopkit.config import get_setting", ""];
  const noun = name.split("_")[1];
  for (const kind of shuffled(rng, SETTING_KINDS).slice(0, randomInt(rng, 2, 3))) {
    const key = `${pkg}.${noun}_${kind.suffix}`;
    if (usedKeys.has(key)) continue;
    usedKeys.add(key);
    lines.push("", `def ${noun}_${kind.suffix}():`, `    """Returns the ${kind.suffix.replace(/_/g, " ")} for the ${noun} step."""`);
    lines.push(`    return get_setting("${key}", ${pythonLiteral(pick(rng, kind.values))})`, "");
  }
  return lines.join("\n") + "\n";
}

/**
 * Builds the export scheduler module, which reads the target key and several similar looking keys.
 * @param solved - when true the target default is 5
 */
function buildSchedulerModule(solved) {
  return [
    '"""Export scheduler: retry rules for the nightly export."""',
    "from shopkit.config import get_setting",
    "",
    "",
    "def retry_limit():",
    '    """How many times a failed export is retried."""',
    solved ? TARGET_LINE_SOLVED : TARGET_LINE_ORIGINAL,
    "",
    "",
    "def retry_delay_seconds():",
    '    """Seconds to wait between retries."""',
    '    return get_setting("export.retry_delay_seconds", 3)',
    "",
    "",
    "def batch_retry_limit():",
    '    """How many times one batch inside an export is retried."""',
    '    return get_setting("export.max_retries_per_batch", 3)',
    "",
    "",
    "def failure_alert_threshold():",
    '    """How many failed exports in a row raise an alert."""',
    '    return get_setting("export.alert_after_failures", 3)',
    "",
  ].join("\n");
}

/**
 * Builds the modules that hold decoys: other keys whose names are close to the target key.
 */
function buildDecoyModules() {
  const files = new Map();
  files.set("shopkit/billing/retry_policy.py", ['"""Billing retry policy."""', "from shopkit.config import get_setting", "", "", "def retry_limit():", '    """How many times a failed charge is retried."""', '    return get_setting("billing.max_retries", 3)', ""].join("\n"));
  files.set("shopkit/exports_legacy/runner.py", ['"""Legacy export runner kept for old customers."""', "from shopkit.config import get_setting", "", "", "def retry_limit():", '    """How many times the legacy exporter retries."""', '    return get_setting("exports.max_retries", 3)', ""].join("\n"));
  files.set("shopkit/export/notes.py", ['"""Notes about the export settings."""', "", '# The key export.max_retries is read in scheduler.py. Teams override it in their settings file.', 'SETTING_NOTE = "export.max_retries defaults to the value in scheduler.py"', ""].join("\n"));
  return files;
}

/**
 * Builds the non code files: a sample settings fixture and a docs page that mention the key.
 */
function buildSupportFiles() {
  const files = new Map();
  files.set("tests/fixtures/settings_sample.json", JSON.stringify({ "export.max_retries": 3, "export.retry_delay_seconds": 3, "billing.max_retries": 3 }, null, 2) + "\n");
  files.set("docs/CONFIG.md", "# Settings\n\nExport retries: `export.max_retries` (see the export scheduler).\n");
  files.set("shopkit/config.py", ['"""Settings lookup shared by every module."""', "", "SETTINGS = {}", "", "", "def get_setting(key, default=None):", '    """Returns the configured value for a key, or the default when nothing is set."""', "    return SETTINGS.get(key, default)", ""].join("\n"));
  files.set("README.md", "# shopkit\n\nGenerated sample package with many settings.\n");
  return files;
}

/**
 * Builds the whole Python package in memory.
 * @param seed - the fixed seed
 * @param options - { solved } when true the target default is changed to 5
 */
export function buildRepo(seed, options = {}) {
  const rng = mulberry32(seed);
  const files = new Map();
  const usedKeys = new Set(["export.max_retries", "billing.max_retries", "exports.max_retries", "export.max_retries_per_batch", "export.retry_delay_seconds", "export.alert_after_failures"]);
  for (const pkg of PACKAGES) {
    files.set(`shopkit/${pkg}/__init__.py`, "");
    const pairs = [];
    for (const adjective of ADJECTIVES) for (const noun of NOUNS) pairs.push(`${adjective}_${noun}`);
    for (const name of shuffled(rng, pairs).slice(0, 16)) {
      files.set(`shopkit/${pkg}/${name}.py`, buildSettingsModule(pkg, name, rng, usedKeys));
    }
  }
  files.set("shopkit/__init__.py", "");
  files.set("shopkit/exports_legacy/__init__.py", "");
  files.set(TARGET_FILE, buildSchedulerModule(Boolean(options.solved)));
  for (const [file, text] of buildDecoyModules()) files.set(file, text);
  for (const [file, text] of buildSupportFiles()) files.set(file, text);
  return { files };
}
