import test from "node:test";
import assert from "node:assert/strict";
import { load, NAMES } from "./_load.mjs";

const address = { name: "Ada", street: "1 Main St", city: "Town", region: "US-CA", postal: "90001" };
const order = { email: "a@b.co", lines: [{ sku: "ABC-1234", qty: 2, unitPrice: 10 }], address };

test("src/pricing.mjs exports its public API", async () => {
  const pricing = await load("src/pricing.mjs");
  for (const name of NAMES.pricing) assert.ok(name in pricing, `missing ${name}`);
  assert.equal(typeof pricing.TAX_RATES, "object");
});

test("src/pricing.mjs prices an order and counts coupon uses", async () => {
  const pricing = await load("src/pricing.mjs");
  pricing.resetAll();
  assert.equal(pricing.priceOrder(order).total, 2744);
  pricing.registerCoupon("one", { type: "fixed", value: 1, maxUses: 1 });
  assert.equal(pricing.checkoutOrder({ ...order, couponCode: "ONE" }).discount, 100);
  assert.throws(() => pricing.checkoutOrder({ ...order, couponCode: "one" }), /used up/);
  pricing.resetAll();
  assert.equal(pricing.getCoupon("one"), undefined);
});

test("src/pricing.mjs resetAll resets the settings, the tax cache and the coupons", async () => {
  const pricing = await load("src/pricing.mjs");
  const money = await load("src/money.mjs");
  money.configure({ currency: "GBP" });
  pricing.taxRateFor("GB");
  pricing.registerCoupon("A", { type: "fixed", value: 1 });
  assert.equal(pricing.taxCacheSize(), 1);
  pricing.resetAll();
  assert.equal(money.getSettings().currency, "USD");
  assert.equal(pricing.taxCacheSize(), 0);
  assert.equal(pricing.getCoupon("A"), undefined);
});

test("src/pricing.mjs uses the currency decimals from the shared settings in src/money.mjs", async () => {
  const pricing = await load("src/pricing.mjs");
  const money = await load("src/money.mjs");
  money.configure({ currency: "JPY" });
  assert.equal(pricing.shippingCost(99, "GB"), 6);
  money.resetSettings();
  assert.equal(pricing.shippingCost(99, "GB"), 599);
});
