import test from "node:test";
import assert from "node:assert/strict";
import path from "node:path";
import { pathToFileURL } from "node:url";

const { createLedger, lowStock, totalUnits } = await import(pathToFileURL(path.join(process.env.WORKSPACE_DIR, "src", "index.mjs")).href);

test("shipping exactly the stock on hand works and returns zero", () => {
  const ledger = createLedger();
  ledger.receive("AB-12", 7);
  assert.equal(ledger.ship("AB-12", 7), 0);
  assert.equal(ledger.balance("AB-12"), 0);
});

test("shipping one more than on hand still throws and changes nothing", () => {
  const ledger = createLedger();
  ledger.receive("AB-12", 7);
  assert.throws(() => ledger.ship("AB-12", 8), /insufficient stock/);
  assert.equal(ledger.balance("AB-12"), 7);
});

test("balance finds a sku typed in lower case with spaces", () => {
  const ledger = createLedger();
  ledger.receive("AB-12", 4);
  assert.equal(ledger.balance("ab-12"), 4);
  assert.equal(ledger.balance("  Ab-12 "), 4);
});

test("mixed case receives add to the same balance", () => {
  const ledger = createLedger();
  ledger.receive("ab-12", 2);
  ledger.receive(" AB-12", 3);
  assert.equal(ledger.balance("AB-12"), 5);
  assert.deepEqual(ledger.snapshot(), { "AB-12": 5 });
});

test("ship with a lower case sku drains the stock to zero", () => {
  const ledger = createLedger();
  ledger.receive("AB-12", 3);
  ledger.ship("ab-12", 3);
  assert.equal(ledger.balance("ab-12"), 0);
});

test("unknown sku balance is zero and a bad sku throws", () => {
  const ledger = createLedger();
  assert.equal(ledger.balance("NOPE-1"), 0);
  assert.throws(() => ledger.balance("   "));
});

test("quantities must be positive integers", () => {
  const ledger = createLedger();
  assert.throws(() => ledger.receive("A-1", 0));
  assert.throws(() => ledger.receive("A-1", 1.5));
  ledger.receive("A-1", 2);
  assert.throws(() => ledger.ship("A-1", -1));
});

test("reports include items shipped down to zero", () => {
  const ledger = createLedger();
  ledger.receive("A-1", 4);
  ledger.receive("B-1", 9);
  ledger.ship("a-1", 4);
  assert.deepEqual(lowStock(ledger, 0), ["A-1"]);
  assert.equal(totalUnits(ledger), 9);
});
