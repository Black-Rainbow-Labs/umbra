// Writes the ten CSV files into the workspace.

import fs from "node:fs";
import path from "node:path";
import { buildTables } from "./fixture.mjs";

/**
 * Generates data/<name>.csv for the given seed.
 * @param workspaceDir - the workspace to fill
 * @param options - { seed }
 */
export async function generate(workspaceDir, { seed }) {
  fs.mkdirSync(path.join(workspaceDir, "data"), { recursive: true });
  for (const table of buildTables(seed)) fs.writeFileSync(path.join(workspaceDir, "data", `${table.name}.csv`), table.csv);
}
