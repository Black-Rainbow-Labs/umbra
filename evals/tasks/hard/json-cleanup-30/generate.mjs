// Writes the 30 messy customer files into data/ of the workspace.

import fs from "node:fs";
import path from "node:path";
import { buildDataset } from "./hidden/dataset.mjs";

/**
 * Generates data/customers-01.json to data/customers-30.json.
 * @param workspaceDir - the workspace folder
 * @param options - { seed }
 */
export async function generate(workspaceDir, { seed }) {
  const dataDir = path.join(workspaceDir, "data");
  fs.mkdirSync(dataDir, { recursive: true });
  for (const file of buildDataset(seed)) fs.writeFileSync(path.join(dataDir, file.name), file.input, "utf8");
}
