// Reference solution: appends every TOTAL line and writes summary.json.

import fs from "node:fs";
import path from "node:path";
import { buildOrders } from "../fixture.mjs";

/**
 * Appends the TOTAL lines and writes the summary.
 * @param workspaceDir - the prepared workspace
 * @param options - { seed }
 */
export async function solve(workspaceDir, { seed }) {
  const { rate, orders, grandTotal, largest } = buildOrders(seed);
  for (const order of orders) fs.appendFileSync(path.join(workspaceDir, order.path), `TOTAL: ${order.total}\n`);
  const summary = { tax_rate: rate, order_count: orders.length, grand_total: grandTotal, largest_order: largest[0] };
  fs.writeFileSync(path.join(workspaceDir, "summary.json"), JSON.stringify(summary, null, 2));
}
