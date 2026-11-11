import test from "node:test";
import assert from "node:assert/strict";
import { createItem, parseInventory, validateItem, toCsv, toJson, renderReport } from "../src/index.mjs";

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
