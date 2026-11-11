// Reference solution: appends the true closing balance to every day.

import fs from "node:fs";
import path from "node:path";
import { buildDays } from "../fixture.mjs";

/**
 * Appends BALANCE lines with the true closing balances.
 * @param workspaceDir - the prepared workspace
 * @param options - { seed }
 */
export async function solve(workspaceDir, { seed }) {
  for (const day of buildDays(seed).days) fs.appendFileSync(path.join(workspaceDir, day.path), `BALANCE: ${day.closing}\n`);
}
