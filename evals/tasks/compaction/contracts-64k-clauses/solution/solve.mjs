// Reference solution: writes report.txt.

import fs from "node:fs";
import path from "node:path";
import { buildContracts, expectedReport } from "../fixture.mjs";

/**
 * Writes the expected clause report.
 * @param workspaceDir - the prepared workspace
 * @param options - { seed }
 */
export async function solve(workspaceDir, { seed }) {
  fs.writeFileSync(path.join(workspaceDir, "report.txt"), expectedReport(buildContracts(seed)).join("\n") + "\n");
}
