import test from "node:test";
import assert from "node:assert/strict";
import { load, NAMES } from "./_load.mjs";

test("src/money.mjs exports its public API", async () => {
  const money = await load("src/money.mjs");
  for (const name of NAMES.money) assert.equal(typeof money[name], "function", `missing ${name}`);
});

test("src/money.mjs converts, rounds and formats", async () => {
  const money = await load("src/money.mjs");
  money.resetSettings();
  assert.equal(money.toCents("19.99"), 1999);
  assert.equal(money.multiplyMoney(25, 0.1), 3);
  assert.deepEqual(money.allocateCents(100, [1, 1, 1]), [34, 33, 33]);
  assert.equal(money.formatMoney(123450), "$1,234.50");
  assert.equal(money.parseMoney("$2.50"), 250);
  money.configure({ currency: "EUR", roundingMode: "half-even" });
  assert.equal(money.formatMoney(100), "€1.00");
  assert.equal(money.multiplyMoney(25, 0.1), 2);
  assert.deepEqual(money.getSettings(), { currency: "EUR", roundingMode: "half-even" });
  money.resetSettings();
  assert.deepEqual(money.getSettings(), { currency: "USD", roundingMode: "half-up" });
});
