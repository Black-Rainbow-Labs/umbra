// Builds the report row formatter module: many near identical functions that differ in small details.

export const ENTITY_NAMES = ["Account", "Invoice", "Order", "Shipment", "Customer", "Supplier", "Payment", "Refund", "Ticket", "Project", "Task", "Release", "Contract", "Quote", "Subscription", "Warehouse", "Carrier", "Pallet", "Parcel", "Return", "Credit", "Debit", "Ledger", "Budget", "Expense", "Vendor", "Employee", "Shift", "Asset", "Device", "License", "Booking", "Reservation", "Voucher", "Coupon", "Campaign", "Lead", "Contact", "Meeting", "Report", "Audit", "Policy", "Claim", "Batch", "Region", "Territory", "Rebate", "Deposit", "Grant", "Tariff"];
export const TARGET_ENTITY = "Shipment";

/**
 * Returns a deterministic pseudo random number generator.
 * @param seed - integer seed
 */
export function mulberry32(seed) {
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
 * Picks one element of a list using the generator.
 * @param random - the generator
 * @param list - the candidates
 */
function pick(random, list) {
  return list[Math.floor(random() * list.length)];
}

/**
 * Builds the source text of one formatter function for an entity.
 * @param entity - entity name such as Invoice
 * @param random - the generator that decides the small differences
 */
function buildFormatter(entity, random) {
  const width = pick(random, [4, 5, 6, 8]);
  const nameLength = pick(random, [16, 20, 24, 32]);
  const decimals = pick(random, [0, 1, 2, 2, 2]);
  const symbol = pick(random, ["$", "EUR ", "GBP ", "USD "]);
  const separator = pick(random, [" | ", " ; ", "\\t", " / "]);
  const lateLabel = pick(random, ["late", "overdue", "delayed"]);
  const lines = [
    "/**",
    ` * Formats one ${entity} record as a single report line.`,
    ` * @param row - the ${entity} record`,
    " */",
    `export function format${entity}Row(row) {`,
    `  const id = String(row.id).padStart(${width}, "0");`,
    `  const name = String(row.name).trim().slice(0, ${nameLength});`,
    `  const amount = (Number(row.amount) / 100).toFixed(${decimals});`,
    `  const weight = Math.round(Number(row.weight)) + " g";`,
    "  const status = String(row.status).toUpperCase();",
    "  const date = String(row.date).slice(0, 10);",
    "  const flags = [];",
    `  if (Number(row.amount) < 0) flags.push("refund");`,
    `  if (row.status === "late") flags.push("${lateLabel}");`,
    `  if (row.priority) flags.push("p" + row.priority);`,
    "  if (row.tags && row.tags.length > 0) flags.push(row.tags.length + \" tags\");",
    "  const owner = row.owner ? String(row.owner).toLowerCase() : \"nobody\";",
    "  const region = row.region ? String(row.region).toUpperCase() : \"--\";",
    "  const note = row.note ? String(row.note).replace(/\\s+/g, \" \").slice(0, 12) : \"\";",
    "  const columns = [];",
    "  columns.push(id);",
    "  columns.push(name);",
    `  columns.push(\`${symbol}\${amount}\`);`,
    "  columns.push(weight);",
    "  columns.push(status);",
    "  columns.push(date);",
    "  columns.push(owner);",
    "  columns.push(region);",
    "  columns.push(flags.join(\"+\"));",
    "  columns.push(note);",
    `  return columns.join("${separator}");`,
    "}",
    "",
  ];
  return lines.join("\n");
}

/**
 * Builds the whole rows module text.
 * @param seed - integer seed
 */
export function buildRowsModule(seed) {
  const random = mulberry32(seed);
  const parts = ["// Report row formatters. One function per record type.\n"];
  for (const entity of ENTITY_NAMES) parts.push(buildFormatter(entity, random));
  return parts.join("\n");
}

/**
 * Returns the character range [start, end) of the target formatter including its comment.
 * @param text - the original module text
 */
export function targetRange(text) {
  const start = text.indexOf(`/**\n * Formats one ${TARGET_ENTITY} record`);
  const marker = `export function format${TARGET_ENTITY}Row(row) {`;
  const close = text.indexOf("\n}\n", text.indexOf(marker)) + 3;
  return [start, close];
}
