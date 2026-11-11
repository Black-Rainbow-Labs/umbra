// Hidden checks of the places that call the tax functions and dueDate. WORKSPACE_DIR is the repository after the fix.

import test from "node:test";
import assert from "node:assert/strict";
import path from "node:path";
import { pathToFileURL } from "node:url";

const load = (relative) => import(pathToFileURL(path.join(process.env.WORKSPACE_DIR, relative)).href);

test("invoice rows round the tax of each line half up", async () => {
  const { invoiceRows, toCsv } = await load("src/csv.mjs");
  const rows = invoiceRows([{ qty: 1, unitCents: 1050 }, { qty: 3, unitCents: 350 }, { qty: 1, unitCents: 10 }], 500);
  assert.deepEqual(rows, [{ line: 1, total: 1050, tax: 53 }, { line: 2, total: 1050, tax: 53 }, { line: 3, total: 10, tax: 1 }]);
  assert.equal(toCsv(rows, ["line", "total", "tax"]), "line,total,tax\n1,1050,53\n2,1050,53\n3,10,1\n");
});

test("the command line summary rounds the tax half up", async () => {
  const fs = await import("node:fs");
  const os = await import("node:os");
  const { runCli } = await load("src/cli.mjs");
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "invoicer-hidden-"));
  try {
    const file = path.join(dir, "invoice.json");
    fs.writeFileSync(file, JSON.stringify({ items: [{ qty: 1, unitCents: 1050 }], bps: 500, currency: "EUR" }));
    assert.equal(runCli([file]), "Subtotal: €10.50\nTax: €0.53\nTotal: €11.03");
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
});

test("business day terms still skip weekends and the plain due date does not", async () => {
  const { businessDueDate } = await load("src/terms.mjs");
  const { dueDate } = await load("src/dates.mjs");
  assert.equal(businessDueDate("2024-02-02", 5), "2024-02-09");
  assert.equal(businessDueDate("2024-02-09", 1), "2024-02-12");
  assert.equal(businessDueDate("2024-03-01", 10), "2024-03-15");
  assert.equal(dueDate("2024-02-02", 5), "2024-02-07");
});

test("the tax rate table and regional summaries", async () => {
  const { summaryForRegion } = await load("src/invoice.mjs");
  assert.equal(summaryForRegion([{ qty: 1, unitCents: 1999 }], "CA"), "Subtotal: $19.99\nTax: $1.45\nTotal: $21.44");
  assert.equal(summaryForRegion([{ qty: 1, unitCents: 1999 }], "ZZ"), "Subtotal: $19.99\nTax: $0.00\nTotal: $19.99");
  const { computeTax, taxFor, rateFor } = await load("src/tax/index.mjs");
  assert.equal(computeTax, taxFor);
  assert.equal(rateFor("WA"), 650);
  assert.equal(taxFor(2575, 650), 167);
});
