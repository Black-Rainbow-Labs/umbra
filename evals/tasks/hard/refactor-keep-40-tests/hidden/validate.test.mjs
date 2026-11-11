import test from "node:test";
import assert from "node:assert/strict";
import { load, NAMES } from "./_load.mjs";

const address = { name: "Ada", street: "1 Main St", city: "Town", region: "DE", postal: "10115" };

test("src/validate.mjs exports its public API", async () => {
  const validate = await load("src/validate.mjs");
  for (const name of NAMES.validate) assert.equal(typeof validate[name], "function", `missing ${name}`);
});

test("src/validate.mjs validates orders and throws ValidationError", async () => {
  const validate = await load("src/validate.mjs");
  const good = { email: "a@b.co", lines: [{ sku: "ABC-1234", qty: 1, unitPrice: 2 }], address };
  assert.deepEqual(validate.validateOrder(good), { valid: true, errors: [] });
  assert.throws(() => validate.assertValidOrder({ ...good, email: "x" }), (error) => error instanceof validate.ValidationError && error.errors.length === 1);
  assert.equal(validate.normalizeOrder({ ...good, email: " A@B.CO " }).email, "a@b.co");
  assert.equal(validate.partitionOrders([good, { ...good, email: "x" }]).invalid.length, 1);
});

test("src/validate.mjs reads the currency from the shared settings in src/money.mjs", async () => {
  const validate = await load("src/validate.mjs");
  const money = await load("src/money.mjs");
  money.resetSettings();
  const order = { email: "a@b.co", currency: "EUR", lines: [{ sku: "ABC-1234", qty: 1, unitPrice: 2 }], address };
  assert.equal(validate.validateOrder(order).valid, false);
  money.configure({ currency: "EUR" });
  assert.equal(validate.validateOrder(order).valid, true);
  money.resetSettings();
});
