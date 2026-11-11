import test from "node:test";
import assert from "node:assert/strict";
import { load, NAMES } from "./_load.mjs";

const address = { name: "Ada", street: "1 Main St", city: "Town", region: "US-CA", postal: "90001" };
const order = { email: "a@b.co", lines: [{ sku: "ABC-1234", qty: 2, unitPrice: 10 }], address };

test("src/report.mjs exports its public API", async () => {
  const report = await load("src/report.mjs");
  for (const name of NAMES.report) assert.equal(typeof report[name], "function", `missing ${name}`);
});

test("src/report.mjs builds receipts, summaries and tables", async () => {
  const report = await load("src/report.mjs");
  const pricing = await load("src/pricing.mjs");
  pricing.resetAll();
  const receipt = report.formatReceipt(pricing.priceOrder(order));
  assert.match(receipt, /Total: \$27\.44$/);
  assert.deepEqual(report.summarizeOrders([order]), { orders: 1, units: 2, revenue: 2744, tax: 145, average: 2744 });
  assert.equal(report.topSkus([order])[0].sku, "ABC-1234");
  assert.equal(report.groupByRegion([order, order])["US-CA"].orders, 2);
  assert.equal(report.renderTable([{ a: 1 }], [{ key: "a", title: "A" }]), "A\n-\n1");
});

test("src/report.mjs formats with the currency set through src/money.mjs", async () => {
  const report = await load("src/report.mjs");
  const pricing = await load("src/pricing.mjs");
  const money = await load("src/money.mjs");
  money.configure({ currency: "EUR" });
  assert.match(report.formatReceipt(pricing.priceOrder(order)), /Total: €27\.44$/);
  assert.match(report.dailyReport([order]), /Revenue: €27\.44/);
  pricing.resetAll();
});
