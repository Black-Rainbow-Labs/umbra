import test from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { createItem, parseInventory, validateItem, toCsv, toJson, renderReport, renderSummary, run, convert, SUPPORTED_CURRENCIES, DEFAULT_CURRENCY, RATES_TO_USD } from "../src/index.mjs";

test("createItem trims text and converts numbers", () => {
  assert.deepEqual(createItem({ sku: " A-1 ", name: " Bolt ", qty: "5", price: "1.5" }), { sku: "A-1", name: "Bolt", qty: 5, price: 1.5, currency: "USD" });
  assert.equal(createItem({ sku: "A", name: "B", qty: 1, price: 1, currency: " eur " }).currency, "EUR");
});

test("parseInventory reads rows with line numbers", () => {
  const rows = parseInventory("sku,name,qty,price\nA-1,Bolt,5,1.50\n\nB-2,\"Nut, large\",2,0.25\n");
  assert.deepEqual(rows.map((r) => r.line), [2, 4]);
  assert.equal(rows[1].item.name, "Nut, large");
});

test("validateItem reports problems in order", () => {
  assert.deepEqual(validateItem({ sku: "", name: "", qty: -1, price: NaN }), ["sku is required", "name is required", "qty must be a non-negative integer", "price must be a non-negative number"]);
});

test("toCsv writes the header and two decimal prices", () => {
  assert.equal(toCsv([{ sku: "A-1", name: "Bolt", qty: 5, price: 1.5 }]), "sku,name,qty,price,currency\nA-1,Bolt,5,1.50,USD\n");
});

test("toJson keeps the key order", () => {
  assert.equal(toJson([{ sku: "A-1", name: "Bolt", qty: 5, price: 1.5 }]), '[\n  {\n    "sku": "A-1",\n    "name": "Bolt",\n    "qty": 5,\n    "price": 1.5,\n    "currency": "USD"\n  }\n]\n');
});

test("renderReport lists items and a total per currency", () => {
  assert.equal(renderReport([{ sku: "A-1", name: "Bolt", qty: 5, price: 1.5 }]), "Inventory report\nA-1 | Bolt | 5 x 1.50 USD = 7.50 USD\n\nTotal (USD): 7.50\nItems: 1\n");
});

test("the supported currencies and the default are exported", () => {
  assert.deepEqual(SUPPORTED_CURRENCIES, ["USD", "EUR", "GBP", "JPY", "CAD"]);
  assert.equal(DEFAULT_CURRENCY, "USD");
});

test("parseInventory reads a currency column in any position and applies the default", () => {
  const rows = parseInventory("Currency,sku,name,qty,price\neur,A-1,Bolt,2,1.50\n,B-2,Nut,3,0.25\n", { defaultCurrency: "gbp" });
  assert.deepEqual(rows.map((r) => r.item.currency), ["EUR", "GBP"]);
});

test("validateItem checks the currency and JPY prices", () => {
  assert.deepEqual(validateItem({ sku: "A-1", name: "Bolt", qty: 1, price: 2, currency: "eur" }), ["currency must be one of USD, EUR, GBP, JPY, CAD"]);
  assert.deepEqual(validateItem({ sku: "A-1", name: "Bolt", qty: 1, price: 2.5, currency: "JPY" }), ["price must be a whole number for JPY"]);
  assert.deepEqual(validateItem({ sku: "A-1", name: "Bolt", qty: 1, price: 2.5 }), []);
});

test("toCsv writes JPY prices without decimals", () => {
  assert.equal(toCsv([{ sku: "B-2", name: "Nut", qty: 2, price: 150, currency: "JPY" }]), "sku,name,qty,price,currency\nB-2,Nut,2,150,JPY\n");
});

test("renderReport writes one total per currency", () => {
  const items = [{ sku: "A-1", name: "Bolt", qty: 2, price: 1.5, currency: "EUR" }, { sku: "B-2", name: "Nut", qty: 2, price: 150, currency: "JPY" }];
  assert.equal(renderReport(items), "Inventory report\nA-1 | Bolt | 2 x 1.50 EUR = 3.00 EUR\nB-2 | Nut | 2 x 150 JPY = 300 JPY\n\nTotal (EUR): 3.00\nTotal (JPY): 300\nItems: 2\n");
});

test("convert goes through USD and rounds to the target decimals", () => {
  assert.equal(RATES_TO_USD.EUR, 1.08);
  assert.equal(convert(100, "EUR", "USD"), 108);
  assert.equal(convert(5000, "jpy", " EUR "), 31.02);
  assert.throws(() => convert(1, "xyz", "USD"), /^Error: unsupported currency: XYZ$/);
});

test("renderSummary lists totals, the item count and the grand total", () => {
  const items = [{ sku: "A-1", name: "Bolt", qty: 2, price: 1.5, currency: "EUR" }];
  assert.equal(renderSummary(items, { totalIn: "usd" }), JSON.stringify({ totals: { EUR: 3 }, items: 1, totalIn: { currency: "USD", amount: 3.24 } }, null, 2) + "\n");
});

test("the command line accepts --currency, --total-in and --format summary", () => {
  const file = path.join(fs.mkdtempSync(path.join(os.tmpdir(), "inventory-")), "in.csv");
  fs.writeFileSync(file, "sku,name,qty,price,currency\nA-1,Bolt,2,150,\nB-2,Nut,1,3,eur\n");
  const result = run([file, "--currency", "jpy", "--total-in=USD"]);
  assert.equal(result.code, 0);
  assert.match(result.stdout, /Total in USD: 5\.25\nItems: 2\n$/);
  assert.equal(run([file, "--total-in", "xyz"]).stderr, "unsupported currency: XYZ\n");
  assert.equal(JSON.parse(run([file, "--format", "summary", "--currency", "CAD"]).stdout).totals.CAD, 300);
});
