// Writes the 40 record files into the workspace.

import fs from "node:fs";
import path from "node:path";
import { buildRecords } from "./fixture.mjs";

/**
 * Generates records/rec-01.txt to rec-40.txt for the given seed.
 * @param workspaceDir - the workspace to fill
 * @param options - { seed }
 */
export async function generate(workspaceDir, { seed }) {
  for (const record of buildRecords(seed).records) {
    const file = path.join(workspaceDir, record.path);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, record.text);
  }
}
