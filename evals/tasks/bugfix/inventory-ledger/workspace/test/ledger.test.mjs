import test from "node:test";
import assert from "node:assert/strict";
import { createLedger, lowStock, totalUnits } from "../src/index.mjs";

test("receive and ship update the balance", () => {
  const ledger = createLedger();
  ledger.receive("AB-12", 10);
  ledger.ship("AB-12", 4);
  assert.equal(ledger.balance("AB-12"), 6);
});

test("shipping more than on hand throws", () => {
  const ledger = createLedger();
  ledger.receive("AB-12", 3);
  assert.throws(() => ledger.ship("AB-12", 4), /insufficient stock/);
});

test("shipping everything that is on hand leaves zero", () => {
  const ledger = createLedger();
  ledger.receive("AB-12", 5);
  ledger.ship("AB-12", 5);
  assert.equal(ledger.balance("AB-12"), 0);
});

test("shipping an unknown sku throws", () => {
  assert.throws(() => createLedger().ship("ZZ-99", 1), /insufficient stock/);
});

test("lowStock and totalUnits", () => {
  const ledger = createLedger();
  ledger.receive("B-1", 2);
  ledger.receive("A-1", 20);
  assert.deepEqual(lowStock(ledger, 5), ["B-1"]);
  assert.equal(totalUnits(ledger), 22);
});
