import test from "node:test";
import assert from "node:assert/strict";
import { load, NAMES } from "./_load.mjs";

test("src/orderkit.mjs exports exactly the original 49 names", async () => {
  const entry = await load("src/orderkit.mjs");
  const expected = Object.values(NAMES).flat().sort();
  assert.equal(expected.length, 49);
  assert.deepEqual(Object.keys(entry).sort(), expected);
});

test("every name exported by src/orderkit.mjs is the same value as the one in its new module", async () => {
  const entry = await load("src/orderkit.mjs");
  for (const [module, names] of Object.entries(NAMES)) {
    const loaded = await load(`src/${module}.mjs`);
    for (const name of names) assert.strictEqual(entry[name], loaded[name], `${name} differs from src/${module}.mjs`);
  }
});

test("a setting changed through src/orderkit.mjs is seen by every new module", async () => {
  const entry = await load("src/orderkit.mjs");
  const money = await load("src/money.mjs");
  const pricing = await load("src/pricing.mjs");
  entry.configure({ currency: "JPY" });
  assert.equal(money.currencyInfo().code, "JPY");
  assert.equal(pricing.shippingCost(99, "GB"), 6);
  entry.resetAll();
  assert.equal(money.getSettings().currency, "USD");
});
