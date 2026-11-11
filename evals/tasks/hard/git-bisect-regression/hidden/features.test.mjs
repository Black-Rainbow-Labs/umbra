// Hidden checks that features added after the broken commit still work. WORKSPACE_DIR is the repository after the fix.

import test from "node:test";
import assert from "node:assert/strict";
import path from "node:path";
import { pathToFileURL } from "node:url";

const load = (relative) => import(pathToFileURL(path.join(process.env.WORKSPACE_DIR, relative)).href);

test("roundCents still rounds toward zero", async () => {
  const { roundCents } = await load("src/money.mjs");
  assert.equal(roundCents(2.9), 2);
  assert.equal(roundCents(2.5), 2);
  assert.equal(roundCents(-2.5), -2);
  assert.equal(roundCents(-2.9), -2);
});

test("refunds still never refund a fraction of a cent", async () => {
  const { refundCents } = await load("src/refunds.mjs");
  assert.equal(refundCents(999, 50), 499);
  assert.equal(refundCents(1001, 50), 500);
  assert.equal(refundCents(1, 50), 0);
  assert.equal(refundCents(333, 10), 33);
});

test("discount cap, zero quantity lines, regions and the due date helper still work", async () => {
  const { applyPercent } = await load("src/discounts.mjs");
  assert.equal(applyPercent(1000, 150), 0);
  assert.equal(applyPercent(999, 10), 899);
  const { lineTotal, summaryForRegion } = await load("src/invoice.mjs");
  assert.equal(lineTotal({ qty: -1, unitCents: 500 }), 0);
  assert.equal(summaryForRegion([{ qty: 2, unitCents: 1000 }, { qty: 1, unitCents: 500 }], "NY"), "Subtotal: $25.00\nTax: $1.00\nTotal: $26.00");
  const { dueDate } = await load("src/dates.mjs");
  assert.equal(dueDate("2024-01-30", 30), "2024-02-29");
  const { rateFor, RATES } = await load("src/tax/index.mjs");
  assert.equal(rateFor("TX"), 625);
  assert.equal(RATES.WA, 650);
});

test("money helpers and formatting still work", async () => {
  const { toCents, fromCents } = await load("src/money.mjs");
  assert.equal(toCents(19.99), 1999);
  assert.equal(fromCents(1999), 19.99);
  const { formatMoney } = await load("src/format/index.mjs");
  assert.equal(formatMoney(-250), "-$2.50");
  assert.equal(formatMoney(1234, "GBP"), "£12.34");
});
