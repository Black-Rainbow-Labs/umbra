// Hidden tax behavior checks. WORKSPACE_DIR is the repository after the fix.

import test from "node:test";
import assert from "node:assert/strict";
import path from "node:path";
import { pathToFileURL } from "node:url";

const load = (relative) => import(pathToFileURL(path.join(process.env.WORKSPACE_DIR, relative)).href);

test("computeTax and taxFor round half up", async () => {
  const { computeTax, taxFor } = await load("src/tax/index.mjs");
  assert.equal(computeTax(1050, 500), 53);
  assert.equal(taxFor(10, 500), 1);
  assert.equal(taxFor(1999, 825), 165);
  assert.equal(taxFor(1, 5000), 1);
  assert.equal(taxFor(3333, 3333), 1111);
  assert.equal(taxFor(0, 500), 0);
  assert.equal(taxFor(1000, 0), 0);
  assert.equal(taxFor(99, 100), 1);
  assert.equal(taxFor(49, 100), 0);
});

test("invoice totals include the rounded tax of each line", async () => {
  const { invoiceTotal, summary } = await load("src/invoice.mjs");
  assert.equal(invoiceTotal([{ qty: 1, unitCents: 1050 }], 500), 1103);
  assert.equal(invoiceTotal([{ qty: 3, unitCents: 350 }, { qty: 1, unitCents: 10 }], 500), 1050 + 53 + 10 + 1);
  assert.equal(summary([{ qty: 1, unitCents: 1050 }], 500), "Subtotal: $10.50\nTax: $0.53\nTotal: $11.03");
});

test("regional summaries round the tax half up", async () => {
  const { summaryForRegion } = await load("src/invoice.mjs");
  assert.equal(summaryForRegion([{ qty: 1, unitCents: 1999 }], "CA"), "Subtotal: $19.99\nTax: $1.45\nTotal: $21.44");
  assert.equal(summaryForRegion([{ qty: 1, unitCents: 1999 }], "NY", "EUR"), "Subtotal: €19.99\nTax: €0.80\nTotal: €20.79");
});
