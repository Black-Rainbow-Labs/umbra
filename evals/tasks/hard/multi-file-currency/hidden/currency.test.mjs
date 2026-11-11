import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";

const workspace = process.env.WORKSPACE_DIR;
const lib = await import(pathToFileURL(path.join(workspace, "src", "index.mjs")).href);
const { createItem, parseInventory, validateItem, toCsv, toJson, renderReport, run, SUPPORTED_CURRENCIES, DEFAULT_CURRENCY } = lib;
const scratch = fs.mkdtempSync(path.join(os.tmpdir(), "currency-hidden-"));

/**
 * Writes CSV text to a scratch file and returns its path.
 * @param text - the file content
 */
function writeCsv(text) {
  const file = path.join(scratch, `f${fs.readdirSync(scratch).length}.csv`);
  fs.writeFileSync(file, text);
  return file;
}

test("constants: SUPPORTED_CURRENCIES and DEFAULT_CURRENCY are exported", () => {
  assert.deepEqual(SUPPORTED_CURRENCIES, ["USD", "EUR", "GBP", "JPY", "CAD"]);
  assert.equal(DEFAULT_CURRENCY, "USD");
});

test("model: createItem defaults a missing, null or blank currency to USD", () => {
  const base = { sku: "A-1", name: "Bolt", qty: "2", price: "1.5" };
  for (const currency of [undefined, null, "", "   "]) {
    assert.deepEqual(createItem({ ...base, currency }), { sku: "A-1", name: "Bolt", qty: 2, price: 1.5, currency: "USD" });
  }
  assert.equal(createItem(base).currency, "USD");
});

test("model: createItem trims and uppercases, and takes a default currency argument", () => {
  const base = { sku: "A-1", name: "Bolt", qty: 2, price: 1 };
  assert.equal(createItem({ ...base, currency: " eur " }).currency, "EUR");
  assert.equal(createItem({ ...base, currency: "" }, "GBP").currency, "GBP");
  assert.equal(createItem({ ...base, currency: "jpy" }, "GBP").currency, "JPY");
});

test("parser: currency column in any position, uppercased, blank becomes USD", () => {
  const rows = parseInventory("Currency,sku,name,qty,price\neur,A-1,Bolt,2,1.50\n,B-2,Nut,3,0.25\n gbp ,C-3,Cap,1,9.00\n");
  assert.deepEqual(rows.map((r) => r.item.currency), ["EUR", "USD", "GBP"]);
  assert.deepEqual(rows.map((r) => r.line), [2, 3, 4]);
  assert.equal(rows[0].item.price, 1.5);
});

test("parser: a file without a currency column gives USD items", () => {
  const rows = parseInventory("sku,name,qty,price\nA-1,\"Nut, large\",2,0.25\n");
  assert.equal(rows[0].item.currency, "USD");
  assert.equal(rows[0].item.name, "Nut, large");
});

test("parser: unknown currency codes are kept uppercased and not rejected", () => {
  const rows = parseInventory("sku,name,qty,price,currency\nA-1,Bolt,2,1.5,xyz\n");
  assert.equal(rows[0].item.currency, "XYZ");
});

test("parser: defaultCurrency option applies only to blank or missing currencies", () => {
  const withColumn = parseInventory("sku,name,qty,price,currency\nA-1,Bolt,2,1,\nB-2,Nut,2,1,eur\n", { defaultCurrency: "CAD" });
  assert.deepEqual(withColumn.map((r) => r.item.currency), ["CAD", "EUR"]);
  const withoutColumn = parseInventory("sku,name,qty,price\nA-1,Bolt,2,1\n", { defaultCurrency: "GBP" });
  assert.equal(withoutColumn[0].item.currency, "GBP");
});

test("validator: supported currencies are valid and a missing currency counts as USD", () => {
  for (const currency of SUPPORTED_CURRENCIES) {
    assert.deepEqual(validateItem({ sku: "A-1", name: "Bolt", qty: 1, price: 2, currency }), []);
  }
  assert.deepEqual(validateItem({ sku: "A-1", name: "Bolt", qty: 1, price: 2.5 }), []);
});

test("validator: unsupported or lowercase currency gives the exact message", () => {
  const message = "currency must be one of USD, EUR, GBP, JPY, CAD";
  assert.deepEqual(validateItem({ sku: "A-1", name: "Bolt", qty: 1, price: 2, currency: "XYZ" }), [message]);
  assert.deepEqual(validateItem({ sku: "A-1", name: "Bolt", qty: 1, price: 2, currency: "eur" }), [message]);
  assert.deepEqual(validateItem({ sku: "A-1", name: "Bolt", qty: 1, price: 2, currency: "" }), [message]);
});

test("validator: JPY prices must be whole numbers and the error comes after the price checks", () => {
  assert.deepEqual(validateItem({ sku: "A-1", name: "Bolt", qty: 1, price: 150, currency: "JPY" }), []);
  assert.deepEqual(validateItem({ sku: "A-1", name: "Bolt", qty: 1, price: 150.5, currency: "JPY" }), ["price must be a whole number for JPY"]);
  assert.deepEqual(validateItem({ sku: "A-1", name: "Bolt", qty: 1, price: -1.5, currency: "JPY" }), ["price must be a non-negative number"]);
  assert.deepEqual(validateItem({ sku: "", name: "Bolt", qty: 1, price: 1.5, currency: "JPY" }), ["sku is required", "price must be a whole number for JPY"]);
  assert.deepEqual(validateItem({ sku: "", name: "Bolt", qty: 1, price: 1.5, currency: "XYZ" }), ["sku is required", "currency must be one of USD, EUR, GBP, JPY, CAD"]);
});

test("serializer: toCsv writes currency last and prices with the currency's decimals", () => {
  const items = [
    { sku: "A-1", name: "Bolt, steel", qty: 5, price: 1.5, currency: "EUR" },
    { sku: "B-2", name: "Nut", qty: 2, price: 150, currency: "JPY" },
    { sku: "C-3", name: "Cap", qty: 1, price: 2 },
  ];
  assert.equal(toCsv(items), 'sku,name,qty,price,currency\nA-1,"Bolt, steel",5,1.50,EUR\nB-2,Nut,2,150,JPY\nC-3,Cap,1,2.00,USD\n');
  assert.equal(toCsv([]), "sku,name,qty,price,currency\n");
});

test("serializer: toJson keeps key order with currency last and a numeric price", () => {
  const out = toJson([{ sku: "B-2", name: "Nut", qty: 2, price: 150, currency: "JPY" }, { sku: "C-3", name: "Cap", qty: 1, price: 2.5 }]);
  assert.equal(out, JSON.stringify([{ sku: "B-2", name: "Nut", qty: 2, price: 150, currency: "JPY" }, { sku: "C-3", name: "Cap", qty: 1, price: 2.5, currency: "USD" }], null, 2) + "\n");
  assert.equal(typeof JSON.parse(out)[0].price, "number");
});

test("report: item lines carry the currency code and totals are per currency, sorted by code", () => {
  const items = [
    { sku: "A-1", name: "Bolt", qty: 3, price: 19.99, currency: "USD" },
    { sku: "B-2", name: "Nut", qty: 2, price: 150, currency: "JPY" },
    { sku: "C-3", name: "Cap", qty: 4, price: 2.5, currency: "EUR" },
    { sku: "D-4", name: "Pin", qty: 1, price: 0.1, currency: "USD" },
    { sku: "E-5", name: "Gear", qty: 1, price: 7 },
  ];
  const expected = [
    "Inventory report",
    "A-1 | Bolt | 3 x 19.99 USD = 59.97 USD",
    "B-2 | Nut | 2 x 150 JPY = 300 JPY",
    "C-3 | Cap | 4 x 2.50 EUR = 10.00 EUR",
    "D-4 | Pin | 1 x 0.10 USD = 0.10 USD",
    "E-5 | Gear | 1 x 7.00 USD = 7.00 USD",
    "",
    "Total (EUR): 10.00",
    "Total (JPY): 300",
    "Total (USD): 67.07",
    "Items: 5",
    "",
  ].join("\n");
  assert.equal(renderReport(items), expected);
});

test("report: an empty list has no total lines", () => {
  assert.equal(renderReport([]), "Inventory report\n\nItems: 0\n");
});

test("cli: report output for a mixed currency file", () => {
  const file = writeCsv("sku,name,qty,price,currency\nA-1,Bolt,3,19.99,usd\nB-2,Nut,2,150,JPY\nC-3,Cap,4,2.5,\n");
  const result = run([file]);
  assert.equal(result.code, 0);
  assert.equal(result.stderr, "");
  assert.equal(result.stdout, "Inventory report\nA-1 | Bolt | 3 x 19.99 USD = 59.97 USD\nB-2 | Nut | 2 x 150 JPY = 300 JPY\nC-3 | Cap | 4 x 2.50 USD = 10.00 USD\n\nTotal (JPY): 300\nTotal (USD): 69.97\nItems: 3\n");
});

test("cli: --format csv and json include the currency", () => {
  const file = writeCsv("sku,name,qty,price,currency\nA-1,Bolt,3,19.99,gbp\nB-2,Nut,2,150,\n");
  const csv = run([file, "--format", "csv"]);
  assert.equal(csv.code, 0);
  assert.equal(csv.stdout, "sku,name,qty,price,currency\nA-1,Bolt,3,19.99,GBP\nB-2,Nut,2,150.00,USD\n");
  const json = run(["--format=json", file]);
  assert.equal(json.code, 0);
  assert.deepEqual(JSON.parse(json.stdout).map((i) => i.currency), ["GBP", "USD"]);
});

test("cli: --currency sets the default for blank rows only, in both option spellings", () => {
  const file = writeCsv("sku,name,qty,price,currency\nA-1,Bolt,1,2,\nB-2,Nut,1,3,gbp\n");
  for (const args of [["--currency", "eur"], ["--currency=EUR"]]) {
    const result = run([file, "--format", "json", ...args]);
    assert.equal(result.code, 0);
    assert.deepEqual(JSON.parse(result.stdout).map((i) => i.currency), ["EUR", "GBP"]);
  }
  const noColumn = writeCsv("sku,name,qty,price\nA-1,Bolt,1,2\n");
  assert.deepEqual(JSON.parse(run([noColumn, "--currency", "CAD", "--format", "json"]).stdout).map((i) => i.currency), ["CAD"]);
});

test("cli: --currency errors", () => {
  const file = writeCsv("sku,name,qty,price\nA-1,Bolt,1,2\n");
  const bad = run([file, "--currency", "xyz"]);
  assert.equal(bad.code, 2);
  assert.equal(bad.stderr, "unsupported currency: XYZ\n");
  assert.equal(bad.stdout, "");
  const missing = run([file, "--currency"]);
  assert.equal(missing.code, 2);
  assert.equal(missing.stderr, "missing value for --currency\n");
});

test("cli: validation errors use file line numbers and exit 1", () => {
  const file = writeCsv("sku,name,qty,price,currency\nA-1,Bolt,1,2,XYZ\n\nB-2,Nut,1,2.5,\nC-3,Cap,1,3,jpy\n");
  const result = run([file, "--currency", "JPY"]);
  assert.equal(result.code, 1);
  assert.equal(result.stdout, "");
  assert.equal(result.stderr, "line 2: currency must be one of USD, EUR, GBP, JPY, CAD\nline 4: price must be a whole number for JPY\n");
});

test("cli: csv output parses back to the same items", () => {
  const file = writeCsv("sku,name,qty,price,currency\nA-1,\"Bolt, \"\"M6\"\"\",3,19.99,eur\nB-2,Nut,2,150,jpy\n");
  const first = run([file, "--format", "csv"]);
  assert.equal(first.code, 0);
  const again = parseInventory(first.stdout).map((r) => r.item);
  assert.deepEqual(again, parseInventory(fs.readFileSync(file, "utf8")).map((r) => r.item));
});

test("cli: existing usage and header errors are unchanged", () => {
  assert.equal(run([]).code, 2);
  assert.match(run([]).stderr, /^usage: /);
  const file = writeCsv("sku,name,price\nA-1,Bolt,2\n");
  const result = run([file]);
  assert.equal(result.code, 2);
  assert.equal(result.stderr, "missing column: qty\n");
  assert.equal(run([path.join(scratch, "nope.csv")]).code, 2);
});

test("defaults: DEFAULT_CURRENCY is what blank rows get without --currency", () => {
  const file = writeCsv("sku,name,qty,price\nA-1,Bolt,1,2\n");
  assert.deepEqual(JSON.parse(run([file, "--format", "json"]).stdout).map((i) => i.currency), [DEFAULT_CURRENCY]);
});
