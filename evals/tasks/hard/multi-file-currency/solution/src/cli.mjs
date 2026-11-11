import fs from "node:fs";
import { parseInventory } from "./parser.mjs";
import { validateItem } from "./validator.mjs";
import { toCsv, toJson } from "./serializer.mjs";
import { renderReport } from "./report.mjs";
import { renderSummary } from "./summary.mjs";
import { SUPPORTED_CURRENCIES, DEFAULT_CURRENCY } from "./money.mjs";

const FORMATS = ["csv", "json", "report", "summary"];
const USAGE = "usage: inventory <file> [--format csv|json|report|summary] [--currency CODE] [--total-in CODE]";

/**
 * Reads the value of an option written as "--name value" or "--name=value".
 * @param argv - the argument list
 * @param index - the index of the current argument
 * @param name - the option name including the leading dashes
 * @returns { matched, value, consumed } where consumed is how many arguments were used
 */
function readOption(argv, index, name) {
  const arg = argv[index];
  if (arg === name) return { matched: true, value: argv[index + 1], consumed: 2 };
  if (arg.startsWith(`${name}=`)) return { matched: true, value: arg.slice(name.length + 1), consumed: 1 };
  return { matched: false };
}

/**
 * Parses command line arguments into { file, format, currency, totalIn } or { error }.
 * @param argv - the argument list without the node and script entries
 */
export function parseArgs(argv) {
  const options = { file: null, format: "report", currency: DEFAULT_CURRENCY, totalIn: null };
  for (let i = 0; i < argv.length; ) {
    const format = readOption(argv, i, "--format");
    const currency = readOption(argv, i, "--currency");
    const totalIn = readOption(argv, i, "--total-in");
    if (format.matched) {
      if (format.value === undefined) return { error: "missing value for --format" };
      options.format = format.value;
      i += format.consumed;
    } else if (currency.matched) {
      if (currency.value === undefined) return { error: "missing value for --currency" };
      options.currency = currency.value.trim().toUpperCase();
      i += currency.consumed;
    } else if (totalIn.matched) {
      if (totalIn.value === undefined) return { error: "missing value for --total-in" };
      options.totalIn = totalIn.value.trim().toUpperCase();
      i += totalIn.consumed;
    } else if (argv[i].startsWith("--")) return { error: `unknown option: ${argv[i]}` };
    else { options.file = argv[i]; i += 1; }
  }
  if (!options.file) return { error: USAGE };
  if (!FORMATS.includes(options.format)) return { error: `unknown format: ${options.format}` };
  if (!SUPPORTED_CURRENCIES.includes(options.currency)) return { error: `unsupported currency: ${options.currency}` };
  if (options.totalIn !== null && !SUPPORTED_CURRENCIES.includes(options.totalIn)) return { error: `unsupported currency: ${options.totalIn}` };
  return options;
}

/**
 * Collects "line N: message" strings for every invalid row.
 * @param rows - the parsed rows
 */
function collectErrors(rows) {
  const errors = [];
  for (const { line, item } of rows) {
    for (const message of validateItem(item)) errors.push(`line ${line}: ${message}`);
  }
  return errors;
}

/**
 * Runs the command line tool. Exit code 0 on success, 1 when a row fails validation, 2 for
 * usage errors, an unreadable file or a malformed header.
 * @param argv - the argument list without the node and script entries
 * @returns { code, stdout, stderr }
 */
export function run(argv) {
  const options = parseArgs(argv);
  if (options.error) return { code: 2, stdout: "", stderr: `${options.error}\n` };
  let text;
  try { text = fs.readFileSync(options.file, "utf8"); }
  catch { return { code: 2, stdout: "", stderr: `cannot read file: ${options.file}\n` }; }
  let rows;
  try { rows = parseInventory(text, { defaultCurrency: options.currency }); }
  catch (error) { return { code: 2, stdout: "", stderr: `${error.message}\n` }; }
  const errors = collectErrors(rows);
  if (errors.length) return { code: 1, stdout: "", stderr: errors.join("\n") + "\n" };
  const items = rows.map((row) => row.item);
  const totalIn = options.totalIn ?? undefined;
  const writers = { csv: toCsv, json: toJson, report: (list) => renderReport(list, { totalIn }), summary: (list) => renderSummary(list, { totalIn }) };
  return { code: 0, stdout: writers[options.format](items), stderr: "" };
}
