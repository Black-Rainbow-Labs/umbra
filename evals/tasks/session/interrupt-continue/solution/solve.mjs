// Reference solution: writes every JSON file and the index.

import fs from "node:fs";
import path from "node:path";
import { buildTables, expectedIndex } from "../fixture.mjs";

/**
 * Writes out/<name>.json and out/index.json.
 * @param workspaceDir - the prepared workspace
 * @param options - { seed }
 */
export async function solve(workspaceDir, { seed }) {
  const tables = buildTables(seed);
  fs.mkdirSync(path.join(workspaceDir, "out"), { recursive: true });
  for (const table of tables) fs.writeFileSync(path.join(workspaceDir, "out", `${table.name}.json`), JSON.stringify(table.rows, null, 2));
  fs.writeFileSync(path.join(workspaceDir, "out", "index.json"), JSON.stringify(expectedIndex(tables)));
}
