// Reference solution: writes report.json from orders.csv.

import fs from "node:fs";
import path from "node:path";
import { computeExpectedReport } from "../fixture.mjs";

/**
 * Computes the report from the workspace orders.csv and writes report.json.
 * @param workspaceDir - the workspace folder
 */
export async function solve(workspaceDir) {
  const csvText = fs.readFileSync(path.join(workspaceDir, "orders.csv"), "utf8");
  fs.writeFileSync(path.join(workspaceDir, "report.json"), `${JSON.stringify(computeExpectedReport(csvText), null, 2)}\n`);
}
