import { createItem } from "./model.mjs";

const REQUIRED_COLUMNS = ["sku", "name", "qty", "price"];

/**
 * Splits one CSV line into fields. Double quotes wrap a field that contains commas, and a
 * doubled quote inside a quoted field is a literal quote.
 * @param line - one line of CSV text
 */
export function splitCsvLine(line) {
  const fields = [];
  let current = "";
  let quoted = false;
  for (let i = 0; i < line.length; i += 1) {
    const ch = line[i];
    if (quoted) {
      if (ch === '"' && line[i + 1] === '"') { current += '"'; i += 1; }
      else if (ch === '"') quoted = false;
      else current += ch;
    } else if (ch === '"') quoted = true;
    else if (ch === ",") { fields.push(current); current = ""; }
    else current += ch;
  }
  fields.push(current);
  return fields;
}

/**
 * Reads the header line and checks that every required column is present.
 * @param fields - the header fields
 */
function readHeader(fields) {
  const header = fields.map((name) => name.trim().toLowerCase());
  for (const column of REQUIRED_COLUMNS) {
    if (!header.includes(column)) throw new Error(`missing column: ${column}`);
  }
  return header;
}

/**
 * Parses inventory CSV text into rows. The first non blank line is the header; columns may be in
 * any order and unknown columns are ignored. Blank lines are skipped. Each row records the
 * 1 based line number of the file so errors can point at it.
 * @param text - the CSV text
 * @returns an array of { line, item }
 */
export function parseInventory(text) {
  const rows = [];
  let header = null;
  text.split(/\r?\n/).forEach((raw, index) => {
    if (raw.trim() === "") return;
    const fields = splitCsvLine(raw);
    if (!header) { header = readHeader(fields); return; }
    const record = {};
    header.forEach((name, i) => { record[name] = fields[i] ?? ""; });
    rows.push({ line: index + 1, item: createItem(record) });
  });
  if (!header) throw new Error("empty input");
  return rows;
}
