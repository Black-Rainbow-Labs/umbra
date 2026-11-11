// Reference solution: appends every STOCK line and writes reconcile.json.

import fs from "node:fs";
import path from "node:path";
import { buildWeeks, SUPPLIER } from "../fixture.mjs";

/**
 * Appends the STOCK lines and writes the reconcile file.
 * @param workspaceDir - the prepared workspace
 * @param options - { seed }
 */
export async function solve(workspaceDir, { seed }) {
  const { opening, reorder, weeks, finalStock, weeksBelow } = buildWeeks(seed);
  for (const week of weeks) fs.appendFileSync(path.join(workspaceDir, week.path), `STOCK: ${week.closing}\n`);
  const report = { opening_stock: opening, reorder_point: reorder, supplier: SUPPLIER, final_stock: finalStock, weeks_below_reorder: weeksBelow };
  fs.writeFileSync(path.join(workspaceDir, "reconcile.json"), JSON.stringify(report, null, 2));
}
